//! Local tool discovery and bounded, cancellation-aware process execution.
use super::{MAX_FILE, check_work, io_error, limit_error};
use crate::document::{ErrorKind, ReaderError};
use std::{
    ffi::OsString,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
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
    directories.push(
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join("native/format-tools"),
    );
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

pub(super) fn run(
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
