//! Bounded conversion coordinator. It never owns PDFium documents or UI resources.
use super::{
    WorkerEvent,
    scheduler::{Command, Mailbox},
};
use crate::document::DocumentId;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool, mpsc},
};

pub(super) enum ImportRequest {
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

pub(super) fn run(
    import_requests: mpsc::Receiver<ImportRequest>,
    import_mailbox: Arc<Mailbox>,
    import_shutdown: Arc<AtomicBool>,
    import_data: PathBuf,
    import_events: mpsc::SyncSender<WorkerEvent>,
    repaint: Arc<dyn Fn() + Send + Sync>,
) {
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
            ImportRequest::Repair(id, path, password, rejected, identity, cancelled) => {
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
                import_mailbox.finish_import(Command::Open(id, path, password, Some(result), true));
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
}
