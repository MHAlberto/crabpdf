use std::env;
use std::path::{Path, PathBuf};

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

pub fn config_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("RUST_READER_CONFIG") {
        return Some(PathBuf::from(path));
    }
    let root = if cfg!(target_os = "windows") {
        env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").map(|p| Path::new(&p).join("Library/Application Support"))
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|p| Path::new(&p).join(".config")))
    };
    root.map(|root| root.join("rust-reader").join("preferences.toml"))
}

/// Construct the async dialog on the UI thread (required for reliable macOS use).
pub fn pdf_dialog() -> impl std::future::Future<Output = Option<rfd::FileHandle>> + Send {
    rfd::AsyncFileDialog::new()
        .set_title("Abrir un documento")
        .add_filter("Documentos compatibles", crate::formats::EXTENSIONS)
        .pick_file()
}

pub fn new_window(path: Option<&Path>) -> Result<(), crate::document::ReaderError> {
    new_window_with_paths(
        path,
        &crate::app_paths::AppPaths::from_environment(config_path()),
    )
}

pub fn new_window_with_paths(
    path: Option<&Path>,
    paths: &crate::app_paths::AppPaths,
) -> Result<(), crate::document::ReaderError> {
    let executable = env::current_exe().map_err(platform_error)?;
    let mut command = std::process::Command::new(executable);
    if let Some(config) = &paths.config {
        command.arg("--config").arg(config);
    }
    command.env("CRABPDF_DATA_DIR", &paths.data);
    if let Some(path) = path {
        command.arg(path);
    }
    command.spawn().map_err(platform_error)?;
    Ok(())
}

pub fn reveal(path: &Path) -> Result<(), crate::document::ReaderError> {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("explorer.exe");
        c.arg(format!("/select,{}", path.display()));
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-R").arg(path);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path.parent().unwrap_or(path));
        c
    };
    command.spawn().map_err(platform_error)?;
    Ok(())
}

fn platform_error(error: std::io::Error) -> crate::document::ReaderError {
    crate::document::ReaderError::new(
        crate::document::ErrorKind::File,
        "No se pudo abrir el servicio del sistema.",
    )
    .with_details(error.to_string())
}
