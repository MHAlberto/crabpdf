use crabpdf::config::{Favorite, Preferences};
use std::path::PathBuf;

#[test]
fn a_second_process_cannot_erase_the_first_windows_favorite() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts")
        .join(format!(
            "profile-process-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&directory).unwrap();
    let config = directory.join("preferences.toml");
    let base = Preferences::load(&config).unwrap();
    let mut first = base.clone();
    first.favorites.push(Favorite {
        path: PathBuf::from("first.pdf"),
        page: 1,
        label: "Primera ventana".into(),
    });

    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "helper_writes_a_favorite"])
        .env("CRABPDF_TEST_PREF_PATH", &config)
        .status()
        .unwrap();
    assert!(status.success());
    // The first process still has the snapshot loaded before the helper wrote to disk.
    first.save_merged(&config, &base).unwrap();
    let latest = Preferences::load(&config).unwrap();
    assert_eq!(latest.favorites.len(), 2);
    assert!(
        latest
            .favorites
            .iter()
            .any(|item| item.path == std::path::Path::new("first.pdf"))
    );
    assert!(
        latest
            .favorites
            .iter()
            .any(|item| item.path == std::path::Path::new("second.pdf"))
    );
}

#[test]
#[ignore = "Invoked by the parent process with a private configuration path"]
fn helper_writes_a_favorite() {
    let Some(path) = std::env::var_os("CRABPDF_TEST_PREF_PATH") else {
        return;
    };
    let path = PathBuf::from(path);
    let base = Preferences::load(&path).unwrap();
    let mut second = base.clone();
    second.favorites.push(Favorite {
        path: PathBuf::from("second.pdf"),
        page: 2,
        label: "Segunda ventana".into(),
    });
    second.save_merged(&path, &base).unwrap();
}
