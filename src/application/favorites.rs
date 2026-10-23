//! Book-scoped favorite rules. No storage, GUI or native document handles.
use crate::document::DocumentInfo;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Existing profile record. Field names and serialization stay compatible.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Favorite {
    pub path: PathBuf,
    pub page: usize,
    pub label: String,
}

pub enum FavoriteAction {
    Toggle { page: usize, label: String },
    Remove(usize),
    Navigate(usize),
}

/// An outcome to apply at the boundary; this module performs no effects itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FavoriteEffect {
    Unchanged,
    Changed,
    Navigate(usize),
}

pub fn for_document<'a>(entries: &'a [Favorite], path: &Path) -> Vec<&'a Favorite> {
    let mut entries: Vec<_> = entries.iter().filter(|entry| entry.path == path).collect();
    entries.sort_by_key(|entry| entry.page);
    entries
}

pub fn remove(entries: &mut Vec<Favorite>, path: &Path, page: usize) -> bool {
    let before = entries.len();
    entries.retain(|entry| entry.path != path || entry.page != page);
    entries.len() != before
}

pub fn apply(
    entries: &mut Vec<Favorite>,
    document: &DocumentInfo,
    action: FavoriteAction,
) -> FavoriteEffect {
    match action {
        FavoriteAction::Toggle { page, label } if page < document.page_count => {
            if !remove(entries, &document.path, page) {
                entries.push(Favorite {
                    path: document.path.clone(),
                    page,
                    label,
                });
            }
            FavoriteEffect::Changed
        }
        FavoriteAction::Remove(page) if remove(entries, &document.path, page) => {
            FavoriteEffect::Changed
        }
        FavoriteAction::Navigate(page)
            if document.page_count > 0
                && entries
                    .iter()
                    .any(|entry| entry.path == document.path && entry.page == page) =>
        {
            // The source may now have fewer pages than when the favorite was saved.
            FavoriteEffect::Navigate(page.min(document.page_count - 1))
        }
        _ => FavoriteEffect::Unchanged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(path: &str) -> DocumentInfo {
        DocumentInfo {
            path: path.into(),
            page_count: 10,
        }
    }
    fn toggle(page: usize) -> FavoriteAction {
        FavoriteAction::Toggle {
            page,
            label: format!("Page {}", page + 1),
        }
    }

    #[test]
    fn commands_are_scoped_to_the_open_book_and_report_effects() {
        let a = document("a/book.pdf");
        let b = document("b/book.pdf");
        let mut entries = Vec::new();
        assert_eq!(apply(&mut entries, &a, toggle(7)), FavoriteEffect::Changed);
        apply(&mut entries, &b, toggle(2));
        apply(&mut entries, &a, toggle(2));
        assert_eq!(
            for_document(&entries, &a.path)
                .iter()
                .map(|f| f.page)
                .collect::<Vec<_>>(),
            vec![2, 7]
        );
        assert_eq!(
            apply(&mut entries, &a, FavoriteAction::Navigate(7)),
            FavoriteEffect::Navigate(7)
        );
        assert_eq!(
            apply(&mut entries, &b, FavoriteAction::Navigate(7)),
            FavoriteEffect::Unchanged
        );
        assert_eq!(
            apply(&mut entries, &a, FavoriteAction::Remove(2)),
            FavoriteEffect::Changed
        );
        assert_eq!(for_document(&entries, &b.path)[0].page, 2);
        assert_eq!(apply(&mut entries, &a, toggle(7)), FavoriteEffect::Changed);
        assert!(for_document(&entries, &a.path).is_empty());
        assert_eq!(
            apply(&mut entries, &a, FavoriteAction::Remove(7)),
            FavoriteEffect::Unchanged
        );
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn changed_page_counts_allow_navigation_and_removal_without_invalid_destinations() {
        let mut book = document("book.epub");
        let mut entries = Vec::new();
        apply(&mut entries, &book, toggle(7));
        book.page_count = 2;
        assert_eq!(
            apply(&mut entries, &book, FavoriteAction::Navigate(7)),
            FavoriteEffect::Navigate(1)
        );
        assert_eq!(
            apply(&mut entries, &book, toggle(2)),
            FavoriteEffect::Unchanged
        );
        book.page_count = 0;
        assert_eq!(
            apply(&mut entries, &book, FavoriteAction::Navigate(7)),
            FavoriteEffect::Unchanged
        );
        assert_eq!(
            apply(&mut entries, &book, FavoriteAction::Remove(7)),
            FavoriteEffect::Changed
        );
        assert!(entries.is_empty());
    }

    #[test]
    fn toggling_removes_duplicate_legacy_records_without_touching_other_books() {
        let a = document("a/book.pdf");
        let b = document("b/book.pdf");
        let mut entries = vec![
            Favorite {
                path: a.path.clone(),
                page: 1,
                label: "Old".into(),
            },
            Favorite {
                path: a.path.clone(),
                page: 1,
                label: "Duplicate".into(),
            },
            Favorite {
                path: b.path.clone(),
                page: 1,
                label: "Other".into(),
            },
        ];
        assert_eq!(apply(&mut entries, &a, toggle(1)), FavoriteEffect::Changed);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, b.path);
    }
}
