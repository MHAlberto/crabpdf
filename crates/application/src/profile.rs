//! Serializable profile records. Disk access and concurrent writes stay in config.
use crate::document::{PageColors, Zoom};
use crate::layout::LayoutMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StoredDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<String>,
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReadingCollection {
    pub id: String,
    pub name: String,
    pub active_view: Option<String>,
    pub tabs: Vec<StoredDocument>,
}

// Optional, additive counters. Document-specific activity stays in the current window.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReadingTotals {
    pub milliseconds: u64,
    pub page_visits: u64,
    pub focus_sessions: u64,
}

impl ReadingTotals {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn merge_over(self, base: Self, disk: Self) -> Self {
        let merge = |current: u64, previous: u64, stored: u64| {
            stored.saturating_add(current.saturating_sub(previous))
        };
        Self {
            milliseconds: merge(self.milliseconds, base.milliseconds, disk.milliseconds),
            page_visits: merge(self.page_visits, base.page_visits, disk.page_visits),
            focus_sessions: merge(
                self.focus_sessions,
                base.focus_sessions,
                disk.focus_sessions,
            ),
        }
    }
}

#[cfg(test)]
mod reading_totals_tests {
    use super::ReadingTotals;

    #[test]
    fn overlapping_windows_preserve_independent_counter_deltas() {
        let baseline = ReadingTotals {
            milliseconds: 5_000,
            page_visits: 15,
            focus_sessions: 1,
        };
        let second_window = ReadingTotals {
            milliseconds: 6_000,
            page_visits: 17,
            focus_sessions: 2,
        };
        let first_window_saved = ReadingTotals {
            milliseconds: 8_000,
            page_visits: 16,
            focus_sessions: 1,
        };
        let merged = second_window.merge_over(baseline, first_window_saved);
        assert_eq!(merged.milliseconds, 9_000);
        assert_eq!(merged.page_visits, 18);
        assert_eq!(merged.focus_sessions, 2);
    }
}
