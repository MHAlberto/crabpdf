//! Format dispatch and bounded, offline imports. The original document is never rewritten.
pub mod djvu_text;
pub use crabpdf_application::formats::{EXTENSIONS, Format, extension};

use crate::document::{ErrorKind, ReaderError};
use std::{
    cmp::Ordering,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    time::Duration,
};

mod archive;
mod cache;
mod comic;
mod ebook;
mod limits;
mod mupdf;
mod process;
pub use crate::source::fingerprint;
#[cfg(test)]
use archive::parse_entries;
use archive::{archive_entries, extract_entry, safe_relative};
pub use cache::prepared_signature;
pub(crate) use cache::{CachedPdfIdentity, cached_pdf_identity, repair_cancellable_in};
use cache::{ImportManifest, part, prune_cache, valid_cache, valid_pdf};
use comic::{import_comic, normalize_image};
use ebook::{collect_files, import_chm, make_epub};
#[cfg(test)]
use limits::lock_cache_for;
pub use limits::prepare_cancellable_in;
use limits::{check_work, lock_cache, with_limits};
use mupdf::convert;
use process::run;
pub use process::tool;
const MAX_FILE: u64 = 512 * 1024 * 1024;
const MAX_ENTRY: u64 = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const CACHE_LOCK_WAIT: Duration = Duration::from_secs(15);
const CACHE_LOCK_POLL: Duration = Duration::from_millis(25);
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct ImportInfo {
    pub title: String,
    pub author: String,
    pub outline: Vec<crate::document::OutlineItem>,
    pub pages: usize,
}
pub fn import_info(path: &Path) -> Option<ImportInfo> {
    let path = path.with_extension("json");
    serde_json::from_slice(&read_bounded(&path, 1024 * 1024).ok()?).ok()
}

fn read_bounded(path: &Path, max_bytes: usize) -> Result<Vec<u8>, ReaderError> {
    let file = File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > max_bytes as u64 {
        return Err(auxiliary_size_error(max_bytes));
    }
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > max_bytes {
        return Err(auxiliary_size_error(max_bytes));
    }
    Ok(bytes)
}

fn auxiliary_size_error(max_bytes: usize) -> ReaderError {
    import_error(format!(
        "El archivo auxiliar excede el límite de {max_bytes} bytes."
    ))
}

static SERIAL: AtomicU64 = AtomicU64::new(0);

