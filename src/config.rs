use crate::document::{PageColors, Zoom};
use crate::layout::LayoutMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::JoinHandle;

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
            favorites: Vec::new(),
            library: Vec::new(),
            markup_color: [255, 230, 50],
        }
    }
}

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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Favorite {
    pub path: PathBuf,
    pub page: usize,
    pub label: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct LibraryEntry {
    pub path: PathBuf,
    pub title: String,
    pub opens: u64,
}

impl Preferences {
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
    pub fn remember(&mut self, document: StoredDocument) {
        self.recent.retain(|entry| entry.path != document.path);
        self.recent.insert(0, document);
        self.recent.truncate(30);
    }
    /// Missing preferences are normal; corrupt/unreadable preferences are reported.
    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("Preferencias inválidas: {e}")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("No se pudieron leer las preferencias: {e}")),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        crate::fileops::atomic_write(path, |file| file.write_all(text.as_bytes()))
            .map_err(|e| format!("No se pudieron guardar las preferencias: {e}"))
    }

    /// Reconcile only fields changed by this window with the latest on-disk state.
    /// The adjacent file lock covers read, merge and atomic replacement across processes.
    pub fn save_merged(&self, path: &Path, base: &Self) -> Result<Self, String> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let lock_path = path.with_extension("lock");
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path)
            .map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        let disk = Self::load(path)?;
        let mut merged = disk.clone();
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
        merge_entries(&base.recent, &self.recent, &mut merged.recent, |a, b| {
            a.path == b.path
        });
        merged.recent.truncate(30);
        merge_entries(&base.session, &self.session, &mut merged.session, |a, b| {
            a.path == b.path
        });
        merge_entries(
            &base.favorites,
            &self.favorites,
            &mut merged.favorites,
            |a, b| a.path == b.path && a.page == b.page,
        );
        merge_entries(&base.library, &self.library, &mut merged.library, |a, b| {
            a.path == b.path
        });
        merged.save(path)?;
        Ok(merged)
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

#[derive(Default)]
struct WriteQueue {
    pending: Option<Preferences>,
    stopping: bool,
}

/// Coalesces saves on a disk thread so fsync and another window's lock never block painting.
pub struct PreferenceWriter {
    queue: Arc<(Mutex<WriteQueue>, Condvar)>,
    results: mpsc::Receiver<Result<(), String>>,
    thread: Option<JoinHandle<()>>,
}

impl PreferenceWriter {
    pub fn start(
        path: PathBuf,
        mut base: Preferences,
        repaint: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
        let queue = Arc::new((Mutex::new(WriteQueue::default()), Condvar::new()));
        let background = Arc::clone(&queue);
        let (sender, results) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("crabpdf-preferences".into())
            .spawn(move || {
                loop {
                    let snapshot = {
                        let (mutex, wake) = &*background;
                        let mut state = match mutex.lock() {
                            Ok(state) => state,
                            Err(_) => return,
                        };
                        while state.pending.is_none() && !state.stopping {
                            state = match wake.wait(state) {
                                Ok(state) => state,
                                Err(_) => return,
                            };
                        }
                        if state.pending.is_none() && state.stopping {
                            return;
                        }
                        state.pending.take().expect("pending snapshot")
                    };
                    let result = snapshot.save_merged(&path, &base).map(|_| {
                        // Track this window's last snapshot, not the merged disk state.
                        // Otherwise its next save could delete another window's additions.
                        base = snapshot;
                    });
                    let _ = sender.send(result);
                    repaint();
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            queue,
            results,
            thread: Some(thread),
        })
    }

    pub fn request(&self, snapshot: Preferences) -> Result<(), String> {
        let (mutex, wake) = &*self.queue;
        mutex.lock().map_err(|e| e.to_string())?.pending = Some(snapshot);
        wake.notify_one();
        Ok(())
    }

    pub fn poll(&self) -> Option<Result<(), String>> {
        self.results.try_recv().ok()
    }

    pub fn finish(&mut self) -> Result<(), String> {
        let (mutex, wake) = &*self.queue;
        mutex.lock().map_err(|e| e.to_string())?.stopping = true;
        wake.notify_one();
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| "Falló el hilo de preferencias.".to_owned())?;
        }
        let mut last = Ok(());
        while let Ok(result) = self.results.try_recv() {
            if result.is_err() {
                last = result;
            }
        }
        last
    }
}

