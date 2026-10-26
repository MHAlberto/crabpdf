//! Desktop requests coordination. Reading rules live in crabpdf-application.
use super::*;

impl ReaderApp {
    pub(super) fn need_page(&mut self, key: RenderKey) -> Option<Arc<DisplayPage>> {
        self.frame_keys.push(key);
        if let Some(page) = self.pages.get(&key) {
            return Some(page);
        }
        if self.pending_pages.insert(key)
            && let Some(worker) = &self.worker
            && let Err(error) = worker.render(key)
        {
            self.pending_pages.remove(&key);
            self.error(error);
        }
        None
    }
    pub(super) fn need_content(&mut self, id: DocumentId, page: usize) -> Option<Arc<PageContent>> {
        // Text is requested only for pages that can receive interaction.
        if let Some(content) = self.content.get(&(id, page)) {
            return Some(content);
        }
        if self.pending_content.insert((id, page))
            && let Some(worker) = &self.worker
            && let Err(error) = worker.content(id, page)
        {
            self.pending_content.remove(&(id, page));
            self.error(error);
        }
        None
    }
    pub(super) fn start_search(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            self.next_search += 1;
            let previous = tab.reader.restart_search(self.next_search);
            if let Some(previous) = previous
                && let Some(worker) = &self.worker
            {
                let _ = worker.stop_search(previous.document_id, previous.generation);
            }
            tab.view.reveal = None;
            tab.view.search.due = tab
                .reader
                .search
                .is_active()
                .then(|| Instant::now() + Duration::from_millis(200));
        }
    }
    pub(super) fn prefetch_page(&mut self, key: RenderKey) {
        self.frame_keys.push(key);
        if self.pages.contains(&key) {
            return;
        }
        if self.pending_pages.insert(key)
            && let Some(worker) = &self.worker
            && let Err(error) = worker.prefetch(key)
        {
            self.pending_pages.remove(&key);
            self.error(error);
        }
    }
    pub(super) fn step_search(&mut self, forward: bool) {
        if let Some(tab) = self.tabs.get_mut(self.active)
            && let Some(destination) = tab.reader.search.step(forward)
        {
            tab.reveal_search(destination);
        }
    }
    pub(super) fn selected_text(&mut self) -> Option<String> {
        let tab = self.tabs.get(self.active)?;
        let selection = tab.reader.selection.range()?;
        Some(
            self.content
                .get(&(tab.reader.id, selection.page))?
                .text(selection.start, selection.end),
        )
    }
}
