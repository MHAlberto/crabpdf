use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// IDs distinguish an old document/render from the user's latest request.
pub type DocumentId = u64;
pub const MAX_SEARCH_HITS: usize = 10_000;

#[derive(Clone, Debug)]
pub struct DocumentInfo {
    pub path: PathBuf,
    pub page_count: usize,
}

impl DocumentInfo {
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Library,
    File,
    InvalidPdf,
    Protected,
    Empty,
    Render,
    Worker,
}

#[derive(Clone, Debug)]
pub struct ReaderError {
    pub kind: ErrorKind,
    pub message: String,
    pub details: Option<String>,
}

impl ReaderError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

impl fmt::Display for ReaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ReaderError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Zoom {
    Percent(u16),
    FitWidth,
    FitHeight,
    FitPage,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PageColors {
    #[default]
    Original,
    Inverted,
    Grayscale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RenderKey {
    pub document_id: DocumentId,
    pub revision: u64,
    pub page: usize,
    pub zoom: Zoom,
    pub viewport_width: u32,
    pub density_milli: u32,
    pub bitmap_side_limit: u32,
    pub viewport_height: u32,
    pub rotation: u8,
    pub colors: PageColors,
    /// A bounded region of the full-resolution page, measured in device pixels.
    /// None renders a memory-capped overview, used for thumbnails and unknown geometry.
    pub region: Option<RasterRegion>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RasterRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// UI-independent pixel data. Only the requested page is rasterized.
pub struct PageRaster {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub logical_width: f32,
    pub logical_height: f32,
    pub zoom_percent: f32,
    pub resolution_limited: bool,
    pub page_size: PageSize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct PageSize {
    pub width: f32,
    pub height: f32,
}

impl Default for PageSize {
    fn default() -> Self {
        Self {
            width: 612.0,
            height: 792.0,
        }
    }
}

/// Pixels for a library cover, independent of its cache, decoder and GUI texture.
pub struct Cover {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Coordinates use the unrotated displayed PDF page, normalized, origin at top-left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Bounds {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
    pub fn intersects(&self, other: &Self) -> bool {
        self.left <= other.right
            && self.right >= other.left
            && self.top <= other.bottom
            && self.bottom >= other.top
    }
    pub fn rotated(self, turns: u8) -> Self {
        let corners = [
            (self.left, self.top),
            (self.right, self.top),
            (self.left, self.bottom),
            (self.right, self.bottom),
        ];
        let rotated = corners.map(|(x, y)| match turns % 4 {
            1 => (1.0 - y, x),
            2 => (1.0 - x, 1.0 - y),
            3 => (y, 1.0 - x),
            _ => (x, y),
        });
        Self {
            left: rotated.iter().map(|p| p.0).fold(f32::INFINITY, f32::min),
            top: rotated.iter().map(|p| p.1).fold(f32::INFINITY, f32::min),
            right: rotated
                .iter()
                .map(|p| p.0)
                .fold(f32::NEG_INFINITY, f32::max),
            bottom: rotated
                .iter()
                .map(|p| p.1)
                .fold(f32::NEG_INFINITY, f32::max),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Glyph {
    pub index: usize,
    pub text: String,
    pub bounds: Option<Bounds>,
}

#[derive(Clone, Debug)]
pub enum LinkTarget {
    Page(usize),
    Uri(String),
}

#[derive(Clone, Debug)]
pub struct PageLink {
    pub bounds: Bounds,
    pub target: LinkTarget,
}

#[derive(Clone, Debug, Default)]
pub struct PageContent {
    pub glyphs: Vec<Glyph>,
    pub links: Vec<PageLink>,
}

impl PageContent {
    pub fn memory_bytes(&self) -> usize {
        self.glyphs.capacity() * std::mem::size_of::<Glyph>()
            + self.glyphs.iter().map(|g| g.text.capacity()).sum::<usize>()
            + self.links.capacity() * std::mem::size_of::<PageLink>()
            + self
                .links
                .iter()
                .map(|link| match &link.target {
                    LinkTarget::Uri(uri) => uri.capacity(),
                    LinkTarget::Page(_) => 0,
                })
                .sum::<usize>()
    }
    pub fn text(&self, start: usize, end: usize) -> String {
        self.glyphs
            .iter()
            .filter(|g| g.index >= start.min(end) && g.index <= start.max(end))
            .map(|g| g.text.as_str())
            .collect()
    }
    pub fn nearest_glyph(&self, x: f32, y: f32) -> Option<usize> {
        let mut nearest = None;
        let mut best = f32::INFINITY;
        for glyph in &self.glyphs {
            let Some(bounds) = glyph.bounds else {
                continue;
            };
            let dx = x.clamp(bounds.left, bounds.right) - x;
            let dy = y.clamp(bounds.top, bounds.bottom) - y;
            let distance = dx * dx + dy * dy;
            if distance < best {
                best = distance;
                nearest = Some(glyph.index);
                // Text under the pointer wins; no need to scan the rest of the page.
                if distance == 0.0 {
                    break;
                }
            }
        }
        nearest
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutlineItem {
    pub title: String,
    pub page: Option<usize>,
    pub depth: usize,
}

#[derive(Clone, Debug, Default)]
pub struct DocumentDetails {
    pub first_page_size: PageSize,
    pub outline: Vec<OutlineItem>,
    pub properties: Vec<(String, String)>,
    pub can_copy: bool,
    pub can_annotate: bool,
    pub title: Option<String>,
    pub annotation_count: usize,
    pub annotation_warning: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnnotationState {
    pub revision: u64,
    pub count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkupKind {
    Highlight,
    Underline,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextMarkup {
    pub page: usize,
    pub start: usize,
    pub end: usize,
    pub kind: MarkupKind,
    pub color: [u8; 3],
}

#[derive(Clone, Debug)]
pub struct SearchRequest {
    pub document_id: DocumentId,
    pub generation: u64,
    pub query: String,
    pub match_case: bool,
    pub whole_word: bool,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub page: usize,
    pub start: usize,
    pub count: usize,
    pub bounds: Vec<Bounds>,
}