pub fn data_directory() -> PathBuf {
    std::env::var_os("CRABPDF_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| crate::paths::config_path().and_then(|path| path.parent().map(Path::to_owned)))
        .unwrap_or_else(|| std::env::temp_dir().join("crabpdf"))
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        // Only this process's newly-created directory, never an input or computed user path.
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn prepare(source: &Path) -> Result<PathBuf, ReaderError> {
    prepare_in(source, &data_directory())
}

pub fn prepare_in(source: &Path, data_directory: &Path) -> Result<PathBuf, ReaderError> {
    check_work()?;
    let source = plain_path(source.canonicalize().map_err(io_error)?);
    let format = Format::detect(&source)?;
    if format == Format::Pdf {
        return Ok(source);
    }
    if source.metadata().map_err(io_error)?.len() > MAX_FILE {
        return Err(limit_error());
    }
    let cache = data_directory.join("documents");
    fs::create_dir_all(&cache).map_err(io_error)?;
    let key = fingerprint(&source).ok_or_else(|| io_error("No se pudo identificar el archivo."))?;
    let target = cache.join(format!("crabpdf-import-v4-{key:016x}.pdf"));
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(target.with_extension("lock"))
        .map_err(io_error)?;
    lock_cache(&lock)?;
    if valid_cache(&target, key) {
        return Ok(target);
    }
    check_work()?;
    // A missing manifest is an incomplete publication, never a usable cache hit.
    for path in [
        &target,
        &target.with_extension("xml"),
        &target.with_extension("json"),
        &target.with_extension("manifest"),
    ] {
        if path.exists() {
            fs::remove_file(path).map_err(io_error)?;
        }
    }
    let scratch = Scratch(cache.join(format!(
        "work-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, AtomicOrdering::Relaxed)
    )));
    fs::create_dir(&scratch.0).map_err(io_error)?;
    let output = scratch.0.join("document.pdf");
    match format {
        Format::Comic => import_comic(&source, &output, &scratch.0)?,
        Format::Chm => import_chm(&source, &output, &scratch.0)?,
        Format::FictionBook if extension(&source) != "fb2" => {
            let entries = archive_entries(&source, &scratch.0)?;
            let entry = entries
                .iter()
                .find(|entry| extension(Path::new(&entry.name)) == "fb2")
                .ok_or_else(|| import_error("El archivo no contiene un FictionBook .fb2."))?;
            let input = scratch.0.join("book.fb2");
            extract_entry(&source, entry, &input, &scratch.0)?;
            convert(&[input], &output, &scratch.0)?;
        }
        Format::Kindle => {
            // libmobi handles KF8 and Huffman/CDIC, which MuPDF's basic MOBI reader doesn't.
            let input = scratch.0.join("book.mobi");
            fs::copy(&source, &input).map_err(io_error)?;
            let resources = scratch.0.join("kindle");
            fs::create_dir(&resources).map_err(io_error)?;
            let args = vec![
                "-e".into(),
                "-s".into(),
                "-o".into(),
                resources.as_os_str().into(),
                input.as_os_str().into(),
            ];
            run("mobitool", &args, &scratch.0, None)?;
            let mut files = Vec::new();
            collect_files(&resources, &mut files)?;
            let pdf = files.iter().find(|path| extension(path) == "pdf");
            if let Some(pdf) = pdf {
                fs::copy(pdf, &output).map_err(io_error)?;
            } else {
                // -s reconstructs HTML/resources and preserves embedded images for every Kindle variant.
                let opf = files.iter().find(|path| extension(path) == "opf").cloned();
                let epub = resources.join("book.epub");
                if let Some(opf) = opf {
                    make_epub(&opf, &files, &epub, &scratch.0)?;
                    convert(&[epub], &output, &scratch.0)?;
                } else {
                    let mut html: Vec<_> = files
                        .into_iter()
                        .filter(|path| matches!(extension(path).as_str(), "html" | "htm" | "xhtml"))
                        .collect();
                    html.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
                    if html.is_empty() {
                        return Err(import_error(
                            "El libro Kindle no contiene texto o PDF legible; comprueba que no tenga DRM.",
                        ));
                    }
                    convert(&html, &output, &scratch.0)?;
                }
            }
        }
        Format::Djvu => {
            let input = scratch.0.join("book.djvu");
            fs::copy(&source, &input).map_err(io_error)?;
            run(
                "ddjvu",
                &[
                    "-format=pdf".into(),
                    "-quality=90".into(),
                    input.as_os_str().into(),
                    output.as_os_str().into(),
                ],
                &scratch.0,
                None,
            )?;
            let text = scratch.0.join("text.xml");
            run(
                "djvutoxml",
                &[input.as_os_str().into(), text.as_os_str().into()],
                &scratch.0,
                None,
            )?;
            fs::copy(&text, output.with_extension("xml")).map_err(io_error)?;
        }
        Format::Xps => {
            let input = scratch.0.join("book.xps");
            fs::copy(&source, &input).map_err(io_error)?;
            convert(&[input], &output, &scratch.0)?;
        }
        Format::Image if matches!(extension(&source).as_str(), "ico" | "tga" | "dib") => {
            let input = scratch.0.join("image.png");
            normalize_image(&source, &input)?;
            convert(&[input], &output, &scratch.0)?;
        }
        Format::Text if !matches!(extension(&source).as_str(), "txt" | "md") => {
            let input = scratch.0.join(if extension(&source) == "markdown" {
                "book.md"
            } else {
                "book.txt"
            });
            fs::copy(&source, &input).map_err(io_error)?;
            convert(&[input], &output, &scratch.0)?;
        }
        _ => convert(std::slice::from_ref(&source), &output, &scratch.0)?,
    }
    check_work()?;
    if !valid_pdf(&output) {
        return Err(import_error(
            "El decodificador no produjo un documento legible.",
        ));
    }
    if fingerprint(&source) != Some(key) {
        return Err(import_error(
            "El archivo cambió durante la importación. Vuelve a abrirlo.",
        ));
    }
    check_work()?;
    let xml = output.with_extension("xml");
    let info = output.with_extension("json");
    let manifest = ImportManifest {
        source_key: key,
        pdf: part(&output)?,
        xml: xml.is_file().then(|| part(&xml)).transpose()?,
        info: info.is_file().then(|| part(&info)).transpose()?,
    };
    if xml.is_file() {
        fs::rename(&xml, target.with_extension("xml")).map_err(io_error)?;
    }
    if info.is_file() {
        fs::rename(&info, target.with_extension("json")).map_err(io_error)?;
    }
    fs::rename(&output, &target).map_err(io_error)?;
    let published = scratch.0.join("complete.manifest");
    let mut file = File::create(&published).map_err(io_error)?;
    serde_json::to_writer(&mut file, &manifest).map_err(io_error)?;
    file.flush().map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    drop(file);
    fs::rename(published, target.with_extension("manifest")).map_err(io_error)?;
    if !valid_cache(&target, key) {
        return Err(import_error("No se pudo verificar la caché del documento."));
    }
    prune_cache(&cache, Some(&target));
    Ok(target)
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left = left.to_lowercase();
    let right = right.to_lowercase();
    let a = left.as_bytes();
    let b = right.as_bytes();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (start_a, start_b) = (i, j);
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let number_a = left[start_a..i].trim_start_matches('0');
            let number_b = right[start_b..j].trim_start_matches('0');
            let order = number_a
                .len()
                .cmp(&number_b.len())
                .then_with(|| number_a.cmp(number_b));
            if order != Ordering::Equal {
                return order;
            }
        } else {
            let order = a[i].cmp(&b[j]);
            if order != Ordering::Equal {
                return order;
            }
            i += 1;
            j += 1;
        }
    }
    a.len().cmp(&b.len()).then_with(|| left.cmp(&right))
}
fn plain_path(path: PathBuf) -> PathBuf {
    // MuPDF's Windows CLI misinterprets the verbatim prefix returned by canonicalize().
    // Keep this adaptation at the subprocess boundary, retaining Unicode names.
    if cfg!(windows)
        && let Some(text) = path.to_str()
    {
        if let Some(rest) = text.strip_prefix("\\\\?\\UNC\\") {
            return PathBuf::from(format!("\\\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix("\\\\?\\") {
            return PathBuf::from(rest);
        }
    }
    path
}

fn io_error(error: impl std::fmt::Display) -> ReaderError {
    ReaderError::new(ErrorKind::File, "No se pudo preparar el documento.")
        .with_details(error.to_string())
}
fn import_error(detail: impl Into<String>) -> ReaderError {
    ReaderError::new(ErrorKind::InvalidPdf, "No se pudo leer este formato.").with_details(detail)
}
fn limit_error() -> ReaderError {
    import_error("La importación excedió el límite de 512 MiB, 10 000 recursos o 3 minutos.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, atomic::AtomicBool},
        time::Instant,
    };
    #[test]
    fn held_cache_lock_times_out_and_can_cancel_an_open() {
        let directory = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join("artifacts")
            .join(format!(
                "import-lock-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let source = directory.join("book.txt");
        let cache = directory.join("data/documents");
        fs::create_dir_all(&cache).unwrap();
        fs::write(&source, "Libro de prueba").unwrap();
        let key = fingerprint(&source).unwrap();
        let lock_path = cache.join(format!("crabpdf-import-v4-{key:016x}.lock"));
        let held = File::create(&lock_path).unwrap();
        held.lock().unwrap();
        let waiter = fs::OpenOptions::new().write(true).open(&lock_path).unwrap();

        let began = Instant::now();
        let timeout = lock_cache_for(&waiter, Duration::from_millis(150)).unwrap_err();
        assert_eq!(timeout.kind, ErrorKind::File);
        assert!(began.elapsed() < Duration::from_secs(2));

        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&cancelled);
        let cancel_thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            signal.store(true, AtomicOrdering::Relaxed);
        });
        let began = Instant::now();
        let result = prepare_cancellable_in(
            &source,
            &directory.join("data"),
            cancelled,
            Arc::new(AtomicBool::new(false)),
        );
        cancel_thread.join().unwrap();
        assert_eq!(result.unwrap_err().kind, ErrorKind::Worker);
        assert!(began.elapsed() < Duration::from_secs(2));
        assert!(!lock_path.with_extension("pdf").exists());
        drop(waiter);
        drop(held);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn formats_and_aliases_dispatch_without_case_confusion() {
        for extension in EXTENSIONS {
            assert!(
                Format::detect(Path::new(&format!("book.{extension}"))).is_ok(),
                "{extension}"
            );
        }
        assert_eq!(
            Format::detect(Path::new("BOOK.EPUB")).unwrap(),
            Format::Epub
        );
        assert_eq!(
            Format::detect(Path::new("book.fb2.zip")).unwrap(),
            Format::FictionBook
        );
        assert!(Format::detect(Path::new("book.exe")).is_err());
    }
    #[test]
    fn archive_paths_and_sizes_are_bounded() {
        for name in [
            "../escape.png",
            "C:/escape.png",
            "/escape.png",
            "a/../../x",
            "a\\..\\x",
        ] {
            assert!(safe_relative(name, false).is_err());
        }
        assert_eq!(
            safe_relative("/help/index.htm", true).unwrap(),
            PathBuf::from("help/index.htm")
        );
        assert!(parse_entries("Path = image.png\nSize = 900000000\n", false).is_err());
        assert!(parse_entries("Path = image.png\nSize = 12\nEncrypted = +\n", false).is_err());
    }
    #[test]
    fn comic_pages_use_natural_order() {
        let mut pages = ["page10.png", "page2.png", "page001.png"];
        pages.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(pages, ["page001.png", "page2.png", "page10.png"]);
    }

    #[test]
    fn cached_import_requires_complete_matching_files() {
        let directory = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join("artifacts")
            .join(format!(
                "import-manifest-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&directory).unwrap();
        let pdf = directory.join("cached.pdf");
        fs::copy(
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
                .join("tests/fixtures/reading-sample.pdf"),
            &pdf,
        )
        .unwrap();
        let source_key = 4242;
        assert!(!valid_cache(&pdf, source_key));
        let info = pdf.with_extension("json");
        fs::write(&info, b"{\"title\":\"book\"}").unwrap();
        let manifest = ImportManifest {
            source_key,
            pdf: part(&pdf).unwrap(),
            xml: None,
            info: Some(part(&info).unwrap()),
        };
        fs::write(
            pdf.with_extension("manifest"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(valid_cache(&pdf, source_key));
        assert!(!valid_cache(&pdf, source_key + 1));
        fs::write(&info, b"{\"title\":\"bad!\"}").unwrap();
        assert!(!valid_cache(&pdf, source_key));
        fs::write(&info, b"{\"title\":\"book\"}").unwrap();
        let mut bytes = fs::read(&pdf).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 0x01;
        fs::write(&pdf, bytes).unwrap();
        assert!(!valid_cache(&pdf, source_key));
    }

    #[cfg(windows)]
    #[test]
    fn cancelled_external_decoder_exits_promptly() {
        let work = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join("artifacts")
            .join(format!(
                "cancel-decoder-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&work).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal_cancelled = Arc::clone(&cancelled);
        let signal = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            signal_cancelled.store(true, AtomicOrdering::Relaxed);
        });
        let began = Instant::now();
        let result = with_limits(
            Arc::clone(&cancelled),
            Arc::new(AtomicBool::new(false)),
            || {
                run(
                    "powershell",
                    &[
                        "-NoProfile".into(),
                        "-Command".into(),
                        "Start-Sleep -Seconds 20".into(),
                    ],
                    &work,
                    None,
                )
            },
        );
        signal.join().unwrap();
        assert!(result.is_err());
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "helper was not terminated promptly"
        );
    }
}
