//! Compatibility and use-case checks without a window or a PDFium instance.
use crabpdf::{
    application::tabs::{InitialView, OpenOutcome, ReadingTab},
    config::{LibraryEntry, Preferences, StoredDocument},
    document::{DocumentInfo, ErrorKind, PageColors, ReaderError, Zoom},
    layout::LayoutMode,
    reading_position::ReadingPosition,
};

fn position(schema: u8, page: usize) -> ReadingPosition {
    serde_json::from_value(serde_json::json!({
        "schema": schema, "page": page,
        "page_x": 500_000, "page_y": 750_000,
        "viewport_x": 500_000, "viewport_y": 500_000
    }))
    .unwrap()
}
fn saved() -> StoredDocument {
    StoredDocument {
        path: "same-book.pdf".into(),
        page: 3,
        zoom: Zoom::Percent(135),
        layout: LayoutMode::Book,
        continuous: false,
        manga: true,
        rotation: 7,
        colors: PageColors::Grayscale,
        position: Some(position(1, 5)),
    }
}
fn loaded(tab: &mut ReadingTab) -> OpenOutcome {
    tab.finish_open(
        tab.id,
        Ok(DocumentInfo {
            path: tab.path.clone(),
            page_count: 10,
        }),
        true,
    )
}
fn page(tab: &ReadingTab) -> usize {
    tab.state.document.as_ref().unwrap().page
}

#[test]
fn restore_keeps_the_normalized_anchor_until_the_view_captures_a_new_one() {
    let saved = saved();
    let mut tab = ReadingTab::new(12, saved.path.clone(), Some(saved.clone()));
    assert!(tab.stored(None).is_none());
    assert_eq!(
        loaded(&mut tab),
        OpenOutcome::Ready {
            initial_search: false
        }
    );
    assert_eq!(page(&tab), 5);
    assert!(tab.restore.is_none());
    assert_eq!(tab.pending_position, saved.position);
    let mut expected = saved;
    expected.page = 5;
    expected.rotation = 3;
    assert_eq!(tab.stored(Some(position(1, 2))), Some(expected.clone()));
    // Once the view consumes restoration, its new normalized capture is used.
    tab.pending_position = None;
    expected.position = Some(position(1, 2));
    assert_eq!(tab.stored(expected.position), Some(expected));
}

#[test]
fn explicit_startup_options_override_restore_and_ask_the_adapter_to_search() {
    let saved = saved();
    let mut tab = ReadingTab::new(17, saved.path.clone(), Some(saved));
    // The CLI facade is the exact application type.
    let view: crabpdf::cli::InitialView = InitialView {
        page: Some(99),
        zoom: Some(Zoom::FitHeight),
        layout: Some(LayoutMode::Facing),
        continuous: Some(true),
        manga: Some(false),
        rotation: Some(1),
        search: Some("Chapter".into()),
        ..Default::default()
    };
    tab.startup = Some(view);
    assert_eq!(
        loaded(&mut tab),
        OpenOutcome::Ready {
            initial_search: true
        }
    );
    assert_eq!(page(&tab), 9);
    assert_eq!(tab.state.zoom, Zoom::FitHeight);
    assert_eq!(tab.state.layout, LayoutMode::Facing);
    assert!(tab.state.continuous && !tab.state.manga);
    assert_eq!(tab.state.rotation, 1);
    assert_eq!(tab.state.colors, PageColors::Grayscale);
    assert!(tab.pending_position.is_none() && tab.startup.is_none());
    assert_eq!(tab.search.query, "Chapter");
    assert_eq!(tab.search.total, 10);
    // Identity and generation are assigned by the adapter, never a hidden clock.
    assert_eq!(tab.search.generation, 0);
}

#[test]
fn every_explicit_view_option_releases_the_saved_anchor_but_empty_options_keep_it() {
    let cases = [
        InitialView {
            page: Some(2),
            ..Default::default()
        },
        InitialView {
            zoom: Some(Zoom::FitPage),
            ..Default::default()
        },
        InitialView {
            layout: Some(LayoutMode::Single),
            ..Default::default()
        },
        InitialView {
            continuous: Some(true),
            ..Default::default()
        },
        InitialView {
            manga: Some(false),
            ..Default::default()
        },
        InitialView {
            rotation: Some(0),
            ..Default::default()
        },
        InitialView {
            search: Some(String::new()),
            ..Default::default()
        },
    ];
    for view in cases {
        let mut tab = ReadingTab::new(1, saved().path, Some(saved()));
        tab.startup = Some(view);
        loaded(&mut tab);
        assert!(tab.pending_position.is_none());
    }
    let mut tab = ReadingTab::new(1, saved().path, Some(saved()));
    tab.startup = Some(InitialView::default());
    loaded(&mut tab);
    assert_eq!(tab.pending_position, saved().position);
}

