use crabpdf::config::{Preferences, StoredDocument};
use crabpdf::document::{PageSize, Zoom};
use crabpdf::layout::{LayoutMode, LayoutSpec, PageLayout};
use crabpdf::reading_position::ReadingPosition;

fn layout(width: u32, height: u32, zoom: Zoom) -> PageLayout {
    PageLayout::build(
        &[
            Some(PageSize::default()),
            Some(PageSize {
                width: 400.0,
                height: 3000.0,
            }),
        ],
        PageSize::default(),
        LayoutSpec {
            width,
            height,
            zoom,
            mode: LayoutMode::Single,
            continuous: true,
            current: 0,
            rotation: 0,
            manga: true,
            compact: false,
            revision: 0,
        },
    )
}

#[test]
fn saved_point_survives_window_zoom_and_logical_scale_changes() {
    let before = layout(1080, 780, Zoom::FitWidth);
    let page = &before.placements[1];
    let scroll = (
        page.left + page.size.width * 0.5 - 540.0,
        page.top + page.size.height * 0.68 - 390.0,
    );
    let saved = ReadingPosition::capture(&before, scroll, (1080.0, 780.0)).unwrap();
    assert_eq!(saved.page(), 1);
    for (width, height, zoom) in [
        (760, 520, Zoom::FitWidth),
        (1200, 900, Zoom::Percent(200)),
        (540, 390, Zoom::FitWidth),
    ] {
        let after = layout(width, height, zoom);
        let anchor = saved.anchor((width as f32, height as f32), 2).unwrap();
        let offset = anchor.offset(&after).unwrap();
        let page = &after.placements[1];
        let visible_y = (offset.1 + anchor.screen_y - page.top) / page.size.height;
        assert!((visible_y - 0.68).abs() < 0.00001);
        let roundtrip =
            ReadingPosition::capture(&after, offset, (width as f32, height as f32)).unwrap();
        assert_eq!(roundtrip, saved);
    }
}

#[test]
fn a_center_in_the_page_gap_does_not_move_the_view_when_reopened() {
    let pages = layout(800, 600, Zoom::Percent(50));
    let first = &pages.placements[0];
    let gap_center = (first.top + first.size.height + pages.placements[1].top) * 0.5;
    let scroll = (0.0, gap_center - 300.0);
    let saved = ReadingPosition::capture(&pages, scroll, (800.0, 600.0)).unwrap();
    let restored = saved
        .anchor((800.0, 600.0), 2)
        .unwrap()
        .offset(&pages)
        .unwrap();
    assert!((restored.0 - scroll.0).abs() < 0.01);
    assert!((restored.1 - scroll.1).abs() < 0.01);
}

#[test]
fn facing_book_rotation_and_manga_keep_the_same_page_point() {
    let sizes = vec![Some(PageSize::default()); 5];
    for mode in [LayoutMode::Facing, LayoutMode::Book] {
        for rotation in 0..4 {
            for manga in [false, true] {
                let spec = LayoutSpec {
                    width: 1200,
                    height: 700,
                    zoom: Zoom::Percent(200),
                    mode,
                    continuous: true,
                    current: 0,
                    rotation,
                    manga,
                    compact: false,
                    revision: 0,
                };
                let before = PageLayout::build(&sizes, PageSize::default(), spec);
                let page = before.placements.iter().find(|p| p.page == 2).unwrap();
                let scroll = (
                    page.left + page.size.width * 0.5 - 600.0,
                    page.top + page.size.height * 0.4 - 350.0,
                );
                let saved = ReadingPosition::capture(&before, scroll, (1200.0, 700.0)).unwrap();
                assert_eq!(saved.page(), 2);
                let after = PageLayout::build(
                    &sizes,
                    PageSize::default(),
                    LayoutSpec {
                        width: 840,
                        height: 500,
                        zoom: Zoom::Percent(150),
                        ..spec
                    },
                );
                let anchor = saved.anchor((840.0, 500.0), sizes.len()).unwrap();
                let offset = anchor.offset(&after).unwrap();
                let actual = ReadingPosition::capture(&after, offset, (840.0, 500.0)).unwrap();
                assert_eq!(actual, saved);
            }
        }
    }
}

#[test]
fn old_preferences_load_and_future_or_invalid_positions_fall_back_to_the_page() {
    let legacy = "path = 'book.pdf'\npage = 1\nzoom = 'FitWidth'\nlayout = 'Single'\ncontinuous = true\nrotation = 0\ncolors = 'Original'\n";
    let old: StoredDocument = toml::from_str(legacy).unwrap();
    assert!(!old.manga);
    assert!(old.position.is_none());
    let layout = layout(800, 600, Zoom::FitWidth);
    let position = ReadingPosition::capture(&layout, (0.0, 1500.0), (800.0, 600.0)).unwrap();
    let saved = StoredDocument {
        manga: true,
        position: Some(position),
        ..old
    };
    let preferences = Preferences {
        recent: vec![saved],
        ..Preferences::default()
    };
    let text = toml::to_string(&preferences).unwrap();
    assert_eq!(toml::from_str::<Preferences>(&text).unwrap(), preferences);
    for invalid in [
        text.replace("schema = 1", "schema = 2"),
        text.replace("page_x = 500000", "page_x = 2000000"),
    ] {
        let decoded: Preferences = toml::from_str(&invalid).unwrap();
        let position = decoded.recent[0].position.unwrap();
        assert!(position.anchor((800.0, 600.0), 2).is_none());
        assert_eq!(decoded.recent[0].page, 1);
    }
    assert!(position.anchor((0.0, 600.0), 2).is_none());
    assert!(position.anchor((800.0, 600.0), 0).is_none());
}
