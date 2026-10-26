use std::env;
use std::path::PathBuf;

/// Keep native library discovery and OS directories outside the PDF engine/UI.
pub fn library_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "pdfium.dll"
    } else if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    }
}

pub fn library_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("RUST_READER_PDFIUM") {
        let path = PathBuf::from(path);
        candidates.push(if path.is_dir() {
            path.join(library_name())
        } else {
            path
        });
    }
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(library_name()));
        candidates.push(dir.join("native").join(library_name()));
    }
    if let Ok(dir) = env::current_dir() {
        candidates.push(dir.join("native").join(library_name()));
    }
    candidates.dedup();
    candidates
}
