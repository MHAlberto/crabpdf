//! Serializable profile records. Disk access and concurrent writes stay in config.
use crate::document::{PageColors, Zoom};
use crate::layout::LayoutMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StoredDocument {
    pub path: PathBuf,
    pub page: usize,
    pub zoom: Zoom,
    pub layout: LayoutMode,
    pub continuous: bool,
    pub rotation: u8,
    pub colors: PageColors,
    #[serde(default)]
    pub manga: bool,
    #[serde(default)]
    pub position: Option<crate::reading_position::ReadingPosition>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct LibraryEntry {
    pub path: PathBuf,
    pub title: String,
    pub opens: u64,
}
