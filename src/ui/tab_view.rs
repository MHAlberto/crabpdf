//! Visual state paired with exactly one logical ReadingTab.
use super::motion;
use crate::{
    application::{profile::StoredDocument, tabs::ReadingTab},
    document::{DocumentId, Zoom},
    layout::{LayoutSpec, PageLayout, ReadingAnchor},
    reading_position::ReadingPosition,
};
use eframe::egui::{self, Vec2};
use std::{path::PathBuf, sync::Arc, time::Instant};

#[derive(Default)]
pub(super) struct SearchViewState {
    pub(super) open: bool,
    pub(super) focus: bool,
    pub(super) due: Option<Instant>,
}
#[derive(Default)]
pub(super) struct TabViewState {
    pub(super) search: SearchViewState,
    pub(super) scroll: Vec2,
    pub(super) key_scroll: super::key_scroll::KeyScroll,
    pub(super) viewport: Vec2,
    pub(super) viewport_rect: Option<egui::Rect>,
    pub(super) zoom_anchor: Option<ReadingAnchor>,
    pub(super) scroll_bottom: bool,
    pub(super) scroll_motion: Option<motion::ScrollMotion>,
    pub(super) displayed_page: Option<usize>,
    pub(super) navigation_wait: Option<(usize, f64)>,
    pub(super) reveal: Option<(usize, crate::document::Bounds)>,
    pub(super) password: String,
    pub(super) selection_geometry: super::selection_geometry::SelectionGeometry,
    pub(super) view_layout: Option<(LayoutSpec, Arc<PageLayout>)>,
}

