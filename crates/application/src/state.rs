use crate::document::{
    DocumentDetails, DocumentId, DocumentInfo, PageColors, PageSize, ReaderError, RenderKey, Zoom,
};
use crate::layout::LayoutMode;

#[derive(Debug)]
pub struct ReadingDocument {
    pub id: DocumentId,
    pub info: DocumentInfo,
    pub page: usize,
}

pub struct AppState {
    pub content_revision: u64,
    pub document: Option<ReadingDocument>,
    pub opening: Option<DocumentId>,
    pub zoom: Zoom,
    pub requested: Option<RenderKey>,
    pub rendering: bool,
    pub error: Option<ReaderError>,
    next_document_id: DocumentId,
    pub layout: LayoutMode,
    pub continuous: bool,
    pub manga: bool,
    pub rotation: u8,
    pub colors: PageColors,
    pub details: Option<DocumentDetails>,
    pub page_sizes: Vec<Option<PageSize>>,
    pub scroll_to_page: bool,
    history: Vec<(usize, Zoom)>,
    history_index: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            content_revision: 0,
            document: None,
            opening: None,
            zoom: Zoom::FitWidth,
            requested: None,
            rendering: false,
            error: None,
            next_document_id: 0,
            layout: LayoutMode::Single,
            continuous: true,
            manga: false,
            rotation: 0,
            colors: PageColors::Original,
            details: None,
            page_sizes: Vec::new(),
            scroll_to_page: false,
            history: Vec::new(),
            history_index: 0,
        }
    }
}

impl AppState {
    pub fn begin_open(&mut self) -> DocumentId {
        self.next_document_id += 1;
        let id = self.next_document_id;
        self.begin_open_with_id(id);
        id
    }

    pub fn begin_open_with_id(&mut self, id: DocumentId) {
        self.content_revision = 0;
        self.next_document_id = self.next_document_id.max(id);
        self.opening = Some(id);
        self.document = None;
        self.requested = None;
        self.rendering = false;
        self.error = None;
        self.zoom = Zoom::FitWidth;
        self.details = None;
        self.page_sizes.clear();
        self.history.clear();
        self.history_index = 0;
        self.rotation = 0;
    }

