//! Serial native document owner and byte-budgeted CPU caches.
use super::{
    WorkerEvent,
    importer::ImportRequest,
    scheduler::{Command, Mailbox},
};
use crate::{
    cache::ByteCache,
    document::*,
    engine::{PdfEngine, PdfiumEngine, bind_pdfium},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool, mpsc},
};

pub(super) fn run(
    background: Arc<Mailbox>,
    render_data: PathBuf,
    repair_sender: mpsc::SyncSender<ImportRequest>,
    sender: mpsc::SyncSender<WorkerEvent>,
    render_repaint: Arc<dyn Fn() + Send + Sync>,
) {
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
                let mut engine = PdfiumEngine::with_data_directory(&pdfium, render_data.clone());
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
                    && let Some(identity) = crate::formats::cached_pdf_identity(&prepared)
                {
                    let cancelled = Arc::new(AtomicBool::new(false));
                    if background.submit_import(id, Arc::clone(&cancelled), || {
                        repair_sender
                            .try_send(ImportRequest::Repair(
                                id,
                                path.clone(),
                                password.clone(),
                                prepared,
                                identity,
                                cancelled,
                            ))
                            .is_ok()
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
                if !send(WorkerEvent::search_result(
                    &request,
                    page + 1,
                    total,
                    result,
                )) {
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
                        if *other_id != id && other.mark_store_path() == Some(store.as_path()) {
                            let result = other.refresh_marks().map(|count| (page, count));
                            rasters.retain(|key| key.document_id != *other_id);
                            if !send(WorkerEvent::Changed(*other_id, other.annotation_state())) {
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
                        if *other_id != id && other.mark_store_path() == Some(store.as_path()) {
                            let result = other.refresh_marks().map(|count| (page, count));
                            rasters.retain(|key| key.document_id != *other_id);
                            if !send(WorkerEvent::Changed(*other_id, other.annotation_state())) {
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
}
