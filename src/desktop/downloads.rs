use std::{
    fs,
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
    let output = if cfg!(target_os = "windows") {
        background(windows_system()?.join("certutil.exe"))
            .arg("-hashfile")
            .arg(path)
            .arg("SHA256")
            .output()?
    } else if cfg!(target_os = "macos") {
        background("/usr/bin/shasum".into())
            .args(["-a", "256"])
            .arg(path)
            .output()?
    } else {
        background("/usr/bin/sha256sum".into())
            .args(["--zero", "--"])
            .arg(path)
            .output()?
    };
    if !output.status.success() {
        return Err(io::Error::other(
            "Falló la comprobación SHA-256 del sistema.",
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let digest = if cfg!(target_os = "windows") {
        // CertUtil localizes the surrounding text and may insert spaces between bytes.
        stdout
            .lines()
            .map(|line| line.replace(' ', ""))
            .find(|line| is_sha256(line))
    } else {
        stdout
            .split_ascii_whitespace()
            .next()
            .filter(|word| is_sha256(word))
            .map(str::to_owned)
    };
    digest
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| io::Error::other("El sistema no devolvió un SHA-256 válido."))
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
