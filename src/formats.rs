//! Format dispatch and bounded, offline imports. The original document is never rewritten.
use crate::document::{ErrorKind, ReaderError};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::time::{Duration, Instant};

pub const EXTENSIONS: &[&str] = &[
    "pdf", "ai", "epub", "mobi", "azw", "azw3", "azw4", "prc", "pdb", "fb2", "fb2z", "fbz", "zfb2",
    "xps", "oxps", "xod", "dwfx", "djvu", "djv", "chm", "cbz", "cbr", "cb7", "cbt", "zip", "rar",
    "7z", "tar", "ora", "png", "jpg", "jpeg", "jfif", "gif", "tif", "tiff", "bmp", "dib", "ico",
    "tga", "webp", "jp2", "j2k", "jpx", "jpf", "jpm", "j2c", "svg", "txt", "log", "nfo", "md",
    "markdown", "html", "htm", "xhtml",
];
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

pub fn prepared_signature(source: &Path, prepared: &Path) -> Result<String, ReaderError> {
    if Format::detect(source)? == Format::Pdf {
        return fingerprint(source)
            .map(|key| format!("pdf:{key:016x}"))
            .ok_or_else(|| io_error("No se pudo identificar el PDF."));
    }
    let file = File::open(prepared.with_extension("manifest")).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > 4096 {
        return Err(import_error("Falta la identidad del documento preparado."));
    }
    let manifest: ImportManifest = serde_json::from_reader(file).map_err(io_error)?;
    Ok(format!(
        "import-v4:{:016x}:{}:{:08x}",
        manifest.source_key, manifest.pdf.bytes, manifest.pdf.crc32
    ))
}
static SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct ImportLimits {
    cancelled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    deadline: Instant,
}

thread_local! {
    static IMPORT_LIMITS: RefCell<Option<ImportLimits>> = const { RefCell::new(None) };
}

struct RestoreLimits(Option<ImportLimits>);
impl Drop for RestoreLimits {
    fn drop(&mut self) {
        IMPORT_LIMITS.with(|current| *current.borrow_mut() = self.0.take());
    }
}

fn check_work() -> Result<(), ReaderError> {
    IMPORT_LIMITS.with(|current| {
        let current = current.borrow();
        if let Some(limits) = current.as_ref() {
            if limits.cancelled.load(AtomicOrdering::Relaxed)
                || limits.shutdown.load(AtomicOrdering::Relaxed)
            {
                return Err(ReaderError::new(
                    ErrorKind::Worker,
                    "Importación cancelada.",
                ));
            }
            if Instant::now() >= limits.deadline {
                return Err(limit_error());
            }
        }
        Ok(())
    })
}

fn lock_cache(file: &File) -> Result<(), ReaderError> {
    lock_cache_for(file, CACHE_LOCK_WAIT)
}

fn lock_cache_for(file: &File, wait: Duration) -> Result<(), ReaderError> {
    let deadline = Instant::now() + wait;
    loop {
        check_work()?;
        match file.try_lock() {
            Ok(()) => {
                check_work()?;
                return Ok(());
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(ReaderError::new(
                        ErrorKind::File,
                        "La caché de este documento sigue ocupada.",
                    )
                    .with_details("Se agotó el tiempo esperando otra importación."));
                }
                std::thread::sleep(remaining.min(CACHE_LOCK_POLL));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(io_error(error)),
        }
    }
}

