//! Document-linked notes. Persistence and UI stay outside the reading model.
use crate::document::Bounds;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

pub const MAX_NOTE_TEXT: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteAnchor {
    /// Physical page indices, zero-based.
    pub first_page: usize,
    pub last_page: usize,
    /// Normalized unrotated region on the first page, when available.
    pub region: Option<Bounds>,
}

impl NoteAnchor {
    pub fn page(page: usize) -> Self {
        Self {
            first_page: page,
            last_page: page,
            region: None,
        }
    }

    pub fn label(self) -> String {
        let first = self.first_page.saturating_add(1);
        if self.first_page == self.last_page {
            format!("Página {first}")
        } else {
            format!("Páginas {first}–{}", self.last_page.saturating_add(1))
        }
    }

    pub fn is_valid(self) -> bool {
        self.first_page <= self.last_page
            && self.region.is_none_or(|r| {
                [r.left, r.top, r.right, r.bottom]
                    .into_iter()
                    .all(|n| (0.0..=1.0).contains(&n))
                    && r.left < r.right
                    && r.top < r.bottom
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    Page,
    Quote,
    Latex,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub kind: NoteKind,
    pub anchor: NoteAnchor,
    pub excerpt: String,
    pub comment: String,
}

impl Note {
    pub fn is_valid(&self) -> bool {
        self.anchor.is_valid()
            && self.excerpt.len() <= MAX_NOTE_TEXT
            && self.comment.len() <= MAX_NOTE_TEXT
            && match self.kind {
                NoteKind::Page => self.excerpt.is_empty() && !self.comment.trim().is_empty(),
                NoteKind::Quote | NoteKind::Latex => !self.excerpt.trim().is_empty(),
            }
    }

    pub fn page(anchor: NoteAnchor) -> Self {
        Self {
            id: 0,
            kind: NoteKind::Page,
            anchor,
            excerpt: String::new(),
            comment: String::new(),
        }
    }
}

/// Export text without requiring a Markdown renderer or interpreting the result.
pub fn markdown(document: &str, notes: &[Note]) -> String {
    let title = document.replace(['\r', '\n'], " ");
    let mut out = format!("# Cuaderno de lectura\n\n{title}\n\n");
    for note in notes {
        let _ = writeln!(out, "## {}\n", note.anchor.label());
        match note.kind {
            NoteKind::Page => {}
            NoteKind::Quote => {
                for line in note.excerpt.lines() {
                    let _ = writeln!(out, "> {line}");
                }
                out.push('\n');
            }
            NoteKind::Latex => {
                let longest = note
                    .excerpt
                    .split(|ch| ch != '\u{60}')
                    .map(str::len)
                    .max()
                    .unwrap_or(0);
                let fence = "\u{60}".repeat(3.max(longest + 1));
                let _ = writeln!(out, "{fence}latex\n{}\n{fence}\n", note.excerpt);
            }
        }
        if !note.comment.trim().is_empty() {
            let _ = writeln!(out, "{}\n", note.comment);
        }
    }
    out
}
