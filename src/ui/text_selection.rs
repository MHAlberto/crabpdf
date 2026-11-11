use super::ReaderApp;
use crate::{
    application::selection::TextRange,
    document::{DocumentId, PageContent, ReaderError},
};
use eframe::egui;
use std::ops::RangeInclusive;

type TextTarget = (usize, (f32, f32));

pub(super) struct SelectionDrag {
    anchor: Option<TextTarget>,
    target: Option<TextTarget>,
    last_hit: Option<TextTarget>,
    released: bool,
}

impl SelectionDrag {
    pub(super) fn new(anchor: TextTarget) -> Self {
        Self {
            anchor: Some(anchor),
            target: None,
            last_hit: None,
            released: false,
        }
    }

    pub(super) fn needs(&self, page: usize) -> bool {
        [self.anchor, self.target]
            .into_iter()
            .flatten()
            .any(|(p, _)| p == page)
    }
}

const MAX_CLIPBOARD_BYTES: usize = 32 * 1024 * 1024;
const COPY_TOO_LARGE: &str = "La selección supera el tamaño de texto admitido por el portapapeles.";
const COPY_FAILED: &str = "No se pudo copiar la selección completa.";

pub(super) struct ClipboardCopy {
    id: DocumentId,
    range: TextRange,
    remaining: RangeInclusive<usize>,
    text: String,
    error: Option<&'static str>,
}

impl ClipboardCopy {
    fn new(id: DocumentId, range: TextRange) -> Self {
        Self {
            id,
            range,
            remaining: range.pages(),
            text: String::new(),
            error: None,
        }
    }

    fn next_page(&self) -> Option<usize> {
        self.remaining.clone().next()
    }

    pub(super) fn accept(
        &mut self,
        id: DocumentId,
        page: usize,
        content: Result<&PageContent, &ReaderError>,
    ) {
        if id != self.id || self.next_page() != Some(page) || self.error.is_some() {
            return;
        }
        self.remaining.next();
        let Ok(content) = content else {
            self.error = Some(COPY_FAILED);
            return;
        };
        let selection = self.range.on_page(page).expect("page is in the selection");
        let mut glyphs = content
            .glyphs
            .iter()
            .filter(|glyph| selection.contains(glyph.index) && !glyph.text.is_empty())
            .peekable();
        if glyphs.peek().is_none() {
            return;
        }
        if !self.text.is_empty() {
            let end = self.text.trim_end_matches(['\r', '\n']).len();
            self.text.truncate(end);
            self.text.push_str("\n\n");
        }
        for glyph in glyphs {
            if self.text.len().saturating_add(glyph.text.len()) > MAX_CLIPBOARD_BYTES {
                self.error = Some(COPY_TOO_LARGE);
                return;
            }
            self.text.push_str(&glyph.text);
        }
    }
}

impl ReaderApp {
    pub(super) fn update_selection_drag(
        &mut self,
        ctx: &egui::Context,
        primary_down: bool,
        target: Option<TextTarget>,
    ) -> bool {
        let Some(mut drag) = self.tabs[self.active].view.selection_drag.take() else {
            return false;
        };
        if !drag.released {
            drag.target = target.or(drag.target);
            drag.released = !primary_down;
        }
        let id = self.tabs[self.active].reader.id;
        if let Some((page, (x, y))) = drag.anchor {
            if let Some(content) = self.need_content(id, page) {
                let Some(index) = content.nearest_glyph(x, y) else {
                    return false;
                };
                self.tabs[self.active]
                    .reader
                    .selection
                    .select(page, index, index);
                drag.anchor = None;
                ctx.request_repaint();
            } else if !self.pending_content.contains(&(id, page)) {
                return false;
            }
        }
        if drag.anchor.is_none()
            && let Some(target @ (page, (x, y))) = drag.target
            && drag.last_hit != Some(target)
        {
            if let Some(content) = self.need_content(id, page) {
                let index = content.nearest_glyph(x, y).unwrap_or(0);
                if self.tabs[self.active].reader.selection.extend(page, index) {
                    ctx.request_repaint();
                }
                drag.last_hit = Some(target);
            } else if !self.pending_content.contains(&(id, page)) {
                return false;
            }
        }
        let held = !drag.released;
        if held || drag.anchor.is_some() || drag.last_hit != drag.target {
            self.tabs[self.active].view.selection_drag = Some(drag);
        } else if self.copy_after_drag == Some(id) {
            ctx.request_repaint();
        }
        if held {
            ctx.set_cursor_icon(egui::CursorIcon::Text);
        }
        held
    }

