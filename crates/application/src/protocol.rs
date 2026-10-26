//! Results exchanged with background adapters. No native handles, I/O or GUI types.
use crate::document::{
    AnnotationState, Cover, DocumentDetails, DocumentId, DocumentInfo, PageContent, PageRaster,
    ReaderError, RenderKey, SearchHit, SearchRequest,
};
use std::{path::PathBuf, sync::Arc};

pub enum WorkerEvent {
    Changed(DocumentId, AnnotationState),
    Cover(PathBuf, Result<Arc<Cover>, ReaderError>),
    Opened(DocumentId, Result<DocumentInfo, ReaderError>),
    Details(DocumentId, Result<DocumentDetails, ReaderError>),
    Rendered(RenderKey, Result<Arc<PageRaster>, ReaderError>),
    Content(DocumentId, usize, Result<Arc<PageContent>, ReaderError>),
    Searched {
        id: DocumentId,
        generation: u64,
        hits: Vec<SearchHit>,
        scanned: usize,
        total: usize,
    },
    SearchFailed {
        id: DocumentId,
        generation: u64,
        error: ReaderError,
    },
    Saved(DocumentId, PathBuf, Result<(), ReaderError>),
    Marked(DocumentId, Result<(usize, usize), ReaderError>),
    Unavailable(ReaderError),
}
impl WorkerEvent {
    /// The same identity accompanies successful batches and native failures.
    pub fn search_result(
        request: &SearchRequest,
        scanned: usize,
        total: usize,
        result: Result<Vec<SearchHit>, ReaderError>,
    ) -> Self {
        match result {
            Ok(hits) => Self::Searched {
                id: request.document_id,
                generation: request.generation,
                hits,
                scanned,
                total,
            },
            Err(error) => Self::SearchFailed {
                id: request.document_id,
                generation: request.generation,
                error,
            },
        }
    }
}
