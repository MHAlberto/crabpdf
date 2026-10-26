//! Bounded render queues and cancellation, independent of PDFium, UI and import I/O.
//! Priority: control writes/open/close, visible renders, content, prefetch, covers, search.
//! Search is deliberately idle work; a continuous render stream can postpone it.
use super::worker_error;
use crate::document::{DocumentId, ReaderError, RenderKey, SearchRequest, TextMarkup};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

pub(super) enum Command {
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
#[derive(Default)]
pub(super) struct Mailbox {
    pending: Mutex<Pending>,
    wake: Condvar,
}
impl Mailbox {
    pub(super) fn next(&self) -> Option<Command> {
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
    pub(super) fn start_search(&self, request: SearchRequest, total: usize) {
        if let Ok(mut p) = self.pending.lock()
            && p.cancelled_searches
                .get(&request.document_id)
                .is_none_or(|generation| request.generation > *generation)
        {
            p.search = Some((request, 0, total));
        }
    }
    pub(super) fn forget_search(&self, id: DocumentId) {
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
    pub(super) fn cancel_search(&self) {
        if let Ok(mut p) = self.pending.lock() {
            p.search = None;
        }
    }

    pub(super) fn close(&self, id: DocumentId) -> Result<(), ReaderError> {
        let mut pending = self.pending.lock().map_err(|_| worker_error())?;
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
        self.wake.notify_one();
        Ok(())
    }
    pub(super) fn render(&self, key: RenderKey) -> Result<(), ReaderError> {
        let mut pending = self.pending.lock().map_err(|_| worker_error())?;
        pending.prefetch.retain(|queued| *queued != key);
        if !pending.renders.contains(&key) {
            pending.renders.push_back(key);
        }
        while pending.renders.len() > 96 {
            pending.renders.pop_front();
        }
        self.wake.notify_one();
        Ok(())
    }
    pub(super) fn prefetch(&self, key: RenderKey) -> Result<(), ReaderError> {
        let mut pending = self.pending.lock().map_err(|_| worker_error())?;
        if !pending.renders.contains(&key) && !pending.prefetch.contains(&key) {
            pending.prefetch.push_back(key);
        }
        while pending.prefetch.len() > 24 {
            pending.prefetch.pop_front();
        }
        self.wake.notify_one();
        Ok(())
    }
    pub(super) fn retain_renders(&self, keys: &[RenderKey]) -> Result<(), ReaderError> {
        let mut pending = self.pending.lock().map_err(|_| worker_error())?;
        pending.renders.retain(|key| keys.contains(key));
        pending.prefetch.retain(|key| keys.contains(key));
        Ok(())
    }
    pub(super) fn stop_search(&self, id: DocumentId, generation: u64) -> Result<(), ReaderError> {
        let mut pending = self.pending.lock().map_err(|_| worker_error())?;
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
    pub(super) fn enqueue(&self, command: Command) -> Result<(), ReaderError> {
        let mut pending = self.pending.lock().map_err(|_| worker_error())?;
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
        self.wake.notify_one();
        Ok(())
    }

    /// Register and submit under the same lock as close/completion.
    pub(super) fn submit_import(
        &self,
        id: DocumentId,
        cancelled: Arc<AtomicBool>,
        submit: impl FnOnce() -> bool,
    ) -> bool {
        let Ok(mut pending) = self.pending.lock() else {
            return false;
        };
        if pending.stop {
            return false;
        }
        pending.importing.insert(id, cancelled);
        if submit() {
            true
        } else {
            pending.importing.remove(&id);
            false
        }
    }
    pub(super) fn is_import_pending(&self, id: DocumentId) -> bool {
        self.pending
            .lock()
            .is_ok_and(|pending| !pending.stop && pending.importing.contains_key(&id))
    }
    pub(super) fn finish_import(&self, command: Command) {
        let id = command.document_id();
        if let Ok(mut pending) = self.pending.lock()
            && !pending.stop
            && pending.importing.remove(&id).is_some()
        {
            pending.control.push_back(command);
            self.wake.notify_one();
        }
    }
    pub(super) fn finish_cover(&self, command: Command) {
        if let Ok(mut pending) = self.pending.lock()
            && !pending.stop
        {
            pending.control.push_back(command);
            self.wake.notify_one();
        }
    }
    pub(super) fn stop(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            for cancelled in pending.importing.values() {
                cancelled.store(true, Ordering::Relaxed);
            }
            pending.stop = true;
            self.wake.notify_one();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelling_search_covers_queued_active_and_dequeued_requests_without_touching_other_tabs() {
        let mailbox = Mailbox::default();
        let request = |document_id, generation| SearchRequest {
            document_id,
            generation,
            query: "reader".into(),
            match_case: false,
            whole_word: false,
        };
        mailbox.enqueue(Command::Search(request(1, 10))).unwrap();
        mailbox.stop_search(2, 10).unwrap();
        assert!(matches!(
            mailbox.pending.lock().unwrap().control.front(),
            Some(Command::Search(_))
        ));
        mailbox.stop_search(1, 10).unwrap();
        assert!(mailbox.pending.lock().unwrap().control.is_empty());

        // A popped request cannot become an active search after it was cancelled.
        mailbox.start_search(request(1, 10), 20);
        assert!(mailbox.pending.lock().unwrap().search.is_none());
        mailbox.start_search(request(1, 11), 20);
        mailbox.stop_search(1, 10).unwrap();
        assert!(mailbox.pending.lock().unwrap().search.is_some());
        mailbox.stop_search(1, 11).unwrap();
        assert!(mailbox.pending.lock().unwrap().search.is_none());
        mailbox.start_search(request(3, 11), 20);
        mailbox.stop_search(1, 99).unwrap();
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
        let mailbox = Mailbox::default();
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
        mailbox.prefetch(RenderKey { page: 1, ..key }).unwrap();
        mailbox.render(key).unwrap();
        assert!(matches!(mailbox.next(),Some(Command::Render(visible)) if visible.page==0));
        mailbox.close(1).unwrap();
        assert!(mailbox.pending.lock().unwrap().prefetch.is_empty());
        assert!(matches!(mailbox.next(), Some(Command::Close(1))));
    }
    #[test]
    fn closed_documents_leave_no_pending_open_requests() {
        let mailbox = Mailbox::default();
        mailbox
            .enqueue(Command::Open(
                1,
                PathBuf::from("one.pdf"),
                None,
                None,
                false,
            ))
            .unwrap();
        mailbox.close(1).unwrap();
        assert!(matches!(mailbox.next(), Some(Command::Close(1))));
        assert!(mailbox.pending.lock().unwrap().control.is_empty());
    }

    fn key(page: usize) -> RenderKey {
        RenderKey {
            document_id: 1,
            revision: 0,
            page,
            zoom: crate::document::Zoom::FitWidth,
            viewport_width: 600,
            viewport_height: 700,
            density_milli: 1000,
            bitmap_side_limit: 8192,
            rotation: 0,
            colors: crate::document::PageColors::Original,
            region: None,
        }
    }
    #[test]
    fn writes_keep_order_and_background_search_resumes_after_reading() {
        let mailbox = Mailbox::default();
        mailbox.start_search(
            SearchRequest {
                document_id: 1,
                generation: 1,
                query: "reader".into(),
                match_case: false,
                whole_word: false,
            },
            2,
        );
        mailbox.finish_cover(Command::Cover(
            "book.pdf".into(),
            None,
            Ok("book.pdf".into()),
        ));
        mailbox.prefetch(key(9)).unwrap();
        mailbox.enqueue(Command::Content(1, 3)).unwrap();
        mailbox.render(key(4)).unwrap();
        mailbox.enqueue(Command::UndoMark(1)).unwrap();
        mailbox
            .enqueue(Command::Save(1, "copy.pdf".into()))
            .unwrap();
        assert!(matches!(mailbox.next(), Some(Command::UndoMark(1))));
        assert!(matches!(mailbox.next(), Some(Command::Save(1, _))));
        assert!(matches!(mailbox.next(), Some(Command::Render(k)) if k.page == 4));
        assert!(matches!(mailbox.next(), Some(Command::Content(1, 3))));
        assert!(matches!(mailbox.next(), Some(Command::Render(k)) if k.page == 9));
        assert!(matches!(mailbox.next(), Some(Command::Cover(..))));
        assert!(matches!(mailbox.next(), Some(Command::SearchPage(_, 0))));
        // Current policy: even new renders overtake the next search page.
        mailbox.render(key(5)).unwrap();
        assert!(matches!(mailbox.next(), Some(Command::Render(k)) if k.page == 5));
        assert!(matches!(mailbox.next(), Some(Command::SearchPage(_, 1))));
    }
    #[test]
    fn bounded_queues_deduplicate_promote_and_discard_obsolete_renders() {
        let mailbox = Mailbox::default();
        for page in 0..110 {
            mailbox.render(key(page)).unwrap();
        }
        for page in 110..140 {
            mailbox.prefetch(key(page)).unwrap();
        }
        mailbox.render(key(139)).unwrap();
        mailbox.render(key(139)).unwrap();
        mailbox.prefetch(key(139)).unwrap();
        let pending = mailbox.pending.lock().unwrap();
        assert_eq!(pending.renders.len(), 96);
        assert_eq!(pending.prefetch.len(), 23);
        assert_eq!(pending.renders.front().unwrap().page, 15);
        assert_eq!(pending.prefetch.front().unwrap().page, 116);
        drop(pending);
        mailbox.retain_renders(&[key(139), key(138)]).unwrap();
        assert!(matches!(mailbox.next(), Some(Command::Render(k)) if k.page == 139));
        assert!(matches!(mailbox.next(), Some(Command::Render(k)) if k.page == 138));
        assert!(mailbox.pending.lock().unwrap().prefetch.is_empty());
    }
    #[test]
    fn closed_import_cannot_reopen_document_and_failed_submission_leaves_no_token() {
        let mailbox = Mailbox::default();
        let cancelled = Arc::new(AtomicBool::new(false));
        assert!(mailbox.submit_import(1, cancelled.clone(), || true));
        assert!(mailbox.is_import_pending(1));
        mailbox.close(1).unwrap();
        assert!(cancelled.load(Ordering::Relaxed));
        mailbox.finish_import(Command::Open(1, "late.pdf".into(), None, None, false));
        assert!(matches!(mailbox.next(), Some(Command::Close(1))));
        assert!(mailbox.pending.lock().unwrap().control.is_empty());
        assert!(!mailbox.submit_import(2, cancelled.clone(), || false));
        assert!(!mailbox.is_import_pending(2));
        mailbox.stop();
        assert!(!mailbox.submit_import(3, cancelled, || panic!("Stopped mailbox submitted work")));
        assert!(mailbox.next().is_none());
    }
    #[test]
    fn stop_wakes_an_empty_waiter_and_cancels_imports() {
        let mailbox = Arc::new(Mailbox::default());
        let cancelled = Arc::new(AtomicBool::new(false));
        assert!(mailbox.submit_import(1, cancelled.clone(), || true));
        let background = mailbox.clone();
        let (done, completed) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            done.send(background.next().is_none()).unwrap();
        });
        mailbox.stop();
        assert!(
            completed
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
        );
        thread.join().unwrap();
        assert!(cancelled.load(Ordering::Relaxed));
    }
}
