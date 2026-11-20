//! Desktop session coordination. Reading rules live in crabpdf-application.
use super::*;

impl ReaderApp {
    pub fn open_path(&mut self, path: PathBuf) {
        self.open_document(path, None);
    }
    pub(super) fn open_document(&mut self, path: PathBuf, restore: Option<StoredDocument>) {
        self.open_document_inner(path, restore, false);
    }
    pub(super) fn open_document_inner(
        &mut self,
        path: PathBuf,
        restore: Option<StoredDocument>,
        duplicate: bool,
    ) {
        self.home = false;
        self.reveal_tab = true;
        let path = path.canonicalize().unwrap_or(path);
        if !duplicate && let Some(index) = self.tabs.iter().position(|tab| tab.reader.path == path)
        {
            self.active = index;
            if let Some(saved) = restore {
                self.tabs[index].go_to(saved.page, true);
            }
            return;
        }
        if self.worker.is_none() {
            return;
        }
        if self.tabs.len() >= 64 {
            self.error(ReaderError::new(
                ErrorKind::File,
                "Cierra una pestaña antes de abrir más documentos.",
            ));
            return;
        }
        self.next_id += 1;
        let id = self.next_id;
        let saved = restore.or_else(|| {
            self.preferences
                .recent
                .iter()
                .find(|entry| entry.path == path)
                .cloned()
        });
        self.tabs.push(Tab::new(id, path.clone(), saved));
        self.active = self.tabs.len() - 1;
        self.submit_active_open();
    }

    pub(super) fn restore_tabs(&mut self, session: Vec<StoredDocument>) {
        if self.worker.is_none() {
            return;
        }
        for saved in session.into_iter().take(64) {
            self.next_id += 1;
            let mut tab = Tab::new(self.next_id, saved.path.clone(), Some(saved));
            tab.deferred_open = true;
            self.tabs.push(tab);
        }
        if !self.tabs.is_empty() {
            self.home = false;
            self.active = self.tabs.len() - 1;
            self.open_deferred_tab();
        }
    }

    pub(super) fn open_deferred_tab(&mut self) {
        if !self.home
            && let Some(tab) = self.tabs.get_mut(self.active)
            && std::mem::take(&mut tab.deferred_open)
        {
            self.submit_active_open();
        }
    }

    fn submit_active_open(&mut self) {
        let tab = &mut self.tabs[self.active];
        if let Some(worker) = &self.worker
            && let Err(error) = worker.open(tab.reader.id, tab.reader.path.clone())
        {
            tab.reader.state.finish_open(tab.reader.id, Err(error));
        }
    }

    pub(super) fn reload_active_tab(&mut self) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let Some(worker) = &self.worker else {
            return;
        };
        let id = tab.reader.id;
        let path = tab.reader.path.clone();
        let view_id = tab.view_id.clone();
        let saved = tab.stored().or_else(|| tab.reader.restore.clone());
        if let Err(error) = worker.close(id) {
            self.error(error);
            return;
        }
        self.forget_tab_cache(id);
        // A fresh worker identity rejects replies still in transit from the old document.
        self.next_id += 1;
        let id = self.next_id;
        self.tabs[self.active] = Tab::new(id, path.clone(), saved);
        self.tabs[self.active].view_id = view_id;
        if let Some(worker) = &self.worker
            && let Err(error) = worker.open(id, path)
        {
            self.tabs[self.active]
                .reader
                .state
                .finish_open(id, Err(error));
        }
    }

    pub(super) fn close_index(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        if let Some(saved) = self.tabs[index].stored() {
            self.closed.push(saved.clone());
            if self.closed.len() > 30 {
                self.closed.remove(0);
            }
            self.preferences.remember(saved);
        }
        let tab = self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.home = true;
        }
        if let Some(worker) = &self.worker
            && let Err(error) = worker.close(tab.reader.id)
        {
            self.empty.error = Some(error);
        }
        self.forget_tab_cache(tab.reader.id);
        if index < self.active {
            self.active -= 1;
        } else {
            self.active = self.active.min(self.tabs.len().saturating_sub(1));
        }
        self.save_preferences();
    }

    fn forget_tab_cache(&mut self, id: DocumentId) {
        self.pages.retain(|key| key.document_id != id);
        self.content.retain(|(doc, _)| *doc != id);
        self.pending_pages.retain(|key| key.document_id != id);
        self.pending_content.retain(|(doc, _)| *doc != id);
    }
}
