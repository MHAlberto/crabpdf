use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(super) struct StagedPackage {
    pub(super) file: PathBuf,
    pub(super) handed_off: bool,
}

impl StagedPackage {
    pub(super) fn new(name: &str) -> io::Result<Self> {
        let nonce = getrandom::u64().map_err(|error| io::Error::other(error.to_string()))?;
        let temp = std::env::temp_dir();
        if !temp.is_absolute() {
            return Err(io::Error::other(
                "La carpeta temporal debe tener una ruta absoluta.",
            ));
        }
        let directory = temp.join(format!("crabpdf-download-{nonce:016x}"));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&directory)?;
        Ok(Self {
            file: directory.join(name),
            handed_off: false,
        })
    }
    #[cfg(windows)]
    pub(super) fn mark_for_cleanup(&self) -> io::Result<()> {
        let marker = self.file.with_file_name("update-handoff");
        let filename = self
            .file
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::other("El instalador no tiene un nombre válido."))?;
        File::create_new(marker)?.write_all(filename.as_bytes())
    }
}

impl Drop for StagedPackage {
    fn drop(&mut self) {
        if !self.handed_off
            && let Some(dir) = self.file.parent()
        {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

/// Reclaim handed-off installers on a later startup. A running executable
/// stays locked by Windows, so its marker is retained until removal succeeds.
#[cfg(windows)]
pub(super) fn clean_installer_downloads() {
    use std::{os::windows::fs::MetadataExt, time::Duration};
    let temp = std::env::temp_dir();
    if !temp.is_absolute() {
        return;
    }
    let Ok(entries) = fs::read_dir(temp) else {
        return;
    };
    let old_enough = |metadata: &fs::Metadata| {
        metadata
            .modified()
            .ok()
            .and_then(|time| time.elapsed().ok())
            .is_some_and(|age| age >= Duration::from_secs(24 * 60 * 60))
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let Some(nonce) = name
            .to_str()
            .and_then(|name| name.strip_prefix("crabpdf-download-"))
        else {
            continue;
        };
        if nonce.len() != 16 || !nonce.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let dir = entry.path();
        if !fs::symlink_metadata(&dir)
            .is_ok_and(|meta| meta.is_dir() && meta.file_attributes() & 0x400 == 0)
        {
            continue;
        }
        let marker = dir.join("update-handoff");
        if !fs::symlink_metadata(&marker).is_ok_and(|meta| {
            meta.is_file()
                && meta.file_attributes() & 0x400 == 0
                && meta.len() <= 128
                && old_enough(&meta)
        }) {
            continue;
        }
        let mut name = String::new();
        if File::open(&marker)
            .and_then(|file| file.take(129).read_to_string(&mut name))
            .is_err()
        {
            continue;
        }
        let Some(version) = name.strip_prefix("crabpdf-").and_then(|name| {
            name.strip_suffix("-win-x64-setup.exe")
                .or_else(|| name.strip_suffix("-win-x86-setup.exe"))
        }) else {
            continue;
        };
        if semver::Version::parse(version).is_err() || name.contains(['/', '\\', ':']) {
            continue;
        }
        let installer = dir.join(name);
        let removed = match fs::symlink_metadata(&installer) {
            Err(error) => error.kind() == io::ErrorKind::NotFound,
            Ok(meta) => {
                meta.is_file()
                    && meta.file_attributes() & 0x400 == 0
                    && fs::remove_file(installer).is_ok()
            }
        };
        if removed {
            let _ = fs::remove_file(marker);
            let _ = fs::remove_dir(dir);
        }
    }
}

pub(super) fn http_get(
    url: &str,
    limit: u64,
    timeout: &str,
    sink: &mut impl Write,
) -> io::Result<u64> {
    let curl = if cfg!(target_os = "windows") {
        windows_system()?.join("curl.exe")
    } else {
        PathBuf::from("/usr/bin/curl")
    };
    let mut child = background(curl)
        .args([
            "--disable",
            "--silent",
            "--fail",
            "--location",
            "--max-redirs",
            "5",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "10",
            "--max-time",
            timeout,
            "--user-agent",
            concat!("CrabPDF/", env!("CARGO_PKG_VERSION")),
            "--",
        ])
        .arg(url)
        .stdout(Stdio::piped())
        .spawn()?;
    let mut response = child
        .stdout
        .take()
        .expect("curl stdout is piped")
        .take(limit + 1);
    let written = io::copy(&mut response, sink);
    drop(response);
    if written.as_ref().is_err() || written.as_ref().is_ok_and(|bytes| *bytes > limit) {
        let _ = child.kill();
    }
    let status = child.wait()?;
    let written = written?;
    if written > limit {
        return Err(io::Error::other("La descarga supera el tamaño previsto."));
    }
    if !status.success() {
        return Err(io::Error::other(format!(
            "La conexión terminó con {status}."
        )));
    }
    Ok(written)
}

fn windows_system() -> io::Result<PathBuf> {
    // Sysnative reaches the native 64-bit tools from a 32-bit process on WOW64.
    let root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| io::Error::other("No se encontró la carpeta del sistema."))?;
    let native = root.join("Sysnative");
    Ok(if native.is_dir() {
        native
    } else {
        root.join("System32")
    })
}

fn background(program: PathBuf) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

pub(super) fn sha256(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    io::copy(&mut file, &mut digest)?;
    Ok(format!("{:x}", digest.finalize()))
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
