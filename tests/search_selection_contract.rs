//! Search and selection contracts run without a GUI or PDFium.
use crabpdf::{
    application::{
        search::{SearchDestination, SearchIdentity, SearchSession, SearchUpdate},
        selection::SelectionState,
        tabs::ReadingTab,
    },
    document::{Bounds, DocumentInfo, Glyph, MAX_SEARCH_HITS, PageContent, SearchHit, Zoom},
};

fn hit(page: usize, start: usize) -> SearchHit {
    SearchHit {
        page,
        start,
        count: 2,
        bounds: vec![Bounds {
            left: 0.2,
            top: 0.4,
            right: 0.6,
            bottom: 0.5,
        }],
    }
}
fn search(id: u64, generation: u64) -> SearchSession {
    let mut search = SearchSession::default();
    search.query = "reader".into();
    search.restart(id, generation, 8);
    search
}
fn content() -> PageContent {
    PageContent {
        // Real glyph indices need not be contiguous; Unicode text is preserved.
        glyphs: ["U", "n", "o", " ", "é", "x", "i", "t", "o", "-", "二"]
            .iter()
            .enumerate()
            .map(|(i, text)| Glyph {
                index: i * 3,
                text: (*text).into(),
                bounds: None,
            })
            .collect(),
        ..Default::default()
    }
}
fn tab(id: u64) -> ReadingTab {
    let mut tab = ReadingTab::new(id, "same.pdf".into(), None);
    tab.finish_open(
        id,
        Ok(DocumentInfo {
            path: "same.pdf".into(),
            page_count: 8,
        }),
        true,
    );
    tab
}

#[test]
fn query_restart_invalidates_previous_results_and_snapshots_current_options() {
    let mut search = search(4, 20);
    search.accept(4, 20, vec![hit(0, 1)], 1, 8);
    search.query = "new".into();
    search.match_case = true;
    search.whole_word = true;
    assert_eq!(
        search.restart(4, 21, 8),
        Some(SearchIdentity {
            document_id: 4,
            generation: 20
        })
    );
    assert!(search.hits.is_empty() && search.index == 0 && search.scanned == 0);
    let request = search.request().unwrap();
    assert_eq!((request.document_id, request.generation), (4, 21));
    assert_eq!(request.query, "new");
    assert!(request.match_case && request.whole_word);
    assert_eq!(
        search.accept(4, 20, vec![hit(1, 2)], 2, 8),
        SearchUpdate::default()
    );
    assert!(search.hits.is_empty());
}

#[test]
fn empty_query_cancels_only_its_previous_identity_and_has_no_delayed_request() {
    let mut first = search(1, 2);
    let mut other = search(2, 3);
    first.query.clear();
    assert_eq!(
        first.restart(1, 4, 8),
        Some(SearchIdentity {
            document_id: 1,
            generation: 2
        })
    );
    assert!(!first.is_active() && first.request().is_none());
    assert_eq!(
        first.accept(1, 4, vec![hit(1, 1)], 1, 8),
        SearchUpdate::default()
    );
    assert!(
        other
            .accept(2, 3, vec![hit(2, 2)], 1, 8)
            .destination
            .is_some()
    );
    assert!(first.restart(1, 5, 8).is_none());
    first.query = "reader".into();
    first.restart(1, 6, 0);
    assert!(first.request().is_none());
}

