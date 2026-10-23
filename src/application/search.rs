//! Search data belongs to the reader; focus and debounce scheduling stay in UI.
use crate::document::SearchHit;

#[derive(Default)]
pub struct SearchSession {
    pub query: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub generation: u64,
    pub hits: Vec<SearchHit>,
    pub index: usize,
    pub scanned: usize,
    pub total: usize,
    pub limited: bool,
}
