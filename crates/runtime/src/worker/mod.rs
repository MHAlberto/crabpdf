use crate::document::{DocumentId, ErrorKind, ReaderError, RenderKey, SearchRequest, TextMarkup};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

pub use crate::application::protocol::WorkerEvent;

mod importer;
mod renderer;
mod scheduler;
use importer::ImportRequest;
use scheduler::{Command, Mailbox};

pub struct PdfWorker {
    mailbox: Arc<Mailbox>,
    pub events: mpsc::Receiver<WorkerEvent>,
    imports: mpsc::SyncSender<ImportRequest>,
    shutdown: Arc<AtomicBool>,
    import_thread: Option<thread::JoinHandle<()>>,
    render_thread: Option<thread::JoinHandle<()>>,
}

impl PdfWorker {
    pub fn spawn(repaint: impl Fn() + Send + Sync + 'static) -> Result<Self, ReaderError> {
        Self::spawn_with_paths(
            repaint,
            crate::app_paths::AppPaths::from_environment(crate::paths::config_path()),
        )
    }

    pub fn spawn_with_paths(
        repaint: impl Fn() + Send + Sync + 'static,
        paths: crate::app_paths::AppPaths,
    ) -> Result<Self, ReaderError> {
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
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
        let render_thread = thread::Builder::new()
            .name("pdf-renderer".into())
            .spawn(move || {
                renderer::run(
                    background,
                    render_data,
                    repair_sender,
                    sender,
                    render_repaint,
                )
            })
            .map_err(|e| {
                ReaderError::new(
                    ErrorKind::Worker,
                    format!("No se pudo iniciar el motor PDF: {e}"),
                )
            })?;
        let import_mailbox = Arc::clone(&mailbox);
        let import_shutdown = Arc::clone(&shutdown);
        let import_thread =
            match thread::Builder::new()
                .name("crabpdf-import".into())
                .spawn(move || {
                    importer::run(
                        import_requests,
                        import_mailbox,
                        import_shutdown,
                        import_data,
                        import_events,
                        repaint,
                    )
                }) {
                Ok(thread) => thread,
                Err(error) => {
                    // Construction failure must wake the already-created render thread too.
                    shutdown.store(true, Ordering::Relaxed);
                    mailbox.stop();
                    drop(events);
                    let _ = render_thread.join();
                    return Err(ReaderError::new(
                        ErrorKind::Worker,
                        "No se pudo iniciar la lectura de formatos.",
                    )
                    .with_details(error.to_string()));
                }
            };
        Ok(Self {
            mailbox,
            events,
            imports,
            shutdown,
            import_thread: Some(import_thread),
            render_thread: Some(render_thread),
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
        if let Some(thread) = self.render_thread.take() {
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
        let render_sender = sender.clone();
        let render_thread = thread::spawn(move || {
            assert!(
                render_sender
                    .send(WorkerEvent::Unavailable(worker_error()))
                    .is_err()
            );
        });
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
            render_thread: Some(render_thread),
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
