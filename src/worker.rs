use crate::cache::ByteCache;
use crate::document::{DocumentId, ErrorKind, ReaderError, RenderKey, SearchRequest, TextMarkup};
use crate::engine::{PdfEngine, PdfiumEngine, bind_pdfium};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

pub use crate::application::protocol::WorkerEvent;

mod scheduler;
use scheduler::{Command, Mailbox};

enum ImportRequest {
    Open(DocumentId, PathBuf, Option<String>, Arc<AtomicBool>),
    Repair(
        DocumentId,
        PathBuf,
        Option<String>,
        PathBuf,
        crate::formats::CachedPdfIdentity,
        Arc<AtomicBool>,
    ),
    Cover(PathBuf, Option<PathBuf>),
}
pub struct PdfWorker {
    mailbox: Arc<Mailbox>,
    pub events: mpsc::Receiver<WorkerEvent>,
    imports: mpsc::SyncSender<ImportRequest>,
    shutdown: Arc<AtomicBool>,
    import_thread: Option<thread::JoinHandle<()>>,
}

impl PdfWorker {
    pub fn spawn(repaint: impl Fn() + Send + Sync + 'static) -> Result<Self, ReaderError> {
        Self::spawn_with_paths(
            repaint,
            crate::app_paths::AppPaths::from_environment(crate::platform::config_path()),
        )
    }