#[test]
fn failed_password_attempt_keeps_restore_and_late_events_cannot_reopen_the_tab() {
    let mut tab = ReadingTab::new(7, saved().path, Some(saved()));
    tab.startup = Some(InitialView {
        search: Some("reader".into()),
        ..Default::default()
    });
    let protected = || Err(ReaderError::new(ErrorKind::Protected, "Password required"));
    assert_eq!(tab.finish_open(8, protected(), true), OpenOutcome::Ignored);
    assert_eq!(tab.state.opening, Some(7));
    assert!(!tab.password_needed);
    assert_eq!(tab.finish_open(7, protected(), true), OpenOutcome::Failed);
    assert!(tab.password_needed && tab.restore.is_some() && tab.startup.is_some());
    assert!(tab.state.document.is_none());
    // Match the existing password adapter: retry under the same document ID.
    tab.retry_open();
    assert!(!tab.password_needed && tab.state.error.is_none());
    assert_eq!(
        loaded(&mut tab),
        OpenOutcome::Ready {
            initial_search: true
        }
    );
    assert!(!tab.password_needed);
    assert!(tab.restore.is_none() && tab.startup.is_none());
    tab.go_to(8, true);
    tab.search.query = "Changed after opening".into();
    assert_eq!(loaded(&mut tab), OpenOutcome::Ignored);
    assert_eq!(tab.finish_open(7, protected(), true), OpenOutcome::Ignored);
    assert_eq!(page(&tab), 8);
    assert_eq!(tab.search.query, "Changed after opening");
    assert!(tab.state.error.is_none());
    let mut imported = ReadingTab::new(9, "book.epub".into(), None);
    assert_eq!(
        imported.finish_open(9, protected(), false),
        OpenOutcome::Failed
    );
    assert!(!imported.password_needed);
}

#[test]
fn invalid_or_future_positions_fall_back_to_the_clamped_saved_page() {
    for invalid in [position(1, 40), position(2, 5)] {
        let mut saved = saved();
        saved.page = 99;
        saved.position = Some(invalid);
        let mut tab = ReadingTab::new(1, saved.path.clone(), Some(saved));
        loaded(&mut tab);
        assert_eq!(page(&tab), 9);
        assert!(tab.pending_position.is_none());
    }
}

#[test]
fn duplicate_tabs_keep_reading_search_and_selection_independent_when_reordered() {
    let mut tabs = [
        ReadingTab::new(12, saved().path, Some(saved())),
        ReadingTab::new(13, saved().path, None),
    ];
    for tab in &mut tabs {
        loaded(tab);
    }
    tabs[0].go_to(7, true);
    tabs[0].search.query = "first".into();
    tabs[0].selection.select(7, 2, 12);
    tabs[1].search.query = "second".into();
    tabs[1].state.set_zoom(Zoom::FitPage);
    tabs.swap(0, 1);
    assert_eq!((tabs[0].id, page(&tabs[0])), (13, 0));
    assert_eq!(tabs[0].search.query, "second");
    assert!(tabs[0].selection.range().is_none());
    assert_eq!(tabs[0].state.zoom, Zoom::FitPage);
    assert_eq!((tabs[1].id, page(&tabs[1])), (12, 7));
    assert_eq!(tabs[1].search.query, "first");
    assert_eq!(
        tabs[1]
            .selection
            .range()
            .map(|range| (range.page, range.start, range.end)),
        Some((7, 2, 12))
    );
    assert_eq!(tabs[1].state.zoom, Zoom::Percent(135));
    assert!(tabs[1].pending_position.is_none());
}

#[test]
fn old_profiles_keep_the_same_types_defaults_and_serialized_fields() {
    let legacy = r#"
[[session]]
path = "book.epub"
page = 4
zoom = "FitWidth"
layout = "Book"
continuous = true
rotation = 1
colors = "Original"
[[library]]
path = "book.epub"
"#;
    let prefs: Preferences = toml::from_str(legacy).unwrap();
    let record: &crabpdf::application::profile::StoredDocument = &prefs.session[0];
    let entry: &LibraryEntry = &prefs.library[0];
    let _: &crabpdf::application::profile::LibraryEntry = entry;
    assert!(!record.manga && record.position.is_none());
    assert!(entry.title.is_empty() && entry.opens == 0);
    let serialized = toml::to_string(&prefs).unwrap();
    let restored: Preferences = toml::from_str(&serialized).unwrap();
    assert_eq!(restored.session, prefs.session);
    assert_eq!(restored.library, prefs.library);
}
