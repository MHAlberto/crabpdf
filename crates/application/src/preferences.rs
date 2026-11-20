//! Profile values and book-scoped rules, independent of storage.
use crate::{
    favorites::Favorite,
    profile::{LibraryEntry, StoredDocument},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Preferences {
    pub theme: Theme,
    pub sidebar_open: bool,
    pub restore_session: bool,
    pub native_titlebar: bool,
    pub reduce_motion: bool,
    pub recent: Vec<StoredDocument>,
    pub session: Vec<StoredDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_active_view: Option<String>,
    pub favorites: Vec<Favorite>,
    pub library: Vec<LibraryEntry>,
    pub markup_color: [u8; 3],
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            sidebar_open: false,
            restore_session: false,
            native_titlebar: false,
            reduce_motion: false,
            recent: Vec::new(),
            session: Vec::new(),
            session_active_view: None,
            favorites: Vec::new(),
            library: Vec::new(),
            markup_color: [255, 230, 50],
        }
    }
}

impl Preferences {
    /// Book-scoped view over the existing profile, preserving other books' entries.
    pub fn favorites_for(&self, path: &Path) -> Vec<&Favorite> {
        crate::favorites::for_document(&self.favorites, path)
    }
    pub fn remove_favorite(&mut self, path: &Path, page: usize) -> bool {
        crate::favorites::remove(&mut self.favorites, path, page)
    }
    pub fn record_open(&mut self, path: &Path) {
        if let Some(entry) = self.library.iter_mut().find(|entry| entry.path == path) {
            entry.opens = entry.opens.saturating_add(1);
        } else {
            self.library.push(LibraryEntry {
                path: path.to_owned(),
                opens: 1,
                ..Default::default()
            });
        }
        self.library
            .retain(|entry| self.recent.iter().any(|saved| saved.path == entry.path));
    }
    pub fn forget(&mut self, path: &Path) {
        self.recent.retain(|entry| entry.path != path);
        self.library.retain(|entry| entry.path != path);
    }
    pub fn remember(&mut self, mut document: StoredDocument) {
        document.view_id = None;
        self.recent.retain(|entry| entry.path != document.path);
        self.recent.insert(0, document);
        self.recent.truncate(30);
    }
}

impl Preferences {
    /// Reconcile only edits made by this window; preserve the other windows' edits.
    /// The storage adapter must hold the profile lock while supplying the latest disk value.
    pub fn merge_over(&self, base: &Self, disk: Self) -> Self {
        let mut merged = disk;
        macro_rules! scalar {
            ($field:ident) => {
                if self.$field != base.$field {
                    merged.$field = self.$field.clone();
                }
            };
        }
        scalar!(theme);
        scalar!(sidebar_open);
        scalar!(restore_session);
        scalar!(native_titlebar);
        scalar!(reduce_motion);
        scalar!(markup_color);
        scalar!(session_active_view);
        merge_entries(&base.recent, &self.recent, &mut merged.recent, |a, b| {
            a.path == b.path
        });
        merged.recent.truncate(30);
        if self.session != base.session {
            let same_view = |a: &StoredDocument, b: &StoredDocument| match (&a.view_id, &b.view_id)
            {
                (Some(a), Some(b)) => a == b,
                (None, None) => a.path == b.path,
                _ => false,
            };
            let mut disk = merged.session;
            let mut ordered = Vec::with_capacity(self.session.len().max(disk.len()));
            for view in &self.session {
                let previous = base.session.iter().find(|old| same_view(old, view));
                let persisted = disk
                    .iter()
                    .position(|old| same_view(old, view))
                    .map(|index| disk.remove(index));
                if previous != Some(view) {
                    ordered.push(view.clone());
                } else if let Some(view) = persisted {
                    ordered.push(view);
                }
            }
            ordered.extend(
                disk.into_iter()
                    .filter(|view| !base.session.iter().any(|old| same_view(old, view))),
            );
            merged.session = ordered;
        }
        if let Some(active) = &merged.session_active_view
            && !merged
                .session
                .iter()
                .any(|view| view.view_id.as_ref() == Some(active))
        {
            merged.session_active_view = None;
        }
        merge_entries(
            &base.favorites,
            &self.favorites,
            &mut merged.favorites,
            |a, b| a.path == b.path && a.page == b.page,
        );
        merged.library.retain(|item| {
            !base.library.iter().any(|old| old.path == item.path)
                || self.library.iter().any(|local| local.path == item.path)
        });
        for item in &self.library {
            let old = base.library.iter().find(|old| old.path == item.path);
            if old == Some(item) {
                continue;
            }
            let opens = item.opens.saturating_sub(old.map_or(0, |old| old.opens));
            if let Some(entry) = merged
                .library
                .iter_mut()
                .find(|entry| entry.path == item.path)
            {
                entry.opens = entry.opens.saturating_add(opens);
                if old.map_or(!item.title.is_empty(), |old| old.title != item.title) {
                    entry.title.clone_from(&item.title);
                }
            } else {
                merged.library.push(item.clone());
            }
        }
        merged
    }
}
fn merge_entries<T: Clone + PartialEq>(
    base: &[T],
    local: &[T],
    disk: &mut Vec<T>,
    same_key: impl Fn(&T, &T) -> bool,
) {
    for old in base {
        if !local.iter().any(|item| same_key(item, old)) {
            disk.retain(|item| !same_key(item, old));
        }
    }
    for item in local.iter().rev() {
        if base.iter().find(|old| same_key(old, item)) != Some(item) {
            disk.retain(|old| !same_key(old, item));
            disk.insert(0, item.clone());
        }
    }
}