/// One owner for logical data and one for presentation; tab order moves both.
pub(super) struct Tab {
    pub(super) reader: ReadingTab,
    pub(super) view: TabViewState,
}
impl Tab {
    pub(super) fn new(id: DocumentId, path: PathBuf, restore: Option<StoredDocument>) -> Self {
        Self {
            reader: ReadingTab::new(id, path, restore),
            view: TabViewState::default(),
        }
    }
    pub(super) fn go_to(&mut self, page: usize, remember: bool) {
        self.reader.go_to(page, remember);
        self.view.key_scroll.cancel();
        self.view.scroll_motion = None;
        self.view.navigation_wait = None;
    }
    pub(super) fn reveal_search(
        &mut self,
        destination: crate::application::search::SearchDestination,
    ) {
        self.go_to(destination.page, true);
        self.view.reveal = destination.bounds.map(|bounds| (destination.page, bounds));
    }
    /// Fit zoom describes page geometry, independent of annotation textures.
    pub(super) fn effective_zoom(&self) -> Option<f32> {
        if let Zoom::Percent(percent) = self.reader.state.zoom {
            return Some(f32::from(percent));
        }
        let document = self.reader.state.document.as_ref()?;
        let mut spec = self.view.view_layout.as_ref()?.0;
        spec.mode = self.reader.state.layout;
        let (width, height) = spec.cell_size();
        let size = self
            .reader
            .state
            .page_sizes
            .get(document.page)
            .copied()
            .flatten()
            .or_else(|| {
                self.reader
                    .state
                    .details
                    .as_ref()
                    .map(|details| details.first_page_size)
            })?;
        let base = if self.reader.state.rotation.is_multiple_of(2) {
            size.width
        } else {
            size.height
        };
        if !base.is_finite() || base <= 0.0 {
            return None;
        }
        let logical = crate::layout::logical_page_size(
            size,
            self.reader.state.rotation,
            self.reader.state.zoom,
            f64::from(width),
            f64::from(height),
        )
        .0;
        Some((logical / (f64::from(base) * 96.0 / 72.0) * 100.0) as f32)
    }
    pub(super) fn supports_password(&self) -> bool {
        matches!(
            crate::formats::Format::detect(&self.reader.path),
            Ok(crate::formats::Format::Pdf)
        )
    }
    pub(super) fn stored(&self) -> Option<StoredDocument> {
        let captured = if self.reader.pending_position.is_some() {
            None
        } else {
            self.view.view_layout.as_ref().and_then(|(_, layout)| {
                ReadingPosition::capture(
                    layout,
                    (self.view.scroll.x, self.view.scroll.y),
                    (self.view.viewport.x, self.view.viewport.y),
                )
                .filter(|position| {
                    self.reader
                        .state
                        .page_sizes
                        .get(position.page())
                        .is_some_and(Option::is_some)
                })
            })
        };
        self.reader.stored(captured)
    }
    pub(super) fn anchor(
        &self,
        pointer: Option<egui::Pos2>,
    ) -> Option<crate::layout::ReadingAnchor> {
        let (_, layout) = self.view.view_layout.as_ref()?;
        let screen = pointer
            .zip(self.view.viewport_rect)
            .filter(|(pointer, rect)| rect.contains(*pointer))
            .map_or(self.view.viewport * 0.5, |(pointer, rect)| {
                pointer - rect.min
            });
        let point = self.view.scroll + screen;
        let placement = layout
            .placements
            .iter()
            .find(|p| {
                point.x >= p.left
                    && point.x <= p.left + p.size.width
                    && point.y >= p.top
                    && point.y <= p.top + p.size.height
            })
            .or_else(|| {
                layout.placements.iter().find(|p| {
                    self.reader
                        .state
                        .document
                        .as_ref()
                        .is_some_and(|document| p.page == document.page)
                })
            })?;
        Some(crate::layout::ReadingAnchor {
            page: placement.page,
            x: ((point.x - placement.left) / placement.size.width).clamp(0.0, 1.0),
            y: ((point.y - placement.top) / placement.size.height).clamp(0.0, 1.0),
            screen_x: screen.x,
            screen_y: screen.y,
        })
    }
    pub(super) fn zoom_at(&mut self, zoom: Zoom, pointer: Option<egui::Pos2>) {
        self.reader.pending_position = None;
        self.view.key_scroll.cancel();
        self.view.scroll_motion = None;
        self.view.navigation_wait = None;
        if self.reader.state.zoom == zoom {
            return;
        }
        let anchor = self.anchor(pointer);
        self.reader.state.set_zoom(zoom);
        self.view.zoom_anchor = anchor;
        if self.view.zoom_anchor.is_some() {
            self.reader.state.scroll_to_page = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{DocumentInfo, PageSize},
        layout::LayoutMode,
    };
    #[test]
    fn fit_zoom_is_stable_while_marking_renders_and_rotated_views_are_pending() {
        let mut tab = Tab::new(1, "book.pdf".into(), None);
        tab.reader.finish_open(
            1,
            Ok(DocumentInfo {
                path: "book.pdf".into(),
                page_count: 1,
            }),
            true,
        );
        let size = PageSize {
            width: 600.0,
            height: 800.0,
        };
        tab.reader.state.page_sizes[0] = Some(size);
        for rotation in 0..4 {
            for zoom in [Zoom::FitWidth, Zoom::FitHeight, Zoom::FitPage] {
                tab.reader.state.zoom = zoom;
                tab.reader.state.rotation = rotation;
                let spec = LayoutSpec {
                    width: 1080,
                    height: 780,
                    zoom,
                    mode: LayoutMode::Single,
                    continuous: true,
                    current: 0,
                    rotation,
                    manga: false,
                    compact: false,
                    revision: 0,
                };
                tab.view.view_layout =
                    Some((spec, Arc::new(PageLayout::build(&[Some(size)], size, spec))));
                let expected = tab.effective_zoom().unwrap();
                let (w, h) = spec.cell_size();
                let key = tab.reader.state.render_key_for(0, w, h, 1.0, 8192).unwrap();
                tab.reader.state.requested = Some(key);
                assert_eq!(tab.effective_zoom(), Some(expected));
                tab.reader.state.content_revision += 1;
                tab.reader.state.requested = None;
                tab.reader.state.rendering = true;
                assert_eq!(tab.effective_zoom(), Some(expected));
                assert!((expected - 100.0).abs() > 0.1);
            }
        }
        tab.reader.state.zoom = Zoom::Percent(175);
        assert_eq!(tab.effective_zoom(), Some(175.0));
    }
}
