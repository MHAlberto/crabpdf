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
        self.pages.retain(|key| key.document_id != tab.reader.id);
        self.content.retain(|(id, _)| *id != tab.reader.id);
        self.pending_pages
            .retain(|key| key.document_id != tab.reader.id);
        self.pending_content.retain(|(id, _)| *id != tab.reader.id);
        if index < self.active {
            self.active -= 1;
        } else {
            self.active = self.active.min(self.tabs.len().saturating_sub(1));
        }
        self.save_preferences();
    }
}
