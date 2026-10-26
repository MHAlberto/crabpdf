use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Write beside the destination, flush, then replace it in one filesystem rename.
/// A failed writer leaves the previous file intact. No whole-document buffer.
pub fn atomic_write(
    path: &Path,
    writer: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "Falta el nombre de archivo.")
    })?;
    let mut temporary = None;
    for _ in 0..100 {
        let mut candidate = name.to_os_string();
        candidate.push(format!(
            ".tmp-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let candidate = parent.join(candidate);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((Temporary(candidate), file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    let (temporary, mut file) =
        temporary.ok_or_else(|| io::Error::other("No se pudo crear un archivo temporal único."))?;
    writer(&mut file)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary.0, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn failed_write_preserves_existing_file() {
        let folder = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join("artifacts/atomic-write-test");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("preferences.toml");
        fs::write(&path, "old").unwrap();
        assert!(
            atomic_write(&path, |file| {
                file.write_all(b"partial")?;
                Err(io::Error::other("Simulated failure"))
            })
            .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), b"old");
        atomic_write(&path, |file| file.write_all(b"new")).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
    }
}