impl Drop for PreferenceWriter {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_roundtrip_and_defaults() {
        let p = Preferences {
            markup_color: [255, 230, 50],
            theme: Theme::Dark,
            sidebar_open: true,
            restore_session: true,
            native_titlebar: false,
            reduce_motion: false,
            recent: vec![StoredDocument {
                path: PathBuf::from("Lecturas/niño.pdf"),
                page: 42,
                zoom: Zoom::Percent(200),
                layout: LayoutMode::Book,
                continuous: true,
                rotation: 3,
                colors: PageColors::Grayscale,
                manga: true,
                position: None,
            }],
            session: vec![StoredDocument {
                path: PathBuf::from("otro.pdf"),
                page: 0,
                zoom: Zoom::FitPage,
                layout: LayoutMode::Facing,
                continuous: false,
                rotation: 0,
                colors: PageColors::Original,
                manga: false,
                position: None,
            }],
            favorites: vec![Favorite {
                path: PathBuf::from("otro.pdf"),
                page: 0,
                label: "Introducción".into(),
            }],
            library: Vec::new(),
        };
        let text = toml::to_string(&p).unwrap();
        assert_eq!(toml::from_str::<Preferences>(&text).unwrap(), p);
        assert_eq!(
            toml::from_str::<Preferences>("").unwrap(),
            Preferences::default()
        );
        assert!(toml::from_str::<Preferences>("theme = 'unknown'").is_err());
    }
    #[test]
    fn library_counts_opens_and_removes_history_without_touching_files() {
        let mut preferences = Preferences::default();
        let saved = StoredDocument {
            path: PathBuf::from("book.pdf"),
            page: 2,
            zoom: Zoom::FitWidth,
            layout: LayoutMode::Single,
            continuous: true,
            rotation: 0,
            colors: PageColors::Original,
            manga: false,
            position: None,
        };
        preferences.remember(saved.clone());
        preferences.record_open(&saved.path);
        preferences.record_open(&saved.path);
        assert_eq!(preferences.library[0].opens, 2);
        let encoded = toml::to_string(&preferences).unwrap();
        assert_eq!(
            Preferences::load(Path::new("does-not-exist.toml")).unwrap(),
            Preferences::default()
        );
        assert_eq!(
            toml::from_str::<Preferences>(&encoded).unwrap(),
            preferences
        );
        preferences.forget(&saved.path);
        assert!(preferences.library.is_empty() && preferences.recent.is_empty());
    }

    #[test]
    fn overlapping_windows_preserve_each_others_favorites_and_sessions() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("artifacts")
            .join(format!(
                "preferences-merge-{}-{}.toml",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let mut first = Preferences::default();
        let mut second = Preferences::default();
        first.favorites.push(Favorite {
            path: PathBuf::from("shared.pdf"),
            page: 1,
            label: "A".into(),
        });
        second.favorites.push(Favorite {
            path: PathBuf::from("shared.pdf"),
            page: 2,
            label: "B".into(),
        });
        let saved = |name: &str| StoredDocument {
            path: PathBuf::from(name),
            page: 2,
            zoom: Zoom::FitWidth,
            layout: LayoutMode::Single,
            continuous: true,
            rotation: 0,
            colors: PageColors::Original,
            manga: false,
            position: None,
        };
        first.session.push(saved("first.pdf"));
        second.session.push(saved("second.pdf"));
        let default = Preferences::default();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                first.save_merged(&path, &default).unwrap();
            });
            scope.spawn(|| {
                second.save_merged(&path, &default).unwrap();
            });
        });
        let merged = Preferences::load(&path).unwrap();
        assert_eq!(merged.favorites.len(), 2);
        assert_eq!(merged.session.len(), 2);
        let first_base = first.clone();
        first.theme = Theme::Dark;
        first.save_merged(&path, &first_base).unwrap();
        let after = Preferences::load(&path).unwrap();
        assert_eq!(after.favorites.len(), 2);
        assert_eq!(after.session.len(), 2);
        assert_eq!(after.theme, Theme::Dark);
    }
}