pub fn prepare_cancellable_in(
    source: &Path,
    data_directory: &Path,
    cancelled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> Result<PathBuf, ReaderError> {
    with_limits(cancelled, shutdown, || prepare_in(source, data_directory))
}

fn with_limits<T>(
    cancelled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    operation: impl FnOnce() -> Result<T, ReaderError>,
) -> Result<T, ReaderError> {
    let previous = IMPORT_LIMITS.with(|current| {
        current.borrow_mut().replace(ImportLimits {
            cancelled,
            shutdown,
            deadline: Instant::now() + Duration::from_secs(180),
        })
    });
    let _restore = RestoreLimits(previous);
    operation()
}

struct ChildGuard {
    child: std::process::Child,
    finished: bool,
}
impl ChildGuard {
    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        let result = self.child.try_wait()?;
        if result.is_some() {
            self.finished = true;
        }
        Ok(result)
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CachedPart {
    bytes: u64,
    crc32: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ImportManifest {
    source_key: u64,
    pdf: CachedPart,
    xml: Option<CachedPart>,
    info: Option<CachedPart>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CachedPdfIdentity {
    source_key: u64,
    bytes: u64,
    crc32: u32,
}

pub(crate) fn cached_pdf_identity(path: &Path) -> Option<CachedPdfIdentity> {
    let file = File::open(path.with_extension("manifest")).ok()?;
    if file.metadata().ok()?.len() > 4096 {
        return None;
    }
    let manifest: ImportManifest = serde_json::from_reader(file).ok()?;
    Some(CachedPdfIdentity {
        source_key: manifest.source_key,
        bytes: manifest.pdf.bytes,
        crc32: manifest.pdf.crc32,
    })
}

pub(crate) fn repair_cancellable_in(
    source: &Path,
    rejected: &Path,
    identity: CachedPdfIdentity,
    data_directory: &Path,
    cancelled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> Result<PathBuf, ReaderError> {
    with_limits(cancelled, shutdown, || {
        check_work()?;
        let source = plain_path(source.canonicalize().map_err(io_error)?);
        let key =
            fingerprint(&source).ok_or_else(|| io_error("No se pudo identificar el archivo."))?;
        let expected = data_directory
            .join("documents")
            .join(format!("crabpdf-import-v4-{key:016x}.pdf"));
        if rejected != expected || identity.source_key != key {
            return Err(import_error(
                "La caché rechazada ya no corresponde al documento.",
            ));
        }
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(expected.with_extension("lock"))
            .map_err(io_error)?;
        lock_cache(&lock)?;
        check_work()?;
        if cached_pdf_identity(&expected) == Some(identity) {
            for path in [
                expected.with_extension("manifest"),
                expected.clone(),
                expected.with_extension("xml"),
                expected.with_extension("json"),
            ] {
                if path.exists() {
                    fs::remove_file(path).map_err(io_error)?;
                }
            }
        }
        drop(lock);
        prepare_in(&source, data_directory)
    })
}

fn part(path: &Path) -> Result<CachedPart, ReaderError> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut bytes = 0_u64;
    let mut crc = crc32fast::Hasher::new();
    loop {
        let read = file.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        bytes = bytes.saturating_add(read as u64);
        if bytes > MAX_FILE {
            return Err(limit_error());
        }
        crc.update(&buffer[..read]);
    }
    Ok(CachedPart {
        bytes,
        crc32: crc.finalize(),
    })
}

fn matches_part(path: &Path, expected: &CachedPart) -> bool {
    path.metadata()
        .is_ok_and(|metadata| metadata.len() == expected.bytes)
        && part(path)
            .is_ok_and(|actual| actual.bytes == expected.bytes && actual.crc32 == expected.crc32)
}

fn valid_cache(path: &Path, source_key: u64) -> bool {
    let manifest = path.with_extension("manifest");
    let Ok(file) = File::open(manifest) else {
        return false;
    };
    if !file.metadata().is_ok_and(|metadata| metadata.len() <= 4096) {
        return false;
    }
    let Ok(manifest) = serde_json::from_reader::<_, ImportManifest>(file) else {
        return false;
    };
    manifest.source_key == source_key
        && valid_pdf(path)
        && matches_part(path, &manifest.pdf)
        && manifest
            .xml
            .as_ref()
            .map_or(!path.with_extension("xml").exists(), |part| {
                matches_part(&path.with_extension("xml"), part)
            })
        && manifest
            .info
            .as_ref()
            .map_or(!path.with_extension("json").exists(), |part| {
                matches_part(&path.with_extension("json"), part)
            })
}

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
pub fn data_directory() -> PathBuf {
    std::env::var_os("CRABPDF_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            crate::platform::config_path().and_then(|path| path.parent().map(Path::to_owned))
        })
        .unwrap_or_else(|| std::env::temp_dir().join("crabpdf"))
}
pub fn fingerprint(path: &Path) -> Option<u64> {
    let metadata = path.metadata().ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let path = path.canonicalize().ok()?;
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

fn valid_pdf(path: &Path) -> bool {
    let mut header = [0; 5];
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let Ok(meta) = file.metadata() else {
        return false;
    };
    if meta.len() <= 5
        || meta.len() > MAX_FILE
        || file.read_exact(&mut header).is_err()
        || &header != b"%PDF-"
    {
        return false;
    }
    let mut tail = Vec::new();
    let offset = (meta.len().min(2048)) as i64;
    file.seek(SeekFrom::End(-offset)).is_ok()
        && file.read_to_end(&mut tail).is_ok()
        && tail.windows(5).any(|window| window == b"%%EOF")
}

pub fn tool(name: &str) -> PathBuf {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let mut directories = Vec::new();
    if let Some(path) = std::env::var_os("CRABPDF_FORMAT_TOOLS") {
        directories.push(PathBuf::from(path));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        directories.push(parent.join("format-tools"));
    }
    directories.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("native/format-tools"));
    let bundled = directories
        .into_iter()
        .map(|dir| dir.join(&file))
        .find(|path| path.is_file());
    if let Some(bundled) = bundled {
        return bundled;
    }
    if !cfg!(windows)
        && name == "7z"
        && let Some(paths) = std::env::var_os("PATH")
    {
        for directory in std::env::split_paths(&paths) {
            let alternative = directory.join("7zz");
            if alternative.is_file() {
                return alternative;
            }
        }
    }
    PathBuf::from(file)
}

fn run(
    name: &str,
    args: &[OsString],
    work: &Path,
    output: Option<&Path>,
) -> Result<(), ReaderError> {
    let stdout = output.map_or_else(|| work.join("tool-out.log"), Path::to_owned);
    let stderr = work.join("tool-error.log");
    let mut command = Command::new(tool(name));
    command
        .args(args)
        .current_dir(work)
        .stdin(Stdio::null())
        .stdout(Stdio::from(File::create(&stdout).map_err(io_error)?))
        .stderr(Stdio::from(File::create(&stderr).map_err(io_error)?));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    check_work()?;
    let mut child = ChildGuard {
        child: command.spawn().map_err(|error| {
            ReaderError::new(
            ErrorKind::Library,
            format!(
                "Falta el decodificador {name}. Conserva la carpeta format-tools junto a CrabPDF."
            ),
        )
        .with_details(error.to_string())
        })?,
        finished: false,
    };
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(io_error)? {
            if status.success() {
                check_work()?;
                if directory_size(work)? > MAX_FILE {
                    return Err(limit_error());
                }
                return Ok(());
            }
            let mut detail = read_log(&stderr);
            if output.is_none() {
                detail.push_str(&read_log(&stdout));
            }
            let kind = if detail.to_lowercase().contains("encrypt")
                || detail.to_lowercase().contains("password")
                || detail.contains("DRM")
            {
                ErrorKind::Protected
            } else {
                ErrorKind::InvalidPdf
            };
            return Err(ReaderError::new(
                kind,
                "No se pudo leer el documento. Comprueba que no esté dañado ni protegido con DRM.",
            )
            .with_details(detail));
        }
        check_work()?;
        if started.elapsed() > Duration::from_secs(180) || directory_size(work)? > MAX_FILE {
            return Err(limit_error());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn read_log(path: &Path) -> String {
    let mut bytes = Vec::new();
    if let Ok(file) = File::open(path) {
        let _ = file.take(64 * 1024).read_to_end(&mut bytes);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
fn directory_size(path: &Path) -> Result<u64, ReaderError> {
    let mut total: u64 = 0;
    for entry in fs::read_dir(path).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let metadata = entry.metadata().map_err(io_error)?;
        if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        } else if metadata.is_dir() {
            total = total.saturating_add(directory_size(&entry.path())?);
        }
    }
    Ok(total)
}

fn convert(inputs: &[PathBuf], output: &Path, work: &Path) -> Result<(), ReaderError> {
    let mut parts = Vec::new();
    for (index, batch) in inputs.chunks(32).enumerate() {
        let part = work.join(format!("part-{index:05}.pdf"));
        let mut args: Vec<OsString> = [
            "convert", "-F", "pdf", "-W", "612", "-H", "792", "-S", "12", "-o",
        ]
        .map(OsString::from)
        .into();
        args.push(part.as_os_str().into());
        args.extend(batch.iter().map(|path| path.as_os_str().into()));
        run("mutool", &args, work, None)?;
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(import_error("El documento no contiene páginas legibles."));
    }
    while parts.len() > 1 {
        let mut merged = Vec::new();
        for (index, batch) in parts.chunks(32).enumerate() {
            let part = work.join(format!(
                "merge-{}-{index}.pdf",
                SERIAL.fetch_add(1, AtomicOrdering::Relaxed)
            ));
            let mut args = vec!["merge".into(), "-o".into(), part.as_os_str().into()];
            args.extend(batch.iter().map(|path| path.as_os_str().into()));
            run("mutool", &args, work, None)?;
            merged.push(part);
        }
        for part in parts {
            fs::remove_file(part).map_err(io_error)?;
        }
        parts = merged;
    }
    fs::rename(&parts[0], output).map_err(io_error)?;
    if inputs.iter().all(|path| {
        matches!(
            extension(path).as_str(),
            "epub" | "fb2" | "html" | "htm" | "xhtml"
        )
    }) {
        // A failed metadata lookup never prevents reading the rendered document.
        let _ = extract_import_info(inputs, output, work);
    }
    Ok(())
}

fn extract_import_info(inputs: &[PathBuf], output: &Path, work: &Path) -> Result<(), ReaderError> {
    let script = work.join("import-info.js");
    fs::write(&script, include_str!("import_info.js")).map_err(io_error)?;
    let mut info = ImportInfo::default();
    for batch in inputs.chunks(32) {
        let mut args = vec!["run".into(), script.as_os_str().into()];
        args.extend(batch.iter().map(|path| path.as_os_str().into()));
        run("mutool", &args, work, None)?;
        let part: ImportInfo =
            serde_json::from_slice(&read_bounded(&work.join("tool-out.log"), 1024 * 1024)?)
                .map_err(io_error)?;
        if info.title.is_empty() {
            info.title = part.title;
            info.author = part.author;
        }
        for mut item in part.outline {
            item.page = item.page.map(|page| page + info.pages);
            if info.outline.len() < 10_000 {
                info.outline.push(item);
            }
        }
        info.pages += part.pages;
    }
    let bytes = serde_json::to_vec(&info).map_err(io_error)?;
    if bytes.len() > 1024 * 1024 {
        return Err(limit_error());
    }
    fs::write(output.with_extension("json"), bytes).map_err(io_error)
}

#[derive(Debug)]
struct Entry {
    name: String,
    size: u64,
    directory: bool,
}
fn archive_entries(source: &Path, work: &Path) -> Result<Vec<Entry>, ReaderError> {
    run(
        "7z",
        &[
            "l".into(),
            "-slt".into(),
            "-ba".into(),
            "-sccUTF-8".into(),
            "-p-".into(),
            source.as_os_str().into(),
        ],
        work,
        None,
    )?;
    let text = String::from_utf8(read_bounded(&work.join("tool-out.log"), 8 * 1024 * 1024)?)
        .map_err(io_error)?;
    parse_entries(&text, Format::detect(source)? == Format::Chm)
}
fn parse_entries(text: &str, chm: bool) -> Result<Vec<Entry>, ReaderError> {
    let mut entries = Vec::new();
    let mut total: u64 = 0;
    for block in text.replace("\r\n", "\n").split("\n\n") {
        let fields: Vec<_> = block
            .lines()
            .filter_map(|line| line.split_once(" = "))
            .collect();
        let field = |name| {
            fields
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| *value)
        };
        let Some(name) = field("Path") else {
            continue;
        };
        if field("Encrypted") == Some("+") {
            return Err(ReaderError::new(
                ErrorKind::Protected,
                "El archivo está protegido con contraseña.",
            ));
        }
        if field("Symbolic Link").is_some_and(|link| !link.is_empty())
            || field("Hard Link").is_some_and(|link| !link.is_empty())
        {
            return Err(import_error("El archivo contiene enlaces no compatibles."));
        }
        let relative = safe_relative(name, chm)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let size = field("Size")
            .unwrap_or("0")
            .parse::<u64>()
            .map_err(io_error)?;
        total = total.saturating_add(size);
        if size > MAX_ENTRY || total > MAX_FILE || entries.len() >= MAX_ENTRIES {
            return Err(limit_error());
        }
        entries.push(Entry {
            name: name.to_owned(),
            size,
            directory: field("Folder") == Some("+")
                || field("Attributes").is_some_and(|value| value.starts_with('D')),
        });
    }
    Ok(entries)
}
fn safe_relative(name: &str, chm: bool) -> Result<PathBuf, ReaderError> {
    let name = name.replace('\\', "/");
    let name = if chm {
        name.trim_start_matches('/')
    } else {
        &name
    };
    if name.contains(':')
        || name.contains('\0')
        || name.starts_with('/')
        || name.split('/').any(|part| part == "..")
    {
        return Err(import_error("El archivo contiene una ruta insegura."));
    }
    let path = PathBuf::from(name);
    if path
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(import_error("Ruta del archivo no compatible."));
    }
    Ok(path)
}
fn extract_entry(
    source: &Path,
    entry: &Entry,
    output: &Path,
    work: &Path,
) -> Result<(), ReaderError> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    run(
        "7z",
        &[
            "e".into(),
            "-so".into(),
            "-spd".into(),
            "-y".into(),
            "-p-".into(),
            source.as_os_str().into(),
            entry.name.clone().into(),
        ],
        work,
        Some(output),
    )?;
    if output.metadata().map_err(io_error)?.len() != entry.size {
        return Err(import_error(format!(
            "No se pudo extraer íntegramente {}: se esperaban {} bytes, se obtuvieron {}.",
            entry.name,
            entry.size,
            output.metadata().map_err(io_error)?.len()
        )));
    }
    Ok(())
}
fn image_entry(path: &Path) -> bool {
    matches!(Format::detect(path), Ok(Format::Image)) && extension(path) != "svg"
}
fn normalize_image(source: &Path, target: &Path) -> Result<(), ReaderError> {
    if extension(source) == "dib" {
        use image::ImageDecoder;
        let mut signature = [0; 2];
        let mut file = File::open(source).map_err(io_error)?;
        file.read_exact(&mut signature).map_err(io_error)?;
        if &signature != b"BM" {
            let mut decoder = image::codecs::bmp::BmpDecoder::new_without_file_header(
                std::io::BufReader::new(File::open(source).map_err(io_error)?),
            )
            .map_err(io_error)?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(128 * 1024 * 1024);
            limits.max_image_width = Some(32768);
            limits.max_image_height = Some(32768);
            decoder.set_limits(limits).map_err(io_error)?;
            return image::DynamicImage::from_decoder(decoder)
                .map_err(io_error)?
                .save_with_format(target, image::ImageFormat::Png)
                .map_err(io_error);
        }
    }
    let mut reader = image::ImageReader::open(source).map_err(io_error)?;
    reader.set_format(match extension(source).as_str() {
        "ico" => image::ImageFormat::Ico,
        "tga" => image::ImageFormat::Tga,
        _ => image::ImageFormat::Bmp,
    });
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    reader.limits(limits);
    let image = reader.decode().map_err(io_error)?;
    image
        .save_with_format(target, image::ImageFormat::Png)
        .map_err(io_error)
}
fn import_comic(source: &Path, output: &Path, work: &Path) -> Result<(), ReaderError> {
    let mut entries: Vec<_> = archive_entries(source, work)?
        .into_iter()
        .filter(|entry| !entry.directory && image_entry(Path::new(&entry.name)))
        .collect();
    // OpenRaster contains layers plus a merged preview, not a collection of comic pages.
    if extension(source) == "ora" {
        entries.retain(|entry| entry.name == "mergedimage.png");
    }
    entries.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    if entries.is_empty() {
        return Err(import_error("El archivo no contiene imágenes compatibles."));
    }
    let pages = work.join("pages");
    fs::create_dir(&pages).map_err(io_error)?;
    let mut names = String::new();
    for (index, entry) in entries.iter().enumerate() {
        let original_extension = extension(Path::new(&entry.name));
        let normalize = matches!(original_extension.as_str(), "ico" | "tga" | "dib");
        let name = format!(
            "pages/{index:05}.{}",
            if normalize {
                "png"
            } else {
                &original_extension
            }
        );
        if normalize {
            let original = work.join(format!("original-{index}.{original_extension}"));
            extract_entry(source, entry, &original, work)?;
            normalize_image(&original, &work.join(&name))?;
            fs::remove_file(original).map_err(io_error)?;
        } else {
            extract_entry(source, entry, &work.join(&name), work)?;
        }
        names.push_str(&name);
        names.push('\n');
    }
    fs::write(work.join("pages.txt"), names).map_err(io_error)?;
    let comic = work.join("pages.cbz");
    run(
        "7z",
        &[
            "a".into(),
            "-tzip".into(),
            "-mx=0".into(),
            "-scsUTF-8".into(),
            comic.as_os_str().into(),
            "@pages.txt".into(),
        ],
        work,
        None,
    )?;
    convert(&[comic], output, work)
}
fn import_chm(source: &Path, output: &Path, work: &Path) -> Result<(), ReaderError> {
    let entries = archive_entries(source, work)?;
    let resources = work.join("help");
    fs::create_dir(&resources).map_err(io_error)?;
    let mut html = Vec::new();
    let mut contents = Vec::new();
    for entry in entries
        .iter()
        .filter(|entry| !entry.directory && entry.size > 0)
    {
        let relative = safe_relative(&entry.name, true)?;
        if relative
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(['#', '$']))
        {
            continue;
        }
        let path = resources.join(relative);
        extract_entry(source, entry, &path, work)?;
        match extension(&path).as_str() {
            "html" | "htm" | "xhtml" => html.push(path),
            "hhc" => contents.push(path),
            _ => {}
        }
    }
    html.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
    // The CHM table of contents defines chapter order. A fallback includes unlisted HTML topics.
    let mut ordered = Vec::new();
    for content in contents {
        let text = String::from_utf8_lossy(&fs::read(&content).map_err(io_error)?).into_owned();
        for tag in text
            .split('<')
            .filter_map(|tag| tag.split_once('>').map(|(tag, _)| tag))
        {
            if !tag.to_lowercase().starts_with("param ") {
                continue;
            }
            let attributes = attributes(tag);
            if attributes
                .iter()
                .any(|(key, value)| key == "name" && value.eq_ignore_ascii_case("local"))
                && let Some((_, local)) = attributes.iter().find(|(key, _)| key == "value")
            {
                let relative = safe_relative(local.split('#').next().unwrap_or(local), true)?;
                let path = resources.join(relative);
                if html.contains(&path) && !ordered.contains(&path) {
                    ordered.push(path);
                }
            }
        }
    }
    for path in html {
        if !ordered.contains(&path) {
            ordered.push(path);
        }
    }
    convert(&ordered, output, work)
}

fn make_epub(opf: &Path, files: &[PathBuf], epub: &Path, work: &Path) -> Result<(), ReaderError> {
    let parent = opf
        .parent()
        .ok_or_else(|| import_error("Paquete Kindle inválido."))?;
    let package = work.join("epub");
    fs::create_dir_all(package.join("META-INF")).map_err(io_error)?;
    fs::write(package.join("mimetype"), "application/epub+zip").map_err(io_error)?;
    let opf_name = opf
        .file_name()
        .ok_or_else(|| import_error("Paquete Kindle inválido."))?
        .to_string_lossy();
    fs::write(package.join("META-INF/container.xml"), format!("<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/{opf_name}\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>")).map_err(io_error)?;
    for file in files {
        if let Ok(relative) = file.strip_prefix(parent) {
            let target = package.join("OEBPS").join(relative);
            if let Some(dir) = target.parent() {
                fs::create_dir_all(dir).map_err(io_error)?;
            }
            fs::copy(file, target).map_err(io_error)?;
        }
    }
    run(
        "7z",
        &[
            "a".into(),
            "-tzip".into(),
            "-mx=0".into(),
            epub.as_os_str().into(),
            "mimetype".into(),
            "META-INF".into(),
            "OEBPS".into(),
        ],
        &package,
        None,
    )
}

fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let bytes = tag.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && (bytes[index].is_ascii_whitespace() || bytes[index] == b'/') {
            index += 1;
        }
        let start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        let key = tag[start..index].to_lowercase();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let quote = bytes
            .get(index)
            .copied()
            .filter(|byte| *byte == b'\'' || *byte == b'"');
        if quote.is_some() {
            index += 1;
        }
        let start = index;
        while index < bytes.len()
            && quote.map_or(!bytes[index].is_ascii_whitespace(), |quote| {
                bytes[index] != quote
            })
        {
            index += 1;
        }
        fields.push((key, tag[start..index].replace("&amp;", "&")));
        if quote.is_some() && index < bytes.len() {
            index += 1;
        }
    }
    fields
}
fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), ReaderError> {
    for entry in fs::read_dir(directory).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let metadata = entry.metadata().map_err(io_error)?;
        if metadata.is_file() {
            files.push(entry.path());
        } else if metadata.is_dir() {
            collect_files(&entry.path(), files)?;
        }
        if files.len() > MAX_ENTRIES {
            return Err(limit_error());
        }
    }
    Ok(())
}
fn prune_cache(cache: &Path, protected: Option<&Path>) {
    let Ok(entries) = fs::read_dir(cache) else {
        return;
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("crabpdf-import-")
                && extension(&entry.path()) == "pdf"
        })
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            let extra = ["xml", "json"]
                .iter()
                .filter_map(|extension| {
                    entry
                        .path()
                        .with_extension(extension)
                        .metadata()
                        .ok()
                        .map(|metadata| metadata.len())
                })
                .sum::<u64>();
            Some((meta.modified().ok()?, meta.len() + extra, entry.path()))
        })
        .collect();
    files.sort_by_key(|(modified, _, _)| *modified);
    let mut bytes: u64 = files.iter().map(|(_, size, _)| *size).sum();
    // Windows refuses removing an open cached document; that document stays available.
    for (_, size, path) in files {
        if bytes <= MAX_FILE {
            break;
        }
        if protected == Some(path.as_path()) {
            continue;
        }
        let Ok(lock) = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path.with_extension("lock"))
        else {
            continue;
        };
        if lock.try_lock().is_err() {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            let _ = fs::remove_file(path.with_extension("xml"));
            let _ = fs::remove_file(path.with_extension("json"));
            let _ = fs::remove_file(path.with_extension("manifest"));
            bytes = bytes.saturating_sub(size);
        }
    }
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
    #[test]
    fn held_cache_lock_times_out_and_can_cancel_an_open() {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/reading-sample.pdf"),
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
        let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
        let previous = IMPORT_LIMITS.with(|current| {
            current.borrow_mut().replace(ImportLimits {
                cancelled: Arc::clone(&cancelled),
                shutdown: Arc::new(AtomicBool::new(false)),
                deadline: Instant::now() + Duration::from_secs(10),
            })
        });
        let _restore = RestoreLimits(previous);
        let signal = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            cancelled.store(true, AtomicOrdering::Relaxed);
        });
        let began = Instant::now();
        let result = run(
            "powershell",
            &[
                "-NoProfile".into(),
                "-Command".into(),
                "Start-Sleep -Seconds 20".into(),
            ],
            &work,
            None,
        );
        signal.join().unwrap();
        assert!(result.is_err());
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "helper was not terminated promptly"
        );
    }
}
