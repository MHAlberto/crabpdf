//! Search rules without a window, clock, worker or native engine.
use crate::document::{Bounds, DocumentId, MAX_SEARCH_HITS, SearchHit, SearchRequest};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchIdentity {
    pub document_id: DocumentId,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchDestination {
    pub page: usize,
    pub bounds: Option<Bounds>,
}
#[derive(Default, Debug, PartialEq)]
pub struct SearchUpdate {
    pub destination: Option<SearchDestination>,
    pub stop: Option<SearchIdentity>,
}

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
    pub error: Option<crate::document::ReaderError>,
    document_id: Option<DocumentId>,
}
impl SearchSession {
    /// Invalidate results immediately; the adapter supplies the next generation.
    /// Stop only the previous identity, including a request still queued in the worker.
    pub fn restart(
        &mut self,
        document_id: DocumentId,
        generation: u64,
        total: usize,
    ) -> Option<SearchIdentity> {
        let previous = self.document_id.take().map(|document_id| SearchIdentity {
            document_id,
            generation: self.generation,
        });
        self.generation = generation;
        self.hits.clear();
        self.index = 0;
        self.scanned = 0;
        self.total = total;
        self.limited = false;
        self.error = None;
        if !self.query.is_empty() && total > 0 {
            self.document_id = Some(document_id);
        }
        previous
    }
    pub fn is_active(&self) -> bool {
        self.document_id.is_some()
    }
    pub fn is_scanning(&self) -> bool {
        self.is_active() && self.scanned < self.total
    }
    /// Accept only a failure from the current, unfinished search.
    pub fn fail(
        &mut self,
        id: DocumentId,
        generation: u64,
        error: crate::document::ReaderError,
    ) -> bool {
        if self.document_id != Some(id) || self.generation != generation || !self.is_scanning() {
            return false;
        }
        self.document_id = None;
        self.error = Some(error);
        true
    }
    pub fn dismiss_error(&mut self) {
        self.error = None;
    }
    /// A single request snapshot, created after the adapter's debounce expires.
    pub fn request(&self) -> Option<SearchRequest> {
        Some(SearchRequest {
            document_id: self.document_id?,
            generation: self.generation,
            query: self.query.clone(),
            match_case: self.match_case,
            whole_word: self.whole_word,
        })
    }
    pub fn accept(
        &mut self,
        id: DocumentId,
        generation: u64,
        hits: Vec<SearchHit>,
        scanned: usize,
        total: usize,
    ) -> SearchUpdate {
        if self.document_id != Some(id)
            || self.generation != generation
            || self.limited
            || scanned <= self.scanned
            || scanned > total
            || total != self.total
        {
            return SearchUpdate::default();
        }
        let first = self.hits.is_empty();
        let remaining = MAX_SEARCH_HITS.saturating_sub(self.hits.len());
        self.hits.extend(hits.into_iter().take(remaining));
        self.scanned = scanned;
        let destination = if first && !self.hits.is_empty() {
            self.index = 0;
            self.destination()
        } else {
            None
        };
        let stop = if self.hits.len() >= MAX_SEARCH_HITS {
            self.limited = true;
            self.document_id = None;
            Some(SearchIdentity {
                document_id: id,
                generation,
            })
        } else {
            None
        };
        SearchUpdate { destination, stop }
    }
    pub fn current(&self) -> Option<&SearchHit> {
        self.hits.get(self.index)
    }
    fn destination(&self) -> Option<SearchDestination> {
        let hit = self.current()?;
        Some(SearchDestination {
            page: hit.page,
            bounds: hit.bounds.first().copied(),
        })
    }
    pub fn step(&mut self, forward: bool) -> Option<SearchDestination> {
        let count = self.hits.len();
        if count == 0 {
            return None;
        }
        let current = self.index % count;
        self.index = if forward {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        };
        self.destination()
    }
}
