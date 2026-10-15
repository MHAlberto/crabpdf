//! Versioned page-relative reading positions, independent of UI and PDF engines.
use crate::layout::{PageLayout, ReadingAnchor};
use serde::{Deserialize, Serialize};

const UNIT: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingPosition {
    schema: u8,
    page: usize,
    page_x: u32,
    page_y: u32,
    viewport_x: u32,
    viewport_y: u32,
}

impl ReadingPosition {
    pub fn page(self) -> usize {
        self.page
    }

    pub fn capture(layout: &PageLayout, scroll: (f32, f32), viewport: (f32, f32)) -> Option<Self> {
        if !valid_viewport(viewport) || !scroll.0.is_finite() || !scroll.1.is_finite() {
            return None;
        }
        let center = (scroll.0 + viewport.0 * 0.5, scroll.1 + viewport.1 * 0.5);
        let placement = layout
            .placements
            .iter()
            .filter(|p| {
                p.left < scroll.0 + viewport.0
                    && p.left + p.size.width > scroll.0
                    && p.top < scroll.1 + viewport.1
                    && p.top + p.size.height > scroll.1
            })
            .min_by(|a, b| {
                let distance = |p: &crate::layout::Placement| {
                    (center.0 - center.0.clamp(p.left, p.left + p.size.width)).powi(2)
                        + (center.1 - center.1.clamp(p.top, p.top + p.size.height)).powi(2)
                };
                distance(a).total_cmp(&distance(b))
            })?;
        let point = (
            center
                .0
                .clamp(placement.left, placement.left + placement.size.width),
            center
                .1
                .clamp(placement.top, placement.top + placement.size.height),
        );
        let encode = |value: f32| (value.clamp(0.0, 1.0) * UNIT as f32).round() as u32;
        Some(Self {
            schema: 1,
            page: placement.page,
            page_x: encode((point.0 - placement.left) / placement.size.width),
            page_y: encode((point.1 - placement.top) / placement.size.height),
            viewport_x: encode((point.0 - scroll.0) / viewport.0),
            viewport_y: encode((point.1 - scroll.1) / viewport.1),
        })
    }

    pub fn anchor(self, viewport: (f32, f32), page_count: usize) -> Option<ReadingAnchor> {
        if self.schema != 1
            || self.page >= page_count
            || !valid_viewport(viewport)
            || [self.page_x, self.page_y, self.viewport_x, self.viewport_y]
                .iter()
                .any(|v| *v > UNIT)
        {
            return None;
        }
        let decode = |value: u32| value as f32 / UNIT as f32;
        Some(ReadingAnchor {
            page: self.page,
            x: decode(self.page_x),
            y: decode(self.page_y),
            screen_x: decode(self.viewport_x) * viewport.0,
            screen_y: decode(self.viewport_y) * viewport.1,
        })
    }
}

fn valid_viewport(viewport: (f32, f32)) -> bool {
    viewport.0.is_finite() && viewport.1.is_finite() && viewport.0 > 0.0 && viewport.1 > 0.0
}
