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
