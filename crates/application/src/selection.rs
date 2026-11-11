//! Page/glyph selection rules. Pointer coordinates and text caching stay in adapters.
use crate::document::{DocumentId, PageContent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSelection {
    pub page: usize,
    pub start: usize,
    pub end: usize,
}
impl TextSelection {
    pub fn contains(self, index: usize) -> bool {
        index >= self.start.min(self.end) && index <= self.start.max(self.end)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TextPosition {
    page: usize,
    index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRange {
    anchor: TextPosition,
    focus: TextPosition,
}
impl TextRange {
    pub fn pages(self) -> std::ops::RangeInclusive<usize> {
        self.anchor.page.min(self.focus.page)..=self.anchor.page.max(self.focus.page)
    }

    pub fn on_page(self, page: usize) -> Option<TextSelection> {
        if !self.pages().contains(&page) {
            return None;
        }
        let first = self.anchor.min(self.focus);
        let last = self.anchor.max(self.focus);
        Some(TextSelection {
            page,
            start: if page == first.page { first.index } else { 0 },
            end: if page == last.page {
                last.index
            } else {
                usize::MAX
            },
        })
    }

    pub fn single_page(self) -> Option<TextSelection> {
        (self.anchor.page == self.focus.page).then_some(TextSelection {
            page: self.anchor.page,
            start: self.anchor.index,
            end: self.focus.index,
        })
    }
}

#[derive(Default, Debug)]
pub struct SelectionState {
    range: Option<TextRange>,
    pending: Option<(DocumentId, usize)>,
}
impl SelectionState {
    pub fn range(&self) -> Option<TextRange> {
        self.range
    }
    pub fn clear(&mut self) {
        self.range = None;
        self.pending = None;
    }
    pub fn cancel_pending(&mut self) {
        self.pending = None;
    }
    pub fn page_changed(&mut self, page: usize) {
        if self.pending.is_some_and(|(_, requested)| requested != page) {
            self.cancel_pending();
        }
    }
    pub fn select(&mut self, page: usize, start: usize, end: usize) {
        self.pending = None;
        self.range = Some(TextRange {
            anchor: TextPosition { page, index: start },
            focus: TextPosition { page, index: end },
        });
    }
    pub fn extend(&mut self, page: usize, index: usize) -> bool {
        if let Some(range) = &mut self.range {
            let focus = TextPosition { page, index };
            self.pending = None;
            if range.focus != focus {
                range.focus = focus;
                return true;
            }
        }
        false
    }
    pub fn word(&mut self, page: usize, content: &PageContent, index: usize) {
        let Some(first) = content.glyphs.iter().position(|glyph| glyph.index == index) else {
            return;
        };
        let mut start = first;
        let mut end = first;
        let word = |index: usize| {
            content.glyphs[index].text.chars().any(|c| {
                c.is_alphanumeric() || c == '_'
                    || matches!(c, '\u{0300}'..='\u{036f}' | '\u{1ab0}'..='\u{1aff}'
                        | '\u{1dc0}'..='\u{1dff}' | '\u{20d0}'..='\u{20ff}' | '\u{fe20}'..='\u{fe2f}')
            })
        };
        if !word(first) {
            self.select(page, index, index);
            return;
        }
        while start > 0 && word(start - 1) {
            start -= 1;
        }
        while end + 1 < content.glyphs.len() && word(end + 1) {
            end += 1;
        }
        self.select(page, content.glyphs[start].index, content.glyphs[end].index);
    }
    pub fn request_all(&mut self, id: DocumentId, page: usize) {
        self.pending = Some((id, page));
    }
    pub fn content_failed(&mut self, id: DocumentId, page: usize) {
        if self.pending == Some((id, page)) {
            self.pending = None;
        }
    }
    /// Consume only the exact request, still on its original page. Cached and
    /// asynchronous content use the same path; empty pages clear the range.
    pub fn accept_content(
        &mut self,
        id: DocumentId,
        page: usize,
        current_page: Option<usize>,
        content: &PageContent,
    ) {
        if self.pending != Some((id, page)) {
            return;
        }
        self.pending = None;
        if current_page != Some(page) {
            return;
        }
        match content.glyphs.first().zip(content.glyphs.last()) {
            Some((first, last)) => self.select(page, first.index, last.index),
            None => self.clear(),
        }
    }
}
