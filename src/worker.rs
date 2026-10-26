use crate::cache::ByteCache;
use crate::document::{DocumentId, ErrorKind, ReaderError, RenderKey, SearchRequest, TextMarkup};
use crate::engine::{PdfEngine, PdfiumEngine, bind_pdfium};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;

pub use crate::application::protocol::WorkerEvent;

enum Command {
    Cover(PathBuf, Option<PathBuf>, Result<PathBuf, ReaderError>),
    Open(
        DocumentId,
        PathBuf,
        Option<String>,
        Option<Result<PathBuf, ReaderError>>,
        bool,
    ),
    Close(DocumentId),
    Render(RenderKey),
    Content(DocumentId, usize),
    Search(SearchRequest),
    SearchPage(SearchRequest, usize),
    Save(DocumentId, PathBuf),
    Mark(DocumentId, TextMarkup),
    UndoMark(DocumentId),
}
impl Command {
    fn document_id(&self) -> DocumentId {
        match self {
            Self::Cover(..) => 0,
            Self::Open(id, ..)
            | Self::Close(id)
            | Self::Content(id, ..)
            | Self::Save(id, ..)
            | Self::Mark(id, ..)
            | Self::UndoMark(id) => *id,
            Self::Render(key) => key.document_id,
            Self::Search(request) | Self::SearchPage(request, _) => request.document_id,
        }
    }
}
#[derive(Default)]
struct Pending {
    control: VecDeque<Command>,
    renders: VecDeque<RenderKey>,
    prefetch: VecDeque<RenderKey>,
    search: Option<(SearchRequest, usize, usize)>,
    cancelled_searches: HashMap<DocumentId, u64>,
    stop: bool,
    importing: HashMap<DocumentId, Arc<AtomicBool>>,
}
struct Mailbox {
    pending: Mutex<Pending>,
    wake: Condvar,
}
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
        let mailbox = Arc::new(Mailbox {
            pending: Mutex::new(Pending::default()),
            wake: Condvar::new(),
        });
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
                                if let Ok(mut pending) = background.pending.lock()
                                    && !pending.stop
                                {
                                    pending.importing.insert(id, Arc::clone(&cancelled));
                                    if repair_sender
                                        .try_send(ImportRequest::Repair(
                                            id,
                                            path.clone(),
                                            password.clone(),
                                            prepared,
                                            identity,
                                            cancelled,
                                        ))
                                        .is_ok()
                                    {
                                        continue;
                                    }
                                    pending.importing.remove(&id);
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
                            if !import_mailbox.pending.lock().is_ok_and(|pending| {
                                !pending.stop && pending.importing.contains_key(&id)
                            }) {
                                continue;
                            }
                            let result = crate::formats::prepare_cancellable_in(
                                &path,
                                &import_data,
                                cancelled,
                                Arc::clone(&import_shutdown),
                            );
                            if let Ok(mut pending) = import_mailbox.pending.lock()
                                && !pending.stop
                                && pending.importing.remove(&id).is_some()
                            {
                                pending.control.push_back(Command::Open(
                                    id,
                                    path,
                                    password,
                                    Some(result),
                                    false,
                                ));
                                import_mailbox.wake.notify_one();
                            }
                        }
                        ImportRequest::Repair(
                            id,
                            path,
                            password,
                            rejected,
                            identity,
                            cancelled,
                        ) => {
                            if !import_mailbox.pending.lock().is_ok_and(|pending| {
                                !pending.stop && pending.importing.contains_key(&id)
                            }) {
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
                            if let Ok(mut pending) = import_mailbox.pending.lock()
                                && !pending.stop
                                && pending.importing.remove(&id).is_some()
                            {
                                pending.control.push_back(Command::Open(
                                    id,
                                    path,
                                    password,
                                    Some(result),
                                    true,
                                ));
                                import_mailbox.wake.notify_one();
                            }
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
                            if let Ok(mut pending) = import_mailbox.pending.lock()
                                && !pending.stop
                            {
                                pending
                                    .control
                                    .push_back(Command::Cover(path, cache, prepared));
                                import_mailbox.wake.notify_one();
                            }
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
            self.mailbox
                .pending
                .lock()
                .map_err(|_| worker_error())?
                .importing
                .insert(id, Arc::clone(&cancelled));
            if self
                .imports
                .try_send(ImportRequest::Open(id, path, password, cancelled))
                .is_err()
            {
                self.mailbox
                    .pending
                    .lock()
                    .map_err(|_| worker_error())?
                    .importing
                    .remove(&id);
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
        let mut pending = self.mailbox.pending.lock().map_err(|_| worker_error())?;
        if let Some(cancelled) = pending.importing.remove(&id) {
            cancelled.store(true, Ordering::Relaxed);
        }
        pending.control.retain(|c| c.document_id() != id);
        pending.renders.retain(|k| k.document_id != id);
        pending.prefetch.retain(|k| k.document_id != id);
        if pending
            .search
            .as_ref()
            .is_some_and(|(r, _, _)| r.document_id == id)
        {
            pending.search = None;
        }
        pending.control.push_back(Command::Close(id));
        self.mailbox.wake.notify_one();
        Ok(())
    }
    pub fn render(&self, key: RenderKey) -> Result<(), ReaderError> {
        let mut pending = self.mailbox.pending.lock().map_err(|_| worker_error())?;
        pending.prefetch.retain(|queued| *queued != key);
        if !pending.renders.contains(&key) {
            pending.renders.push_back(key);
        }
        while pending.renders.len() > 96 {
            pending.renders.pop_front();
        }
        self.mailbox.wake.notify_one();
        Ok(())
    }
    /// Speculative work never delays a requested visible page.
    pub fn prefetch(&self, key: RenderKey) -> Result<(), ReaderError> {
        let mut pending = self.mailbox.pending.lock().map_err(|_| worker_error())?;
        if !pending.renders.contains(&key) && !pending.prefetch.contains(&key) {
            pending.prefetch.push_back(key);
        }
        while pending.prefetch.len() > 24 {
            pending.prefetch.pop_front();
        }
        self.mailbox.wake.notify_one();
        Ok(())
    }
    pub fn retain_renders(&self, keys: &[RenderKey]) -> Result<(), ReaderError> {
        let mut pending = self.mailbox.pending.lock().map_err(|_| worker_error())?;
        pending.renders.retain(|key| keys.contains(key));
        pending.prefetch.retain(|key| keys.contains(key));
        Ok(())
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
        let mut pending = self.mailbox.pending.lock().map_err(|_| worker_error())?;
        pending
            .cancelled_searches
            .entry(id)
            .and_modify(|cancelled| *cancelled = (*cancelled).max(generation))
            .or_insert(generation);
        pending.control.retain(|command| {
            !matches!(command, Command::Search(request)
                if request.document_id == id && request.generation == generation)
        });
        if pending
            .search
            .as_ref()
            .is_some_and(|(r, _, _)| r.document_id == id && r.generation == generation)
        {
            pending.search = None;
        }
        Ok(())
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
        let mut pending = self.mailbox.pending.lock().map_err(|_| worker_error())?;
        match &command {
            Command::Open(id, ..) => pending
                .control
                .retain(|c| !matches!(c,Command::Open(old,..)if old==id)),
            Command::Content(id, page) => {
                if pending
                    .control
                    .iter()
                    .any(|c| matches!(c,Command::Content(doc,p)if doc==id&&p==page))
                {
                    return Ok(());
                }
            }
            Command::Search(_) => {
                pending.search = None;
                pending.control.retain(|c| !matches!(c, Command::Search(_)));
            }
            Command::Cover(path, _, _)
                if pending.control.iter().any(
                    |command| matches!(command, Command::Cover(queued, _, _) if queued == path),
                ) =>
            {
                return Ok(());
            }
            _ => {}
        }
        pending.control.push_back(command);
        self.mailbox.wake.notify_one();
        Ok(())
    }
}
impl Mailbox {
    fn next(&self) -> Option<Command> {
        let mut pending = self.pending.lock().ok()?;
        loop {
            if pending.stop {
                return None;
            }
            if let Some(index) = pending
                .control
                .iter()
                .position(|command| !matches!(command, Command::Content(..) | Command::Cover(..)))
            {
                return pending.control.remove(index);
            }
            if let Some(key) = pending.renders.pop_front() {
                return Some(Command::Render(key));
            }
            if let Some(index) = pending
                .control
                .iter()
                .position(|command| matches!(command, Command::Content(..)))
            {
                return pending.control.remove(index);
            }
            if let Some(key) = pending.prefetch.pop_front() {
                return Some(Command::Render(key));
            }
            if let Some(command) = pending.control.pop_front() {
                return Some(command);
            }
            if let Some((request, next, total)) = pending.search.as_mut()
                && *next < *total
            {
                let page = *next;
                *next += 1;
                return Some(Command::SearchPage(request.clone(), page));
            }
            pending = self.wake.wait(pending).ok()?;
        }
    }
    fn start_search(&self, request: SearchRequest, total: usize) {
        if let Ok(mut p) = self.pending.lock()
            && p.cancelled_searches
                .get(&request.document_id)
                .is_none_or(|generation| request.generation > *generation)
        {
            p.search = Some((request, 0, total));
        }
    }
    fn forget_search(&self, id: DocumentId) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.cancelled_searches.remove(&id);
            if pending
                .search
                .as_ref()
                .is_some_and(|(request, _, _)| request.document_id == id)
            {
                pending.search = None;
            }
        }
    }
    fn cancel_search(&self) {
        if let Ok(mut p) = self.pending.lock() {
            p.search = None;
        }
    }
}
impl Drop for PdfWorker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Ok(mut pending) = self.mailbox.pending.lock() {
            for cancelled in pending.importing.values() {
                cancelled.store(true, Ordering::Relaxed);
            }
            pending.stop = true;
            self.mailbox.wake.notify_one();
        }
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
    fn cancelling_search_covers_queued_active_and_dequeued_requests_without_touching_other_tabs() {
        let mailbox = Arc::new(Mailbox {
            pending: Mutex::new(Pending::default()),
            wake: Condvar::new(),
        });
        let (_, events) = mpsc::sync_channel(2);
        let worker = PdfWorker {
            mailbox: Arc::clone(&mailbox),
            events,
            imports: mpsc::sync_channel(1).0,
            shutdown: Arc::new(AtomicBool::new(false)),
            import_thread: None,
        };
        let request = |document_id, generation| SearchRequest {
            document_id,
            generation,
            query: "reader".into(),
            match_case: false,
            whole_word: false,
        };
        worker.search(request(1, 10)).unwrap();
        worker.stop_search(2, 10).unwrap();
        assert!(matches!(
            mailbox.pending.lock().unwrap().control.front(),
            Some(Command::Search(_))
        ));
        worker.stop_search(1, 10).unwrap();
        assert!(mailbox.pending.lock().unwrap().control.is_empty());

        // A popped request cannot become an active search after it was cancelled.
        mailbox.start_search(request(1, 10), 20);
        assert!(mailbox.pending.lock().unwrap().search.is_none());
        mailbox.start_search(request(1, 11), 20);
        worker.stop_search(1, 10).unwrap();
        assert!(mailbox.pending.lock().unwrap().search.is_some());
        worker.stop_search(1, 11).unwrap();
        assert!(mailbox.pending.lock().unwrap().search.is_none());
        mailbox.start_search(request(3, 11), 20);
        worker.stop_search(1, 99).unwrap();
        assert_eq!(
            mailbox
                .pending
                .lock()
                .unwrap()
                .search
                .as_ref()
                .unwrap()
                .0
                .document_id,
            3
        );
        mailbox.forget_search(1);
        assert!(
            !mailbox
                .pending
                .lock()
                .unwrap()
                .cancelled_searches
                .contains_key(&1)
        );
        mailbox.forget_search(3);
        assert!(mailbox.pending.lock().unwrap().search.is_none());
    }

    #[test]
    fn visible_render_overtakes_prefetch_and_close_cancels_both() {
        let mailbox = Arc::new(Mailbox {
            pending: Mutex::new(Pending::default()),
            wake: Condvar::new(),
        });
        let (_, events) = mpsc::sync_channel(2);
        let worker = PdfWorker {
            mailbox: Arc::clone(&mailbox),
            events,
            imports: mpsc::sync_channel(1).0,
            shutdown: Arc::new(AtomicBool::new(false)),
            import_thread: None,
        };
        let key = RenderKey {
            document_id: 1,
            revision: 0,
            page: 0,
            zoom: crate::document::Zoom::FitWidth,
            viewport_width: 600,
            viewport_height: 700,
            density_milli: 1000,
            bitmap_side_limit: 8192,
            rotation: 0,
            colors: crate::document::PageColors::Original,
            region: None,
        };
        worker.prefetch(RenderKey { page: 1, ..key }).unwrap();
        worker.render(key).unwrap();
        assert!(matches!(mailbox.next(),Some(Command::Render(visible)) if visible.page==0));
        worker.close(1).unwrap();
        assert!(mailbox.pending.lock().unwrap().prefetch.is_empty());
        assert!(matches!(mailbox.next(), Some(Command::Close(1))));
    }
    #[test]
    fn closed_documents_leave_no_pending_open_requests() {
        let mailbox = Arc::new(Mailbox {
            pending: Mutex::new(Pending::default()),
            wake: Condvar::new(),
        });
        let (_, events) = mpsc::sync_channel(2);
        let worker = PdfWorker {
            mailbox: Arc::clone(&mailbox),
            events,
            imports: mpsc::sync_channel(1).0,
            shutdown: Arc::new(AtomicBool::new(false)),
            import_thread: None,
        };
        worker.open(1, PathBuf::from("one.pdf")).unwrap();
        worker.close(1).unwrap();
        assert!(matches!(mailbox.next(), Some(Command::Close(1))));
        assert!(mailbox.pending.lock().unwrap().control.is_empty());
    }
}