    pub fn spawn_with_paths(
        repaint: impl Fn() + Send + Sync + 'static,
        paths: crate::app_paths::AppPaths,
    ) -> Result<Self, ReaderError> {
        let repaint = Arc::new(repaint);
        let shutdown = Arc::new(AtomicBool::new(false));
        let render_data = paths.data.clone();
        let import_data = paths.data;
        let mailbox = Arc::new(Mailbox::default());
        let (sender, events) = mpsc::sync_channel(2);
        let (imports, import_requests) = mpsc::sync_channel::<ImportRequest>(16);
        let repair_sender = imports.clone();
        let background = Arc::clone(&mailbox);
        let render_repaint = Arc::clone(&repaint);
        let import_events = sender.clone();
        thread::Builder::new()
            .name("pdf-renderer".into())
            .spawn(move || {
                let pdfium = match bind_pdfium() {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = sender.send(WorkerEvent::Unavailable(e));
                        render_repaint();
                        return;
                    }
                };
                let mut documents = HashMap::new();
                let mut page_counts = HashMap::new();
                let mut rasters = ByteCache::new(if cfg!(target_pointer_width = "32") {
                    32 * 1024 * 1024
                } else {
                    96 * 1024 * 1024
                });
                let mut contents = ByteCache::new(16 * 1024 * 1024);
                let send = |event| {
                    let result = sender.send(event).is_ok();
                    render_repaint();
                    result
                };
                while let Some(command) = background.next() {
                    match command {
                        Command::Cover(path, cache, prepared) => {
                            let result = prepared.and_then(|prepared| {
                                crate::library::load_prepared(
                                    &pdfium,
                                    &path,
                                    &prepared,
                                    cache.as_deref(),
                                    &render_data,
                                )
                            });
                            if !send(WorkerEvent::Cover(path, result)) {
                                break;
                            }
                        }
                        Command::Open(id, path, mut password, prepared, repaired) => {
                            let mut engine =
                                PdfiumEngine::with_data_directory(&pdfium, render_data.clone());
                            let result = match &prepared {
                                Some(Ok(prepared)) => {
                                    engine.open_prepared(&path, prepared, password.as_deref())
                                }
                                Some(Err(error)) => Err(error.clone()),
                                None => engine.open_with_password(&path, password.as_deref()),
                            };
                            if !repaired
                                && matches!(result, Err(ref error) if matches!(error.kind, ErrorKind::InvalidPdf | ErrorKind::Empty))
                                && let Some(Ok(prepared)) = prepared
                                && let Some(identity) =
                                    crate::formats::cached_pdf_identity(&prepared)
                            {
                                let cancelled = Arc::new(AtomicBool::new(false));
                                if background.submit_import(id, Arc::clone(&cancelled), || {
                                    repair_sender.try_send(ImportRequest::Repair(
                                        id, path.clone(), password.clone(), prepared, identity, cancelled,
                                    )).is_ok()
                                }) {
                                    continue;
                                }
                            }
                            // Passwords are never persisted; drop the request after loading.
                            password.take();
                            let details = if result.is_ok() {
                                Some(engine.details())
                            } else {
                                None
                            };
                            if let Ok(info) = &result {
                                page_counts.insert(id, info.page_count);
                                documents.insert(id, engine);
                                rasters.retain(|k: &RenderKey| k.document_id != id);
                                contents.retain(|(doc, _): &(DocumentId, usize)| *doc != id);
                            }
                            if !send(WorkerEvent::Opened(id, result)) {
                                break;
                            }
                            if let Some(details) = details
                                && !send(WorkerEvent::Details(id, details))
                            {
                                break;
                            }
                        }
                        Command::Close(id) => {
                            background.forget_search(id);
                            documents.remove(&id);
                            page_counts.remove(&id);
                            rasters.retain(|k| k.document_id != id);
                            contents.retain(|(doc, _)| *doc != id);
                        }
                        Command::Render(key) => {
                            let Some(engine) = documents.get(&key.document_id) else {
                                continue;
                            };
                            if key.revision != engine.annotation_state().revision {
                                continue;
                            }
                            let result = if let Some(cached) = rasters.get(&key) {
                                Ok(cached)
                            } else {
                                engine.render(key).map(|page| {
                                    let size = page.rgba.len();
                                    let page = Arc::new(page);
                                    rasters.insert(key, Arc::clone(&page), size);
                                    page
                                })
                            };
                            if !send(WorkerEvent::Rendered(key, result)) {
                                break;
                            }
                        }
                        Command::Content(id, page) => {
                            let Some(engine) = documents.get_mut(&id) else {
                                continue;
                            };
                            let result = if let Some(cached) = contents.get(&(id, page)) {
                                Ok(cached)
                            } else {
                                engine.page_content(page).map(|content| {
                                    let bytes = content.memory_bytes();
                                    let content = Arc::new(content);
                                    contents.insert((id, page), Arc::clone(&content), bytes);
                                    content
                                })
                            };
                            if !send(WorkerEvent::Content(id, page, result)) {
                                break;
                            }
                        }
                        Command::Search(request) => {
                            if request.query.is_empty() {
                                background.cancel_search();
                            } else if let Some(total) = page_counts.get(&request.document_id) {
                                background.start_search(request, *total);
                            }
                        }
                        Command::SearchPage(request, page) => {
                            let Some(engine) = documents.get_mut(&request.document_id) else {
                                continue;
                            };
                            let total = page_counts.get(&request.document_id).copied().unwrap_or(0);
                            let result = engine.search_page(page, &request);
                            if result.is_err() {
                                background.cancel_search();
                            }
                            if !send(WorkerEvent::search_result(&request, page + 1, total, result)) {
                                break;
                            }
                        }
                        Command::Save(id, path) => {
                            let Some(engine) = documents.get_mut(&id) else {
                                continue;
                            };
                            let result = engine.save_copy(&path);
                            rasters.retain(|key| key.document_id != id);
                            if !send(WorkerEvent::Changed(id, engine.annotation_state())) {
                                break;
                            }
                            if !send(WorkerEvent::Saved(id, path, result)) {
                                break;
                            }
                        }
                        Command::Mark(id, mark) => {
                            let Some(engine) = documents.get_mut(&id) else {
                                continue;
                            };
                            let page = mark.page;
                            let result = engine.add_markup(mark).map(|count| (page, count));
                            rasters.retain(|key| key.document_id != id);
                            if !send(WorkerEvent::Changed(id, engine.annotation_state())) {
                                break;
                            }
                            let store = if result.is_ok() {
                                engine.mark_store_path().map(std::path::Path::to_owned)
                            } else {
                                None
                            };
                            if result.is_ok() {
                                rasters.retain(|key| key.document_id != id || key.page != page);
                            }
                            if !send(WorkerEvent::Marked(id, result)) {
                                break;
                            }
                            if let Some(store) = store {
                                for (other_id, other) in &mut documents {
                                    if *other_id != id
                                        && other.mark_store_path() == Some(store.as_path())
                                    {
                                        let result =
                                            other.refresh_marks().map(|count| (page, count));
                                        rasters.retain(|key| key.document_id != *other_id);
                                        if !send(WorkerEvent::Changed(
                                            *other_id,
                                            other.annotation_state(),
                                        )) {
                                            break;
                                        }
                                        if !send(WorkerEvent::Marked(*other_id, result)) {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        Command::UndoMark(id) => {
                            let Some(engine) = documents.get_mut(&id) else {
                                continue;
                            };
                            let result = engine.undo_markup();
                            rasters.retain(|key| key.document_id != id);
                            if !send(WorkerEvent::Changed(id, engine.annotation_state())) {
                                break;
                            }
                            let store = engine.mark_store_path().map(std::path::Path::to_owned);
                            let page = result.as_ref().ok().map(|(page, _)| *page);
                            if let Ok((page, _)) = &result {
                                rasters.retain(|key| key.document_id != id || key.page != *page);
                            }
                            if !send(WorkerEvent::Marked(id, result)) {
                                break;
                            }
                            if let Some((store, page)) = store.zip(page) {
                                for (other_id, other) in &mut documents {
                                    if *other_id != id
                                        && other.mark_store_path() == Some(store.as_path())
                                    {
                                        let result =
                                            other.refresh_marks().map(|count| (page, count));
                                        rasters.retain(|key| key.document_id != *other_id);
                                        if !send(WorkerEvent::Changed(
                                            *other_id,
                                            other.annotation_state(),
                                        )) {
                                            break;
                                        }
                                        if !send(WorkerEvent::Marked(*other_id, result)) {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .map_err(|e| {
                ReaderError::new(
                    ErrorKind::Worker,
                    format!("No se pudo iniciar el motor PDF: {e}"),
                )
            })?;
        let import_mailbox = Arc::clone(&mailbox);
        let import_shutdown = Arc::clone(&shutdown);
        let import_thread = thread::Builder::new()
            .name("crabpdf-import".into())
            .spawn(move || {
                while let Ok(request) = import_requests.recv() {
                    match request {
                        ImportRequest::Open(id, path, password, cancelled) => {
                            if !import_mailbox.is_import_pending(id) {
                                continue;
                            }
                            let result = crate::formats::prepare_cancellable_in(
                                &path,
                                &import_data,
                                cancelled,
                                Arc::clone(&import_shutdown),
                            );
                            import_mailbox.finish_import(Command::Open(
                                id,
                                path,
                                password,
                                Some(result),
                                false,
                            ));
                        }
                        ImportRequest::Repair(
                            id,
                            path,
                            password,
                            rejected,
                            identity,
                            cancelled,
                        ) => {
                            if !import_mailbox.is_import_pending(id) {
                                continue;
                            }
                            let result = crate::formats::repair_cancellable_in(
                                &path,
                                &rejected,
                                identity,
                                &import_data,
                                cancelled,
                                Arc::clone(&import_shutdown),
                            );
                            import_mailbox.finish_import(Command::Open(
                                id,
                                path,
                                password,
                                Some(result),
                                true,
                            ));
                        }
                        ImportRequest::Cover(path, cache) => {
                            if let Some(cover) = crate::library::cached(&path, cache.as_deref()) {
                                if import_events
                                    .send(WorkerEvent::Cover(path, Ok(cover)))
                                    .is_err()
                                {
                                    break;
                                }
                                repaint();
                                continue;
                            }
                            let prepared = crate::formats::prepare_cancellable_in(
                                &path,
                                &import_data,
                                Arc::new(AtomicBool::new(false)),
                                Arc::clone(&import_shutdown),
                            );
                            import_mailbox.finish_cover(Command::Cover(path, cache, prepared));
                        }
                    }
                }
            })
            .map_err(|error| {
                ReaderError::new(
                    ErrorKind::Worker,
                    "No se pudo iniciar la lectura de formatos.",
                )
                .with_details(error.to_string())
            })?;
        Ok(Self {
            mailbox,
            events,
            imports,
            shutdown,
            import_thread: Some(import_thread),
        })
    }
    pub fn open(&self, id: DocumentId, path: PathBuf) -> Result<(), ReaderError> {
        self.open_with_password(id, path, None)
    }
    pub fn open_with_password(
        &self,
        id: DocumentId,
        path: PathBuf,
        password: Option<String>,
    ) -> Result<(), ReaderError> {
        if matches!(crate::formats::Format::detect(&path), Ok(format) if format!=crate::formats::Format::Pdf)
        {
            let cancelled = Arc::new(AtomicBool::new(false));
            if !self.mailbox.submit_import(id, Arc::clone(&cancelled), || {
                self.imports
                    .try_send(ImportRequest::Open(id, path, password, cancelled))
                    .is_ok()
            }) {
                return Err(ReaderError::new(
                    ErrorKind::Worker,
                    "Hay demasiados documentos pendientes de apertura. Espera a que terminen.",
                ));
            }
            Ok(())
        } else {
            self.enqueue(Command::Open(id, path, password, None, false))
        }
    }
    pub fn close(&self, id: DocumentId) -> Result<(), ReaderError> {
        self.mailbox.close(id)
    }
    pub fn render(&self, key: RenderKey) -> Result<(), ReaderError> {
        self.mailbox.render(key)
    }
    /// Speculative work never delays a requested visible page.
    pub fn prefetch(&self, key: RenderKey) -> Result<(), ReaderError> {
        self.mailbox.prefetch(key)
    }
    pub fn retain_renders(&self, keys: &[RenderKey]) -> Result<(), ReaderError> {
        self.mailbox.retain_renders(keys)
    }
    pub fn content(&self, id: DocumentId, page: usize) -> Result<(), ReaderError> {
        self.enqueue(Command::Content(id, page))
    }
    pub fn cover(&self, path: PathBuf, cache: Option<PathBuf>) -> Result<(), ReaderError> {
        self.imports
            .try_send(ImportRequest::Cover(path, cache))
            .map_err(|_| {
                ReaderError::new(
                    ErrorKind::Worker,
                    "Hay demasiadas portadas pendientes. Vuelve a Inicio para reintentarlo.",
                )
            })
    }
    pub fn search(&self, request: SearchRequest) -> Result<(), ReaderError> {
        self.enqueue(Command::Search(request))
    }
    pub fn stop_search(&self, id: DocumentId, generation: u64) -> Result<(), ReaderError> {
        self.mailbox.stop_search(id, generation)
    }
    pub fn save(&self, id: DocumentId, path: PathBuf) -> Result<(), ReaderError> {
        self.enqueue(Command::Save(id, path))
    }
    pub fn mark(&self, id: DocumentId, mark: TextMarkup) -> Result<(), ReaderError> {
        self.enqueue(Command::Mark(id, mark))
    }
    pub fn undo_mark(&self, id: DocumentId) -> Result<(), ReaderError> {
        self.enqueue(Command::UndoMark(id))
    }
    fn enqueue(&self, command: Command) -> Result<(), ReaderError> {
        self.mailbox.enqueue(command)
    }
}
impl Drop for PdfWorker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.mailbox.stop();
        let (replacement, receiver) = mpsc::sync_channel(0);
        drop(receiver);
        drop(std::mem::replace(&mut self.imports, replacement));
        let (sender, replacement) = mpsc::sync_channel(0);
        drop(sender);
        drop(std::mem::replace(&mut self.events, replacement));
        if let Some(thread) = self.import_thread.take() {
            let _ = thread.join();
        }
    }
}
fn worker_error() -> ReaderError {
    ReaderError::new(
        ErrorKind::Worker,
        "El hilo de renderizado dejó de responder. Reinicia la aplicación.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dropping_worker_disconnects_full_events_before_joining_import_thread() {
        let mailbox = Arc::new(Mailbox::default());
        let (sender, events) = mpsc::sync_channel(2);
        let event = || WorkerEvent::Unavailable(worker_error());
        sender.send(event()).unwrap();
        sender.send(event()).unwrap();
        let (ready, started) = mpsc::channel();
        let (done, completed) = mpsc::channel();
        let import_thread = thread::spawn(move || {
            ready.send(()).unwrap();
            assert!(sender.send(event()).is_err());
        });
        started
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let worker = PdfWorker {
            mailbox,
            events,
            imports: mpsc::sync_channel(1).0,
            shutdown: Arc::new(AtomicBool::new(false)),
            import_thread: Some(import_thread),
        };
        thread::spawn(move || {
            drop(worker);
            done.send(()).unwrap();
        });
        completed
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("Worker shutdown blocked on a full event channel");
    }
}
