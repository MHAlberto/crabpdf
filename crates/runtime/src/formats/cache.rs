//! Import identity and transactional cache publication/recovery. Existing v4 keys are preserved.
use super::{
    Format, MAX_FILE, check_work, extension, fingerprint, import_error, io_error, limit_error,
    lock_cache, plain_path, prepare_in, with_limits,
};
use crate::document::ReaderError;
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct CachedPart {
    pub(super) bytes: u64,
    pub(super) crc32: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct ImportManifest {
    pub(super) source_key: u64,
    pub(super) pdf: CachedPart,
    pub(super) xml: Option<CachedPart>,
    pub(super) info: Option<CachedPart>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CachedPdfIdentity {
    pub(super) source_key: u64,
    pub(super) bytes: u64,
    pub(super) crc32: u32,
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

pub(super) fn part(path: &Path) -> Result<CachedPart, ReaderError> {
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

pub(super) fn matches_part(path: &Path, expected: &CachedPart) -> bool {
    path.metadata()
        .is_ok_and(|metadata| metadata.len() == expected.bytes)
        && part(path)
            .is_ok_and(|actual| actual.bytes == expected.bytes && actual.crc32 == expected.crc32)
}

pub(super) fn valid_cache(path: &Path, source_key: u64) -> bool {
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
pub(super) fn valid_pdf(path: &Path) -> bool {
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

pub(super) fn prune_cache(cache: &Path, protected: Option<&Path>) {
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
