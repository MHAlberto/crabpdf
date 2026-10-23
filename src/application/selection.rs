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

#[derive(Default, Debug)]
pub struct SelectionState {
    range: Option<TextSelection>,
    pending: Option<(DocumentId, usize)>,
}
impl SelectionState {
    pub fn range(&self) -> Option<TextSelection> {
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
        self.range = Some(TextSelection { page, start, end });
    }
    pub fn extend(&mut self, page: usize, index: usize) {
        if let Some(range) = &mut self.range
            && range.page == page
        {
            range.end = index;
            self.pending = None;
        }
    }
    pub fn word(&mut self, page: usize, content: &PageContent, index: usize) {
        let Some(first) = content.glyphs.iter().position(|glyph| glyph.index == index) else {
            return;
        };
        let mut start = first;
        let mut end = first;
        let word = |index: usize| {
            content.glyphs[index]
                .text
                .chars()
                .any(char::is_alphanumeric)
        };
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
        self.range = content
            .glyphs
            .first()
            .zip(content.glyphs.last())
            .map(|(first, last)| TextSelection {
                page,
                start: first.index,
                end: last.index,
            });
    }
}