    pub fn finish_open(&mut self, id: DocumentId, result: Result<DocumentInfo, ReaderError>) {
        if self.opening != Some(id) {
            return;
        }
        self.opening = None;
        match result {
            Ok(info) => {
                self.page_sizes = vec![None; info.page_count];
                self.document = Some(ReadingDocument { id, info, page: 0 });
                self.history = vec![(0, self.zoom)];
                self.scroll_to_page = true;
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn navigate(&mut self, forward: bool) {
        if let Some(document) = &self.document {
            let spread = self.layout.spread(document.page, document.info.page_count);
            let page = if forward {
                spread
                    .last()
                    .copied()
                    .unwrap_or(document.page)
                    .saturating_add(1)
            } else {
                spread
                    .first()
                    .copied()
                    .unwrap_or(document.page)
                    .saturating_sub(1)
            };
            self.go_to(page, true);
        }
    }

    pub fn go_to(&mut self, page: usize, remember: bool) {
        let Some(document) = self.document.as_mut() else {
            return;
        };
        let page = page.min(document.info.page_count.saturating_sub(1));
        if document.page == page {
            self.scroll_to_page = true;
            return;
        }
        document.page = page;
        if remember {
            self.history.truncate(self.history_index + 1);
            self.history.push((page, self.zoom));
            if self.history.len() > 256 {
                self.history.remove(0);
            }
            self.history_index = self.history.len() - 1;
        }
        self.invalidate();
        self.scroll_to_page = true;
    }

    pub fn can_back(&self) -> bool {
        self.history_index > 0
    }
    pub fn can_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }
    pub fn history_navigate(&mut self, forward: bool) {
        if forward && self.can_forward() {
            self.history_index += 1;
        } else if !forward && self.can_back() {
            self.history_index -= 1;
        } else {
            return;
        }
        let (page, zoom) = self.history[self.history_index];
        self.set_zoom(zoom);
        self.go_to(page, false);
    }
    pub fn invalidate(&mut self) {
        self.requested = None;
        self.rendering = false;
        self.error = None;
    }
    pub fn rotate(&mut self, clockwise: bool) {
        self.rotation = (self.rotation + if clockwise { 1 } else { 3 }) % 4;
        self.invalidate();
        self.scroll_to_page = true;
    }

    pub fn change_zoom(&mut self, increase: bool, effective_percent: f32) {
        let current = match self.zoom {
            Zoom::Percent(value) => f32::from(value),
            _ => effective_percent,
        };
        const STEPS: [u16; 18] = [
            8, 12, 25, 50, 75, 100, 125, 150, 200, 300, 400, 600, 800, 1200, 1600, 2400, 3200, 6400,
        ];
        let value = if increase {
            STEPS
                .into_iter()
                .find(|p| f32::from(*p) > current + 0.01)
                .unwrap_or(6400)
        } else {
            STEPS
                .into_iter()
                .rev()
                .find(|p| f32::from(*p) < current - 0.01)
                .unwrap_or(8)
        };
        self.set_zoom(Zoom::Percent(value));
    }

    pub fn set_zoom(&mut self, zoom: Zoom) {
        let zoom = match zoom {
            Zoom::Percent(percent) => Zoom::Percent(percent.clamp(8, 6400)),
            other => other,
        };
        if self.zoom != zoom {
            self.zoom = zoom;
            self.invalidate();
            self.scroll_to_page = true;
        }
    }

    pub fn render_key(&self, width: f32, density: f32, texture_limit: usize) -> Option<RenderKey> {
        self.render_key_for(
            self.document.as_ref()?.page,
            width,
            700.0,
            density,
            texture_limit,
        )
    }

    pub fn render_key_for(
        &self,
        page: usize,
        width: f32,
        height: f32,
        density: f32,
        texture_limit: usize,
    ) -> Option<RenderKey> {
        self.document.as_ref().map(|document| RenderKey {
            document_id: document.id,
            revision: self.content_revision,
            page,
            zoom: self.zoom,
            // Width is irrelevant at a fixed zoom. Resizing then needs no re-render.
            viewport_width: if matches!(self.zoom, Zoom::FitWidth | Zoom::FitPage) {
                width.round().max(1.0) as u32
            } else {
                0
            },
            density_milli: (density * 1000.0).round().clamp(500.0, 8000.0) as u32,
            bitmap_side_limit: texture_limit.clamp(1, 8192) as u32,
            viewport_height: if matches!(self.zoom, Zoom::FitHeight | Zoom::FitPage) {
                height.round().max(1.0) as u32
            } else {
                0
            },
            rotation: self.rotation,
            colors: self.colors,
            region: None,
        })
    }

    pub fn accepts_render(&self, key: RenderKey) -> bool {
        self.requested == Some(key)
            && self
                .document
                .as_ref()
                .is_some_and(|d| d.id == key.document_id && d.page == key.page)
            && self.zoom == key.zoom
            && self.rotation == key.rotation
            && self.colors == key.colors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn info() -> DocumentInfo {
        DocumentInfo {
            path: PathBuf::from("test.pdf"),
            page_count: 3,
        }
    }

    #[test]
    fn late_open_and_render_cannot_replace_latest_request() {
        let mut state = AppState::default();
        let old = state.begin_open();
        let newest = state.begin_open();
        state.finish_open(old, Ok(info()));
        assert!(state.document.is_none());
        state.finish_open(newest, Ok(info()));
        let key = state.render_key(800.0, 1.0, 8192).unwrap();
        state.requested = Some(key);
        assert!(state.accepts_render(key));
        state.navigate(true);
        assert!(!state.accepts_render(key));
        state.requested = state.render_key(800.0, 1.0, 8192);
        state.set_zoom(Zoom::Percent(200));
        assert!(state.requested.is_none());
    }

    #[test]
    fn navigation_and_zoom_are_bounded() {
        let mut state = AppState::default();
        let id = state.begin_open();
        state.finish_open(id, Ok(info()));
        state.navigate(false);
        assert_eq!(state.document.as_ref().unwrap().page, 0);
        for _ in 0..10 {
            state.navigate(true);
        }
        assert_eq!(state.document.as_ref().unwrap().page, 2);
        state.change_zoom(true, 390.0);
        assert_eq!(state.zoom, Zoom::Percent(400));
        for _ in 0..20 {
            state.change_zoom(false, 100.0);
        }
        assert_eq!(state.zoom, Zoom::Percent(8));
    }
}
