use crate::document::{Bounds, PageSize, RasterRegion, Zoom};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutMode {
    #[default]
    Single,
    Facing,
    Book,
}

impl LayoutMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "Una página",
            Self::Facing => "Dos páginas",
            Self::Book => "Libro",
        }
    }
    pub fn spread(self, page: usize, count: usize) -> Vec<usize> {
        let start = match self {
            Self::Single => page,
            Self::Facing => page / 2 * 2,
            Self::Book => {
                if page == 0 {
                    0
                } else {
                    (page - 1) / 2 * 2 + 1
                }
            }
        };
        let end = if self == Self::Single || self == Self::Book && start == 0 {
            start + 1
        } else {
            start + 2
        };
        (start..end.min(count)).collect()
    }
}

pub fn page_size(size: PageSize, rotation: u8, zoom: Zoom, width: f32, height: f32) -> PageSize {
    let (width, height) =
        logical_page_size(size, rotation, zoom, f64::from(width), f64::from(height));
    PageSize {
        width: width as f32,
        height: height as f32,
    }
}
pub fn logical_page_size(
    size: PageSize,
    rotation: u8,
    zoom: Zoom,
    width: f64,
    height: f64,
) -> (f64, f64) {
    let (w, h) = if rotation.is_multiple_of(2) {
        (size.width, size.height)
    } else {
        (size.height, size.width)
    };
    let w = f64::from(w);
    let h = f64::from(h);
    let base = w * 96.0 / 72.0;
    let desired = match zoom {
        Zoom::Percent(p) => base * f64::from(p.clamp(8, 6400)) / 100.0,
        Zoom::FitWidth => width,
        Zoom::FitHeight => height * w / h,
        Zoom::FitPage => width.min(height * w / h),
    };
    (desired.max(1.0), (desired * h / w).max(1.0))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutSpec {
    pub width: u32,
    pub height: u32,
    pub zoom: Zoom,
    pub mode: LayoutMode,
    pub continuous: bool,
    pub current: usize,
    pub rotation: u8,
    pub manga: bool,
    pub compact: bool,
    pub revision: u64,
}
impl LayoutSpec {
    pub fn gutter(self) -> f32 {
        if self.compact { 4.0 } else { 24.0 }
    }

    fn gap(self) -> f32 {
        if self.compact { 8.0 } else { 16.0 }
    }

    pub fn cell_size(self) -> (f32, f32) {
        let width = if self.mode == LayoutMode::Single {
            self.width as f32 - 2.0 * self.gutter()
        } else {
            (self.width as f32 - 2.0 * self.gutter() - self.gap()) / 2.0
        };
        (
            width.round().max(1.0),
            (self.height as f32 - 2.0 * self.gutter()).round().max(1.0),
        )
    }
}
pub struct Placement {
    pub page: usize,
    pub left: f32,
    pub top: f32,
    pub size: PageSize,
}
pub struct PageLayout {
    pub placements: Vec<Placement>,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct ReadingAnchor {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub screen_x: f32,
    pub screen_y: f32,
}
impl ReadingAnchor {
    pub fn offset(self, layout: &PageLayout) -> Option<(f32, f32)> {
        let placement = layout.placements.iter().find(|p| p.page == self.page)?;
        Some((
            (placement.left + self.x * placement.size.width - self.screen_x).max(0.0),
            (placement.top + self.y * placement.size.height - self.screen_y).max(0.0),
        ))
    }
}
impl PageLayout {
    pub fn build(sizes: &[Option<PageSize>], fallback: PageSize, spec: LayoutSpec) -> Self {
        let gutter = spec.gutter();
        let gap = spec.gap();
        let (cell_width, cell_height) = spec.cell_size();
        let mut spreads = Vec::new();
        let mut next = 0;
        if spec.continuous {
            while next < sizes.len() {
                let spread = spec.mode.spread(next, sizes.len());
                next = spread.last().copied().unwrap_or(next) + 1;
                spreads.push(spread);
            }
        } else {
            spreads.push(spec.mode.spread(spec.current, sizes.len()));
        }
        let mut placements = Vec::new();
        let mut top = gutter;
        let mut width = spec.width as f32;
        let mut rows = Vec::new();
        for mut spread in spreads {
            if spec.manga {
                spread.reverse();
            }
            let measured = spread
                .iter()
                .map(|page| {
                    page_size(
                        sizes[*page].unwrap_or(fallback),
                        spec.rotation,
                        spec.zoom,
                        cell_width.max(1.0),
                        cell_height,
                    )
                })
                .collect::<Vec<_>>();
            let row_width = measured.iter().map(|s| s.width).sum::<f32>()
                + gap * measured.len().saturating_sub(1) as f32;
            let row_height = measured.iter().map(|s| s.height).fold(0.0, f32::max);
            let mut left = gutter;
            let start = placements.len();
            for (page, size) in spread.into_iter().zip(measured) {
                placements.push(Placement {
                    page,
                    left,
                    top,
                    size,
                });
                left += size.width + gap;
            }
            rows.push((start, placements.len(), row_width));
            width = width.max(row_width + 2.0 * gutter);
            top += row_height + gutter;
        }
        for (start, end, row_width) in rows {
            let offset = (width - row_width) / 2.0 - gutter;
            for placement in &mut placements[start..end] {
                placement.left += offset;
            }
        }
        Self {
            placements,
            width,
            height: top,
        }
    }
    pub fn top_of(&self, page: usize) -> Option<f32> {
        self.placements
            .iter()
            .find(|p| p.page == page)
            .map(|p| p.top)
    }
}

/// Only tiles intersecting the viewport are requested. Memory is independent of
/// the page's total pixel area, including at 6400% zoom.
pub fn visible_tiles(width: u32, height: u32, visible: Bounds, side: u32) -> Vec<RasterRegion> {
    let side = side.clamp(1, 1024);
    let left = (visible.left.clamp(0.0, 1.0) * width as f32).floor() as u32 / side;
    let top = (visible.top.clamp(0.0, 1.0) * height as f32).floor() as u32 / side;
    let right = (visible.right.clamp(0.0, 1.0) * width as f32).ceil() as u32;
    let bottom = (visible.bottom.clamp(0.0, 1.0) * height as f32).ceil() as u32;
    let mut result = Vec::new();
    for y in top..bottom.div_ceil(side) {
        for x in left..right.div_ceil(side) {
            let x = x * side;
            let y = y * side;
            result.push(RasterRegion {
                x,
                y,
                width: side.min(width.saturating_sub(x)),
                height: side.min(height.saturating_sub(y)),
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vertical_pages_fit_without_horizontal_overflow_and_anchor_survives_zoom() {
        let sizes = [
            Some(PageSize {
                width: 600.0,
                height: 800.0,
            }),
            Some(PageSize {
                width: 800.0,
                height: 500.0,
            }),
        ];
        let spec = LayoutSpec {
            width: 1000,
            height: 700,
            zoom: Zoom::FitWidth,
            mode: LayoutMode::Single,
            continuous: true,
            current: 0,
            rotation: 0,
            manga: false,
            compact: false,
            revision: 0,
        };
        let layout = PageLayout::build(&sizes, PageSize::default(), spec);
        assert!(
            layout
                .placements
                .iter()
                .all(|page| page.left >= 0.0 && page.left + page.size.width <= 1000.0)
        );
        assert!(
            layout.placements[1].top > layout.placements[0].top + layout.placements[0].size.height
        );
        let anchor = ReadingAnchor {
            page: 1,
            x: 0.5,
            y: 0.5,
            screen_x: 500.0,
            screen_y: 350.0,
        };
        let enlarged = PageLayout::build(
            &sizes,
            PageSize::default(),
            LayoutSpec {
                zoom: Zoom::Percent(200),
                ..spec
            },
        );
        let (x, y) = anchor.offset(&enlarged).unwrap();
        let page = &enlarged.placements[1];
        assert!(((x + 500.0 - page.left) / page.size.width - 0.5).abs() < 0.0001);
        assert!(((y + 350.0 - page.top) / page.size.height - 0.5).abs() < 0.0001);
    }
    #[test]
    fn book_cover_and_last_spread_are_correct() {
        assert_eq!(LayoutMode::Book.spread(0, 5), vec![0]);
        assert_eq!(LayoutMode::Book.spread(2, 5), vec![1, 2]);
        assert_eq!(LayoutMode::Facing.spread(4, 5), vec![4]);
        assert_eq!(LayoutMode::Book.spread(3, 4), vec![3]);
    }
    #[test]
    fn fit_page_preserves_aspect_ratio_after_rotation() {
        let size = page_size(
            PageSize {
                width: 600.0,
                height: 800.0,
            },
            1,
            Zoom::FitPage,
            800.0,
            400.0,
        );
        assert!((size.width - 533.3333).abs() < 0.01);
        assert_eq!(size.height, 400.0);
    }
    #[test]
    fn zoomed_viewport_requests_bounded_tiles_and_clips_edges() {
        let tiles = visible_tiles(
            51200,
            68267,
            Bounds {
                left: 0.5,
                top: 0.5,
                right: 0.52,
                bottom: 0.515,
            },
            1024,
        );
        assert!(!tiles.is_empty() && tiles.len() <= 6);
        assert!(tiles.iter().all(|t| t.width <= 1024 && t.height <= 1024));
        let edge = visible_tiles(
            1100,
            1300,
            Bounds {
                left: 0.99,
                top: 0.99,
                right: 1.0,
                bottom: 1.0,
            },
            1024,
        );
        assert_eq!(
            edge,
            vec![RasterRegion {
                x: 1024,
                y: 1024,
                width: 76,
                height: 276
            }]
        );
    }
}
