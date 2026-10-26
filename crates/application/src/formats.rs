//! Supported format vocabulary; classification never opens a file.
use crabpdf_core::document::{ErrorKind, ReaderError};
use std::path::Path;

pub const EXTENSIONS: &[&str] = &[
    "pdf", "ai", "epub", "mobi", "azw", "azw3", "azw4", "prc", "pdb", "fb2", "fb2z", "fbz", "zfb2",
    "xps", "oxps", "xod", "dwfx", "djvu", "djv", "chm", "cbz", "cbr", "cb7", "cbt", "zip", "rar",
    "7z", "tar", "ora", "png", "jpg", "jpeg", "jfif", "gif", "tif", "tiff", "bmp", "dib", "ico",
    "tga", "webp", "jp2", "j2k", "jpx", "jpf", "jpm", "j2c", "svg", "txt", "log", "nfo", "md",
    "markdown", "html", "htm", "xhtml",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Epub,
    Kindle,
    FictionBook,
    Xps,
    Djvu,
    Chm,
    Comic,
    Image,
    Text,
    Html,
}
impl Format {
    pub fn detect(path: &Path) -> Result<Self, ReaderError> {
        let extension = extension(path);
        let format = match extension.as_str() {
            "pdf" | "ai" => Self::Pdf,
            "epub" => Self::Epub,
            "mobi" | "azw" | "azw3" | "azw4" | "prc" | "pdb" => Self::Kindle,
            "fb2" | "fb2z" | "fbz" | "zfb2" => Self::FictionBook,
            "zip"
                if path.file_name().is_some_and(|name| {
                    name.to_string_lossy().to_lowercase().ends_with(".fb2.zip")
                }) =>
            {
                Self::FictionBook
            }
            "xps" | "oxps" | "xod" | "dwfx" => Self::Xps,
            "djvu" | "djv" => Self::Djvu,
            "chm" => Self::Chm,
            "cbz" | "cbr" | "cb7" | "cbt" | "zip" | "rar" | "7z" | "tar" | "ora" => Self::Comic,
            "svg" | "png" | "jpg" | "jpeg" | "jfif" | "gif" | "tif" | "tiff" | "bmp" | "dib"
            | "ico" | "tga" | "webp" | "jp2" | "j2k" | "jpx" | "jpf" | "jpm" | "j2c" => Self::Image,
            "txt" | "log" | "nfo" | "md" | "markdown" => Self::Text,
            "html" | "htm" | "xhtml" => Self::Html,
            _ => {
                return Err(
                    ReaderError::new(ErrorKind::InvalidPdf, "Formato no compatible.")
                        .with_details(format!("Extensión: .{extension}")),
                );
            }
        };
        Ok(format)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Epub => "EPUB",
            Self::Kindle => "MOBI / Kindle / PalmDOC",
            Self::FictionBook => "FictionBook",
            Self::Xps => "XPS",
            Self::Djvu => "DjVu",
            Self::Chm => "CHM",
            Self::Comic => "Cómic / archivo de imágenes",
            Self::Image => "Imagen",
            Self::Text => "Texto / Markdown",
            Self::Html => "HTML",
        }
    }
}

pub fn extension(path: &Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
}
