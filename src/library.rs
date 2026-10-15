//! Small persistent cover cache. File I/O and rendering run on the PDF worker.
use crate::document::{PageColors, PageRaster, ReaderError, RenderKey, Zoom};
use crate::engine::{PdfEngine, PdfiumEngine};
use pdfium_render::prelude::Pdfium;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Cover {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Synchronous helper retained for callers outside the render worker.
pub fn load(pdfium: &Pdfium, path: &Path, cache: Option<&Path>) -> Result<Arc<Cover>, ReaderError> {
    if let Some(cover) = cached(path, cache) {
        return Ok(cover);
    }
    let data = crate::formats::data_directory();
    let prepared = crate::formats::prepare_in(path, &data)?;
    load_prepared(pdfium, path, &prepared, cache, &data)
}

pub fn cached(path: &Path, cache: Option<&Path>) -> Option<Arc<Cover>> {
    let cached = cache.and_then(|dir| {
        fingerprint(path).map(|key| dir.join(format!("crabpdf-cover-{key:016x}.png")))
    });
    if let Some(file) = &cached
        && file
            .metadata()
            .is_ok_and(|metadata| metadata.len() <= 256 * 1024)
        && let Ok(bytes) = std::fs::read(file)
        && bytes.len() <= 256 * 1024
    {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes));
        reader.set_format(image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(256);
        limits.max_image_height = Some(256);
        limits.max_alloc = Some(1024 * 1024);
        reader.limits(limits);
        if let Ok(image) = reader.decode() {
            let image = image.into_rgba8();
            return Some(Arc::new(Cover {
                width: image.width() as usize,
                height: image.height() as usize,
                rgba: image.into_raw(),
            }));
        }
    }
    None
}

/// PDFium remains owned by the render worker. Conversion has already completed elsewhere.
pub fn load_prepared(
    pdfium: &Pdfium,
    path: &Path,
    prepared: &Path,
    cache: Option<&Path>,
    data_directory: &Path,
) -> Result<Arc<Cover>, ReaderError> {
    if let Some(cover) = cached(path, cache) {
        return Ok(cover);
    }
    let cached = cache.and_then(|dir| {
        fingerprint(path).map(|key| dir.join(format!("crabpdf-cover-{key:016x}.png")))
    });
    let mut engine = PdfiumEngine::with_data_directory(pdfium, data_directory.to_path_buf());
    engine.open_prepared(path, prepared, None)?;
    let PageRaster {
        width,
        height,
        rgba,
        ..
    } = engine.render(RenderKey {
        document_id: 0,
        revision: 0,
        page: 0,
        zoom: Zoom::FitPage,
        viewport_width: 192,
        viewport_height: 256,
        density_milli: 1000,
        bitmap_side_limit: 256,
        rotation: 0,
        colors: PageColors::Original,
        region: None,
    })?;
    if let Some(file) = cached
        && let Some(parent) = file.parent()
        && std::fs::create_dir_all(parent).is_ok()
    {
        let mut png = Vec::new();
        if let Some(image) = image::RgbaImage::from_raw(width as u32, height as u32, rgba.clone())
            && image
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .is_ok()
        {
            use std::io::Write;
            let _ = crate::fileops::atomic_write(&file, |output| output.write_all(&png));
            prune(parent);
        }
    }
    Ok(Arc::new(Cover {
        width,
        height,
        rgba,
    }))
}

fn fingerprint(path: &Path) -> Option<u64> {
    let metadata = path.metadata().ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    // Stable FNV-1a; no dependency or content scan is needed for a local thumbnail.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in path
        .to_string_lossy()
        .bytes()
        .chain(metadata.len().to_le_bytes())
        .chain(modified.to_le_bytes())
    {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    Some(hash)
}

fn prune(directory: &Path) {
    let Ok(entries) = directory.read_dir() else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("crabpdf-cover-")
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "png")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    files.sort_by_key(|(modified, _)| *modified);
    let excess = files.len().saturating_sub(64);
    for (_, file) in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(file);
    }
}
