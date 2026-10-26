//! Selection hit testing stays equivalent on whitespace, ties and partial glyphs.
use crabpdf::document::{Bounds, Glyph, PageContent};
fn reference(content: &PageContent, x: f32, y: f32) -> Option<usize> {
    content
        .glyphs
        .iter()
        .filter_map(|g| g.bounds.map(|b| (g.index, b)))
        .min_by(|(_, a), (_, b)| {
            let distance = |r: Bounds| {
                (x.clamp(r.left, r.right) - x).powi(2) + (y.clamp(r.top, r.bottom) - y).powi(2)
            };
            distance(*a).total_cmp(&distance(*b))
        })
        .map(|(index, _)| index)
}
#[test]
fn single_pass_matches_the_reference_across_a_dense_page_and_outside_it() {
    let content = PageContent {
        glyphs: (0..6000)
            .map(|index| {
                let x = (index % 100) as f32 * 0.009;
                let y = (index / 100) as f32 * 0.015;
                Glyph {
                    index,
                    text: "a".into(),
                    bounds: (index % 11 != 0).then_some(Bounds {
                        left: x,
                        right: x + 0.008,
                        top: y,
                        bottom: y + 0.012,
                    }),
                }
            })
            .collect(),
        ..Default::default()
    };
    for step in 0..500 {
        let x = ((step * 37) % 123) as f32 / 100.0 - 0.1;
        let y = ((step * 17) % 117) as f32 / 100.0 - 0.1;
        assert_eq!(content.nearest_glyph(x, y), reference(&content, x, y));
    }
}
#[test]
fn empty_missing_and_overlapping_glyphs_choose_the_first_real_hit() {
    let mut content = PageContent::default();
    assert_eq!(content.nearest_glyph(0.5, 0.5), None);
    content.glyphs = vec![
        Glyph {
            index: 8,
            text: " ".into(),
            bounds: None,
        },
        Glyph {
            index: 9,
            text: "a".into(),
            bounds: Some(Bounds {
                left: 0.1,
                right: 0.2,
                top: 0.1,
                bottom: 0.2,
            }),
        },
        Glyph {
            index: 10,
            text: "b".into(),
            bounds: content.glyphs.first().and_then(|g| g.bounds),
        },
    ];
    content.glyphs[2].bounds = content.glyphs[1].bounds;
    assert_eq!(content.nearest_glyph(0.15, 0.15), Some(9));
    assert_eq!(content.nearest_glyph(0.3, 0.15), Some(9));
}