    pub(super) fn copy_selection(&mut self, ctx: &egui::Context) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        self.clipboard_copy = None;
        if tab.view.selection_drag.is_some() {
            self.copy_after_drag = Some(tab.reader.id);
            return;
        }
        let Some(range) = tab.reader.selection.range() else {
            return;
        };
        self.clipboard_copy = Some(ClipboardCopy::new(tab.reader.id, range));
        self.advance_copy(ctx);
        if self.clipboard_copy.is_some() {
            self.notice = Some("Preparando el texto seleccionado…".into());
        }
    }

    pub(super) fn advance_copy(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.copy_after_drag.take()
            && !self.home
            && let Some(tab) = self.tabs.get(self.active).filter(|tab| tab.reader.id == id)
        {
            if tab.view.selection_drag.is_some() {
                self.copy_after_drag = Some(id);
            } else if let Some(range) = tab.reader.selection.range() {
                self.clipboard_copy = Some(ClipboardCopy::new(id, range));
            }
        }
        let Some(mut copy) = self.clipboard_copy.take() else {
            return;
        };
        if self.home
            || !self.tabs.get(self.active).is_some_and(|tab| {
                tab.reader.id == copy.id
                    && tab.reader.selection.range() == Some(copy.range)
                    && tab
                        .reader
                        .state
                        .details
                        .as_ref()
                        .is_some_and(|details| details.can_copy)
            })
        {
            return;
        }
        // Read one uncached page at a time. Cached pages also yield between batches.
        for _ in 0..8 {
            if let Some(error) = copy.error {
                self.notice = Some(error.into());
                return;
            }
            let Some(page) = copy.next_page() else {
                if !copy.text.is_empty() {
                    ctx.copy_text(copy.text);
                    if copy.range.single_page().is_none() {
                        self.notice = Some("Selección copiada.".into());
                    }
                }
                return;
            };
            if let Some(content) = self.need_content(copy.id, page) {
                copy.accept(copy.id, page, Ok(&content));
            } else if self.pending_content.contains(&(copy.id, page)) {
                self.clipboard_copy = Some(copy);
                return;
            } else {
                self.notice = Some(COPY_FAILED.into());
                return;
            }
        }
        self.clipboard_copy = Some(copy);
        ctx.request_repaint();
    }

    pub(super) fn selected_text(&mut self) -> Option<String> {
        let tab = self.tabs.get(self.active)?;
        if !tab.reader.state.details.as_ref()?.can_copy {
            return None;
        }
        let mut copy = ClipboardCopy::new(tab.reader.id, tab.reader.selection.range()?);
        while let Some(page) = copy.next_page() {
            let content = self.content.get(&(copy.id, page))?;
            copy.accept(copy.id, page, Ok(&content));
            if copy.error.is_some() {
                return None;
            }
        }
        Some(copy.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{application::selection::SelectionState, document::Glyph};

    #[test]
    fn copies_a_backward_range_in_document_order_with_an_empty_page() {
        let mut selection = SelectionState::default();
        selection.select(4, 0, 0);
        selection.extend(0, 1);
        let mut copy = ClipboardCopy::new(7, selection.range().unwrap());
        let page = |a: &str, b: &str| PageContent {
            glyphs: [a, b]
                .into_iter()
                .enumerate()
                .map(|(index, text)| Glyph {
                    index,
                    text: text.into(),
                    bounds: None,
                })
                .collect(),
            ..Default::default()
        };
        copy.accept(8, 0, Ok(&page("wrong", "document")));
        copy.accept(7, 4, Ok(&page("early", "result")));
        copy.accept(7, 0, Ok(&page("omit", "café\r\n")));
        copy.accept(7, 1, Ok(&page("中", "間")));
        copy.accept(7, 2, Ok(&PageContent::default()));
        copy.accept(7, 3, Ok(&page("página", " cuatro")));
        copy.accept(7, 4, Ok(&page("fin", "omit")));
        assert_eq!(copy.next_page(), None);
        assert_eq!(copy.text, "café\n\n中間\n\npágina cuatro\n\nfin");
    }
}