#[test]
fn first_match_reveals_once_and_progress_rejects_duplicated_or_out_of_order_batches() {
    let mut search = search(7, 9);
    assert_eq!(search.accept(7, 9, vec![], 1, 8), SearchUpdate::default());
    assert_eq!(search.scanned, 1);
    let first = search.accept(7, 9, vec![hit(1, 5)], 2, 8);
    assert_eq!(
        first.destination,
        Some(SearchDestination {
            page: 1,
            bounds: hit(1, 5).bounds.first().copied()
        })
    );
    assert!(first.stop.is_none());
    for (id, generation, scanned, total) in [
        (8, 9, 3, 8),
        (7, 10, 3, 8),
        (7, 9, 2, 8),
        (7, 9, 1, 8),
        (7, 9, 9, 8),
        (7, 9, 3, 9),
    ] {
        assert_eq!(
            search.accept(id, generation, vec![hit(0, 0)], scanned, total),
            SearchUpdate::default()
        );
        assert_eq!(search.hits.len(), 1);
        assert_eq!(search.scanned, 2);
    }
    assert!(
        search
            .accept(7, 9, vec![hit(2, 8)], 3, 8)
            .destination
            .is_none()
    );
    assert_eq!(search.hits.len(), 2);
    assert_eq!(search.current().unwrap().page, 1);
    search.accept(7, 9, vec![], 8, 8);
    assert_eq!(search.scanned, search.total);
}

#[test]
fn result_limit_is_bounded_and_requests_stop_once_even_if_more_batches_arrive() {
    let mut search = search(3, 4);
    let update = search.accept(
        3,
        4,
        (0..MAX_SEARCH_HITS + 7).map(|i| hit(0, i)).collect(),
        1,
        8,
    );
    assert_eq!(
        update.stop,
        Some(SearchIdentity {
            document_id: 3,
            generation: 4
        })
    );
    assert!(update.destination.is_some());
    assert!(search.limited && search.request().is_none());
    assert_eq!(search.hits.len(), MAX_SEARCH_HITS);
    assert_eq!(
        search.accept(3, 4, vec![hit(1, 0)], 2, 8),
        SearchUpdate::default()
    );
    assert_eq!(search.hits.len(), MAX_SEARCH_HITS);
    assert_eq!(search.scanned, 1);
    search.restart(3, 5, 8);
    assert!(!search.limited && search.hits.is_empty());
    assert!(search.request().is_some());
}

#[test]
fn next_previous_wrap_and_do_not_require_match_rectangles() {
    let mut search = search(3, 4);
    assert!(search.step(true).is_none());
    let mut no_bounds = hit(2, 9);
    no_bounds.bounds.clear();
    search.accept(3, 4, vec![hit(1, 0), no_bounds, hit(3, 12)], 8, 8);
    assert_eq!(search.step(false).unwrap().page, 3);
    assert_eq!(search.step(true).unwrap().page, 1);
    assert_eq!(
        search.step(true),
        Some(SearchDestination {
            page: 2,
            bounds: None
        })
    );
    assert_eq!(search.current().unwrap().start, 9);
}

#[test]
fn duplicate_tabs_with_the_same_generation_cannot_accept_each_others_batches() {
    let mut first = tab(1);
    let mut second = tab(2);
    first.search.query = "Uno".into();
    second.search.query = "éxito".into();
    first.restart_search(10);
    second.restart_search(10);
    assert_eq!(
        first.search.accept(2, 10, vec![hit(1, 0)], 1, 8),
        SearchUpdate::default()
    );
    let effect = second.search.accept(2, 10, vec![hit(2, 3)], 1, 8);
    second.go_to(effect.destination.unwrap().page, true);
    assert_eq!(first.state.document.as_ref().unwrap().page, 0);
    assert!(first.search.hits.is_empty());
    assert_eq!(second.state.document.as_ref().unwrap().page, 2);
}

#[test]
fn word_selection_preserves_unicode_sparse_indices_and_original_boundaries() {
    let mut selection = SelectionState::default();
    let content = content();
    selection.word(4, &content, 18);
    let range = selection.range().unwrap();
    assert_eq!((range.page, range.start, range.end), (4, 12, 24));
    assert_eq!(content.text(range.start, range.end), "éxito");
    selection.word(4, &content, 30);
    let range = selection.range().unwrap();
    assert_eq!(content.text(range.start, range.end), "二");
    selection.word(4, &content, 999);
    assert_eq!(selection.range(), Some(range));
    selection.word(4, &PageContent::default(), 0);
    assert_eq!(selection.range(), Some(range));
}

