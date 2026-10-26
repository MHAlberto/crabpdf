//! Contract checks need neither PDFium nor a window.
use crabpdf::{
    application::{protocol::WorkerEvent, search::SearchSession, tabs::ReadingTab},
    document::{DocumentDetails, DocumentInfo, ErrorKind, ReaderError, SearchHit, SearchRequest},
};
use std::sync::Arc;

fn request(id: u64, generation: u64) -> SearchRequest {
    SearchRequest {
        document_id: id,
        generation,
        query: "reader".into(),
        match_case: false,
        whole_word: true,
    }
}
fn session() -> SearchSession {
    let mut session = SearchSession::default();
    session.query = "reader".into();
    session.restart(7, 10, 3);
    session
}
fn failure(message: &str) -> ReaderError {
    ReaderError::new(ErrorKind::Render, message).with_details("native search failure")
}
fn hit(page: usize) -> SearchHit {
    SearchHit {
        page,
        start: 2,
        count: 3,
        bounds: vec![],
    }
}

#[test]
fn public_facades_share_the_same_event_and_cover_allocation() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<WorkerEvent>();
    let cover: Arc<crabpdf::library::Cover> = Arc::new(crabpdf::document::Cover {
        width: 1,
        height: 1,
        rgba: vec![20, 30, 40, 255],
    });
    let original = Arc::clone(&cover);
    let event: crabpdf::worker::WorkerEvent = WorkerEvent::Cover("book.epub".into(), Ok(cover));
    let WorkerEvent::Cover(path, Ok(received)) = event else {
        panic!("Expected cover");
    };
    assert_eq!(path.to_str(), Some("book.epub"));
    assert!(Arc::ptr_eq(&received, &original));
    assert_eq!(received.rgba, [20, 30, 40, 255]);
}

#[test]
fn search_success_and_failure_keep_identity_and_do_not_masquerade_as_metadata() {
    let request = request(5, 42);
    let event = WorkerEvent::search_result(&request, 2, 3, Ok(vec![hit(1)]));
    let WorkerEvent::Searched {
        id,
        generation,
        hits,
        scanned,
        total,
    } = event
    else {
        panic!("Expected successful batch");
    };
    assert_eq!((id, generation, scanned, total), (5, 42, 2, 3));
    assert_eq!(hits[0].page, 1);
    let event = WorkerEvent::search_result(&request, 3, 3, Err(failure("Failed page")));
    let WorkerEvent::SearchFailed {
        id,
        generation,
        error,
    } = event
    else {
        panic!("Search errors must carry their generation");
    };
    assert_eq!((id, generation), (5, 42));
    assert_eq!(error.kind, ErrorKind::Render);
    assert_eq!(error.message, "Failed page");
    assert_eq!(error.details.as_deref(), Some("native search failure"));
}

#[test]
fn stale_or_foreign_search_failures_cannot_interrupt_a_new_query() {
    let mut session = session();
    for (id, generation) in [(8, 10), (7, 9)] {
        assert!(!session.fail(id, generation, failure("Old failure")));
        assert!(session.error.is_none() && session.is_scanning());
    }
    session.query = "new".into();
    session.restart(7, 11, 3);
    assert!(!session.fail(7, 10, failure("Late failure")));
    assert!(session.error.is_none() && session.is_scanning());
    session.query.clear();
    session.restart(7, 12, 3);
    assert!(!session.fail(7, 11, failure("Cancelled failure")));
    assert!(session.error.is_none() && !session.is_scanning());
}

#[test]
fn current_search_failure_preserves_partial_matches_document_errors_and_metadata() {
    let mut tab = ReadingTab::new(7, "book.pdf".into(), None);
    tab.finish_open(
        7,
        Ok(DocumentInfo {
            path: "book.pdf".into(),
            page_count: 3,
        }),
        true,
    );
    tab.state.details = Some(DocumentDetails {
        title: Some("Book title".into()),
        ..Default::default()
    });
    tab.state.error = Some(failure("Independent render error"));
    tab.search.query = "reader".into();
    tab.restart_search(10);
    tab.search.accept(7, 10, vec![hit(0)], 1, 3);
    let WorkerEvent::SearchFailed {
        id,
        generation,
        error,
    } = WorkerEvent::search_result(&request(7, 10), 2, 3, Err(failure("Search failed")))
    else {
        panic!("Expected search failure");
    };
    assert!(tab.search.fail(id, generation, error));
    assert_eq!(tab.search.error.as_ref().unwrap().message, "Search failed");
    assert_eq!(
        tab.state.error.as_ref().unwrap().message,
        "Independent render error"
    );
    assert_eq!(
        tab.state.details.as_ref().unwrap().title.as_deref(),
        Some("Book title")
    );
    assert_eq!(tab.state.document.as_ref().unwrap().page, 0);
    assert!(!tab.password_needed);
    assert!(!tab.search.is_scanning() && tab.search.request().is_none());
    assert_eq!(
        (tab.search.scanned, tab.search.total, tab.search.hits.len()),
        (1, 3, 1)
    );
    assert_eq!(tab.search.step(true).unwrap().page, 0);
    assert!(!tab.search.fail(7, 10, failure("Duplicate failure")));
    assert_eq!(tab.search.error.as_ref().unwrap().message, "Search failed");
}

#[test]
fn completed_and_limited_searches_ignore_late_failures() {
    let mut completed = session();
    completed.accept(7, 10, vec![hit(0)], 3, 3);
    assert!(!completed.is_scanning());
    assert!(!completed.fail(7, 10, failure("After completion")));
    assert!(completed.error.is_none());
    let mut limited = session();
    limited.accept(
        7,
        10,
        (0..crabpdf::document::MAX_SEARCH_HITS)
            .map(|_| hit(0))
            .collect(),
        1,
        3,
    );
    assert!(limited.limited && !limited.is_scanning());
    assert!(!limited.fail(7, 10, failure("After limit")));
    assert!(limited.error.is_none());
}

#[test]
fn dismissing_and_retrying_search_does_not_revive_old_failures() {
    let mut session = session();
    assert!(session.fail(7, 10, failure("Current failure")));
    session.dismiss_error();
    assert!(session.error.is_none() && !session.is_scanning());
    assert!(session.request().is_none());
    session.restart(7, 11, 3);
    assert!(session.is_scanning() && session.error.is_none());
    assert!(!session.fail(7, 10, failure("Old retry")));
    assert!(
        session
            .accept(7, 11, vec![hit(0)], 1, 3)
            .destination
            .is_some()
    );
    assert!(session.fail(7, 11, failure("Second failure")));
    session.restart(7, 12, 3);
    assert!(session.error.is_none() && session.is_scanning());
}
