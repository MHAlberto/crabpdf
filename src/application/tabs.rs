//! Logical state of an open tab. No GUI objects, file I/O or native handles.
use super::{
    profile::{LibraryEntry, StoredDocument},
    search::{SearchIdentity, SearchSession},
    selection::SelectionState,
};
use crate::{
    document::{DocumentDetails, DocumentId, DocumentInfo, ErrorKind, ReaderError, Zoom},
    layout::LayoutMode,
    reading_position::ReadingPosition,
    state::AppState,
};
use std::path::PathBuf;

/// Explicit initial reading options, shared by CLI and application entry points.
#[derive(Clone, Debug, Default)]
pub struct InitialView {
    pub page: Option<usize>,
    pub zoom: Option<Zoom>,
    pub layout: Option<LayoutMode>,
    pub continuous: Option<bool>,
    pub manga: Option<bool>,
    pub rotation: Option<u8>,
    pub search: Option<String>,
}
impl InitialView {
    fn overrides_position(&self) -> bool {
        self.page.is_some()
            || self.zoom.is_some()
            || self.layout.is_some()
            || self.continuous.is_some()
            || self.manga.is_some()
            || self.rotation.is_some()
            || self.search.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenOutcome {
    Ignored,
    Failed,
    Ready { initial_search: bool },
}

pub struct ReadingTab {
    pub id: DocumentId,
    pub path: PathBuf,
    pub state: AppState,
    pub search: SearchSession,
    pub selection: SelectionState,
    pub local_marks: usize,
    pub marking: bool,
    pub password_needed: bool,
    pub restore: Option<StoredDocument>,
    pub startup: Option<InitialView>,
    pub pending_position: Option<ReadingPosition>,
    pub layout_revision: u64,
}

impl ReadingTab {
    pub fn new(id: DocumentId, path: PathBuf, restore: Option<StoredDocument>) -> Self {
        let mut state = AppState::default();
        state.begin_open_with_id(id);
        Self {
            id,
            path,
            state,
            restore,
            search: SearchSession::default(),
            selection: SelectionState::default(),
            local_marks: 0,
            marking: false,
            password_needed: false,
            startup: None,
            pending_position: None,
            layout_revision: 0,
        }
    }

    /// Preserve restoration through failed password attempts. The adapter reveals
    /// and schedules an initial search using its own clock only after Ready.
    pub fn finish_open(
        &mut self,
        id: DocumentId,
        result: Result<DocumentInfo, ReaderError>,
        supports_password: bool,
    ) -> OpenOutcome {
        if id != self.id || self.state.opening != Some(id) {
            return OpenOutcome::Ignored;
        }
        let protected = supports_password
            && result
                .as_ref()
                .is_err_and(|error| error.kind == ErrorKind::Protected);
        self.state.finish_open(id, result);
        self.password_needed = protected;
        let Some(document) = &self.state.document else {
            return OpenOutcome::Failed;
        };
        let count = document.info.page_count;
        if let Some(saved) = self.restore.take() {
            self.state.set_zoom(saved.zoom);
            self.state.layout = saved.layout;
            self.state.continuous = saved.continuous;
            self.state.rotation = saved.rotation % 4;
            self.state.colors = saved.colors;
            self.state.manga = saved.manga;
            self.pending_position = saved
                .position
                .filter(|p| p.anchor((1.0, 1.0), count).is_some());
            self.state.go_to(
                self.pending_position.map_or(saved.page, |p| p.page()),
                false,
            );
        }
        if let Some(view) = self.startup.take() {
            if view.overrides_position() {
                self.pending_position = None;
            }
            if let Some(zoom) = view.zoom {
                self.state.set_zoom(zoom);
            }
            if let Some(layout) = view.layout {
                self.state.layout = layout;
            }
            if let Some(continuous) = view.continuous {
                self.state.continuous = continuous;
            }
            if let Some(manga) = view.manga {
                self.state.manga = manga;
            }
            if let Some(rotation) = view.rotation {
                self.state.rotation = rotation;
            }
            if let Some(page) = view.page {
                self.state.go_to(page, false);
            }
            if let Some(query) = view.search {
                self.search.query = query;
                self.search.total = count;
                return OpenOutcome::Ready {
                    initial_search: true,
                };
            }
        }
        OpenOutcome::Ready {
            initial_search: false,
        }
    }

    /// Retry the same document without consuming saved or explicit startup options.
    pub fn retry_open(&mut self) {
        self.password_needed = false;
        self.state.opening = Some(self.id);
        self.state.error = None;
    }

    pub fn go_to(&mut self, page: usize, remember: bool) {
        self.pending_position = None;
        self.selection.cancel_pending();
        self.state.go_to(page, remember);
    }

    pub fn restart_search(&mut self, generation: u64) -> Option<SearchIdentity> {
        let total = self
            .state
            .document
            .as_ref()
            .map_or(0, |d| d.info.page_count);
        self.search.restart(self.id, generation, total)
    }

    pub fn request_select_all(&mut self) -> Option<usize> {
        let page = self.state.document.as_ref()?.page;
        self.selection.request_all(self.id, page);
        Some(page)
    }

    pub fn accept_content(
        &mut self,
        id: DocumentId,
        page: usize,
        content: &crate::document::PageContent,
    ) {
        let current = self.state.document.as_ref().map(|d| d.page);
        self.selection.accept_content(id, page, current, content);
    }

    /// The view supplies a captured normalized position, never screen coordinates.
    pub fn stored(&self, captured_position: Option<ReadingPosition>) -> Option<StoredDocument> {
        let document = self.state.document.as_ref()?;
        Some(StoredDocument {
            path: self.path.clone(),
            page: document.page,
            zoom: self.state.zoom,
            layout: self.state.layout,
            continuous: self.state.continuous,
            rotation: self.state.rotation,
            colors: self.state.colors,
            manga: self.state.manga,
            position: self.pending_position.or(captured_position),
        })
    }

    pub fn apply_details(
        &mut self,
        details: DocumentDetails,
        library: &mut [LibraryEntry],
    ) -> Option<String> {
        let warning = details.annotation_warning.clone();
        self.local_marks = details.annotation_count;
        if let Some(title) = &details.title
            && let Some(entry) = library.iter_mut().find(|entry| entry.path == self.path)
        {
            entry.title.clone_from(title);
        }
        if let Some(size) = self.state.page_sizes.get_mut(0) {
            *size = Some(details.first_page_size);
        }
        self.layout_revision += 1;
        self.state.details = Some(details);
        warning
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::profile::LibraryEntry,
        document::{DocumentDetails, DocumentInfo, PageSize},
    };

    #[test]
    fn typed_details_ignore_presentation_labels_and_preserve_unrelated_books() {
        let path = PathBuf::from("book.epub");
        let mut tab = ReadingTab::new(7, path.clone(), None);
        tab.state.finish_open(
            7,
            Ok(DocumentInfo {
                path: path.clone(),
                page_count: 2,
            }),
        );
        let mut library = [
            LibraryEntry {
                path,
                title: "Old title".into(),
                opens: 1,
            },
            LibraryEntry {
                path: "other.pdf".into(),
                title: "Other book".into(),
                opens: 2,
            },
        ];
        let details = DocumentDetails {
            title: Some("Imported book title".into()),
            annotation_count: 4,
            annotation_warning: Some("Typed warning".into()),
            first_page_size: PageSize {
                width: 420.0,
                height: 595.0,
            },
            properties: vec![
                ("Title".into(), "Misleading display title".into()),
                ("Marcas locales".into(), "999".into()),
                ("Aviso de anotaciones".into(), "Misleading warning".into()),
            ],
            ..Default::default()
        };
        assert_eq!(
            tab.apply_details(details, &mut library).as_deref(),
            Some("Typed warning")
        );
        assert_eq!(tab.local_marks, 4);
        assert_eq!(library[0].title, "Imported book title");
        assert_eq!(library[1].title, "Other book");
        assert_eq!(
            tab.state.page_sizes[0],
            Some(PageSize {
                width: 420.0,
                height: 595.0
            })
        );
        assert_eq!(tab.layout_revision, 1);
        let update = DocumentDetails {
            properties: vec![("Título".into(), "Presentation only".into())],
            ..Default::default()
        };
        assert!(tab.apply_details(update, &mut library).is_none());
        assert_eq!(tab.local_marks, 0);
        assert_eq!(library[0].title, "Imported book title");
        assert_eq!(tab.layout_revision, 2);
        assert!(tab.state.details.unwrap().annotation_warning.is_none());
    }
}