#[test]
fn backwards_drag_keeps_its_anchor_and_ignores_other_page_motion() {
    let mut selection = SelectionState::default();
    selection.select(2, 24, 24);
    selection.extend(3, 0);
    assert_eq!(selection.range().unwrap().end, 24);
    selection.extend(2, 12);
    let range = selection.range().unwrap();
    assert_eq!((range.start, range.end), (24, 12));
    assert!(range.contains(12) && range.contains(24) && !range.contains(9));
    assert_eq!(content().text(range.start, range.end), "éxito");
}

#[test]
fn select_all_accepts_only_the_requested_document_and_page() {
    let mut selection = SelectionState::default();
    let content = content();
    selection.request_all(7, 2);
    selection.accept_content(8, 2, Some(2), &content);
    selection.accept_content(7, 3, Some(3), &content);
    assert!(selection.range().is_none());
    selection.accept_content(7, 2, Some(2), &content);
    let selected = selection.range().unwrap();
    assert_eq!(content.text(selected.start, selected.end), "Uno éxito-二");
    selection.accept_content(7, 2, Some(2), &PageContent::default());
    assert_eq!(selection.range(), Some(selected));
}

#[test]
fn escape_manual_selection_and_navigation_cancel_pending_select_all() {
    for action in 0..4 {
        let mut selection = SelectionState::default();
        selection.request_all(1, 0);
        match action {
            0 => selection.clear(),           // Escape
            1 => selection.select(0, 12, 24), // a newer pointer selection
            2 => {
                selection.page_changed(1);
                selection.page_changed(0);
            } // leave and return
            _ => selection.cancel_pending(),  // explicit navigation before layout changes
        }
        let before = selection.range();
        selection.accept_content(1, 0, Some(0), &content());
        assert_eq!(selection.range(), before);
    }
}

#[test]
fn late_selection_content_after_a_page_change_cannot_select_a_new_page() {
    let mut selection = SelectionState::default();
    selection.request_all(1, 0);
    selection.accept_content(1, 0, Some(1), &content());
    selection.accept_content(1, 0, Some(0), &content());
    assert!(selection.range().is_none());
    selection.request_all(1, 1);
    selection.request_all(1, 2);
    selection.accept_content(1, 1, Some(1), &content());
    assert!(selection.range().is_none());
    selection.accept_content(1, 2, Some(2), &content());
    assert_eq!(selection.range().unwrap().page, 2);
}

#[test]
fn empty_or_failed_content_consumes_only_its_own_pending_selection() {
    let mut selection = SelectionState::default();
    selection.select(0, 0, 6);
    selection.request_all(1, 0);
    selection.content_failed(2, 0);
    selection.accept_content(1, 0, Some(0), &PageContent::default());
    assert!(selection.range().is_none());
    selection.request_all(1, 0);
    selection.content_failed(1, 0);
    selection.accept_content(1, 0, Some(0), &content());
    assert!(selection.range().is_none());
}

#[test]
fn reading_tab_selection_uses_one_path_for_cache_and_async_content_and_cancels_on_go_to() {
    let mut tab = tab(8);
    tab.state.zoom = Zoom::Percent(150);
    assert_eq!(tab.request_select_all(), Some(0));
    tab.accept_content(8, 0, &content()); // cached
    assert_eq!(tab.selection.range().unwrap().page, 0);
    tab.selection.clear();
    tab.request_select_all();
    tab.go_to(2, true);
    tab.accept_content(8, 0, &content()); // late
    assert!(tab.selection.range().is_none());
    assert_eq!(tab.request_select_all(), Some(2));
    tab.accept_content(9, 2, &content());
    assert!(tab.selection.range().is_none());
    tab.accept_content(8, 2, &content()); // async
    assert_eq!(tab.selection.range().unwrap().page, 2);
    assert_eq!(tab.state.zoom, Zoom::Percent(150));
}
