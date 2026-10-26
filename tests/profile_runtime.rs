use crabpdf::config::{Favorite, Preferences, ProfileSession, ProfileStore};
use std::{path::PathBuf, sync::Arc};

fn directory(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts/migration-tests")
        .join(format!(
            "{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
}
fn favorite(name: &str) -> Favorite {
    Favorite {
        path: name.into(),
        page: 0,
        label: name.into(),
    }
}

#[test]
fn asynchronous_sessions_flush_and_merge_using_each_windows_original_baseline() {
    let directory = directory("profile-session");
    let path = directory.join("preferences.toml");
    let (mut first, mut a, warning) = ProfileSession::open(Some(path.clone()), Arc::new(|| {}));
    assert!(warning.is_none());
    let (mut second, mut b, warning) = ProfileSession::open(Some(path.clone()), Arc::new(|| {}));
    assert!(warning.is_none());
    a.favorites.push(favorite("first.pdf"));
    first.request(a).unwrap();
    first.finish().unwrap();
    b.favorites.push(favorite("second.pdf"));
    second.request(b).unwrap();
    second.finish().unwrap();
    let merged = ProfileStore::load(&path).unwrap();
    assert_eq!(merged.favorites.len(), 2);
    assert!(
        merged
            .favorites
            .iter()
            .any(|f| f.path == std::path::Path::new("first.pdf"))
    );
    assert!(
        merged
            .favorites
            .iter()
            .any(|f| f.path == std::path::Path::new("second.pdf"))
    );
}

#[test]
fn asynchronous_storage_reports_failed_writes_without_replacing_an_unrelated_file() {
    let directory = directory("profile-failure");
    std::fs::create_dir_all(&directory).unwrap();
    let blocker = directory.join("blocked");
    std::fs::write(&blocker, b"keep this original file").unwrap();
    let path = blocker.join("preferences.toml");
    let (mut session, _, _) = ProfileSession::open(Some(path), Arc::new(|| {}));
    session.request(Preferences::default()).unwrap();
    assert!(session.finish().is_err());
    assert_eq!(std::fs::read(blocker).unwrap(), b"keep this original file");
}
