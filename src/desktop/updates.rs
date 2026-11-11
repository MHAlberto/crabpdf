use semver::Version;
use serde::Deserialize;
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub const RELEASES: &str = "https://github.com/MHAlberto/crabpdf/releases";
const API: &str = "https://api.github.com/repos/MHAlberto/crabpdf/releases?per_page=30";
const MAX_METADATA: u64 = 2 * 1024 * 1024;
const MAX_PACKAGE: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Package {
    WindowsSetup(&'static str),
    WindowsPortable(&'static str),
    Debian,
    LinuxTar,
}

impl Package {
    pub fn detect() -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            use winreg::{RegKey, enums::*};
            let arch = match std::env::consts::ARCH {
                "x86_64" => "x64",
                "x86" => "x86",
                _ => return None,
            };
            let installed_exe = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(
                    format!(r"Software\CrabPDF\Installations\{arch}"),
                    KEY_READ | KEY_WOW64_32KEY,
                )
                .and_then(|key| key.get_value::<String, _>("InstallLocation"))
                .ok()
                .map(|path| PathBuf::from(path).join("crabpdf.exe"));
            let running = std::env::current_exe().and_then(fs::canonicalize).ok();
            let installed = installed_exe.and_then(|path| fs::canonicalize(path).ok());
            Some(if installed.is_some() && installed == running {
                Self::WindowsSetup(arch)
            } else {
                Self::WindowsPortable(arch)
            })
        }
        #[cfg(target_os = "linux")]
        {
            if std::env::consts::ARCH != "x86_64" {
                return None;
            }
            let running = std::env::current_exe().ok();
            Some(
                if running.as_deref() == Some(Path::new("/opt/crabpdf/crabpdf")) {
                    Self::Debian
                } else {
                    Self::LinuxTar
                },
            )
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        None
    }

    fn asset(self, version: &Version) -> String {
        let suffix = match self {
            Self::WindowsSetup(arch) => format!("win-{arch}-setup.exe"),
            Self::WindowsPortable(arch) => format!("win-{arch}.zip"),
            Self::Debian => "ubuntu24.04-amd64.deb".into(),
            Self::LinuxTar => "linux-x64.tar.gz".into(),
        };
        format!("crabpdf-{version}-{suffix}")
    }

    pub fn can_install(self) -> bool {
        matches!(self, Self::WindowsSetup(_) | Self::Debian)
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    state: String,
    size: u64,
    digest: Option<String>,
}

#[derive(Debug)]
pub struct Update {
    pub version: Version,
    pub package: Package,
    asset: AssetDownload,
}

#[derive(Debug)]
struct AssetDownload {
    name: String,
    size: u64,
    sha256: String,
}

impl Update {
    pub fn notes_url(&self) -> String {
        format!("{RELEASES}/tag/v{}", self.version)
    }

    pub fn download_url(&self) -> String {
        format!("{RELEASES}/download/v{}/{}", self.version, self.asset.name)
    }
}

pub fn check(package: Package) -> Result<Option<Update>, String> {
    let mut body = Vec::new();
    http_get(API, MAX_METADATA, "25", &mut body)
        .map_err(|error| format!("No se pudo consultar GitHub. {error}"))?;
    let releases = serde_json::from_slice(&body)
        .map_err(|error| format!("GitHub devolvió una lista de versiones inválida. {error}"))?;
    let current = Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo version is valid");
    Ok(find_update(releases, package, &current))
}

fn find_update(releases: Vec<Release>, package: Package, current: &Version) -> Option<Update> {
    releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = Version::parse(release.tag_name.strip_prefix('v')?).ok()?;
            // Published development releases use numeric tags such as v0.4.3.
            if &version <= current || !version.pre.is_empty() || !version.build.is_empty() {
                return None;
            }
            let name = package.asset(&version);
            let asset = release
                .assets
                .into_iter()
                .find(|asset| asset.name == name)?;
            let sha256 = asset.digest?.strip_prefix("sha256:")?.to_ascii_lowercase();
            if asset.state != "uploaded"
                || !(1..=MAX_PACKAGE).contains(&asset.size)
                || !is_sha256(&sha256)
            {
                return None;
            }
            Some(Update {
                version,
                package,
                asset: AssetDownload {
                    name,
                    size: asset.size,
                    sha256,
                },
            })
        })
        .max_by(|a, b| a.version.cmp(&b.version))
}

pub enum Installed {
    /// The installer is waiting for this application to close.
    WindowsInstaller,
    DebianPackage,
}

pub fn install(update: &Update) -> Result<Installed, String> {
    if !update.package.can_install() || Package::detect() != Some(update.package) {
        return Err("La instalación actual cambió. Vuelve a buscar la actualización.".into());
    }
    let mut staged = StagedPackage::new(&update.asset.name)
        .map_err(|error| format!("No se pudo preparar la descarga. {error}"))?;
    let mut file = File::create_new(&staged.file)
        .map_err(|error| format!("No se pudo guardar la descarga. {error}"))?;
    let bytes = http_get(&update.download_url(), update.asset.size, "600", &mut file)
        .map_err(|error| format!("No se pudo descargar la actualización. {error}"))?;
    drop(file);
    if bytes != update.asset.size
        || sha256(&staged.file)
            .map_err(|error| format!("No se pudo verificar la descarga. {error}"))?
            != update.asset.sha256
    {
        return Err("El tamaño o SHA-256 del paquete difiere del publicado en GitHub.".into());
    }
    if Package::detect() != Some(update.package) {
        return Err("La instalación actual cambió durante la descarga.".into());
    }
    match update.package {
        Package::WindowsSetup(_) => {
            Command::new(&staged.file)
                .arg("/UPDATE")
                .spawn()
                .map_err(|error| format!("No se pudo abrir el instalador. {error}"))?;
            // NSIS still needs its executable while it installs the package.
            staged.handed_off = true;
            Ok(Installed::WindowsInstaller)
        }
        Package::Debian => {
            let status = Command::new("/usr/bin/pkexec")
                .args([
                    "--disable-internal-agent",
                    "/usr/bin/apt-get",
                    "install",
                    "-y",
                    "--",
                ])
                .arg(&staged.file)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|error| {
                    format!("No se pudo abrir apt con autorización de administrador. {error}")
                })?;
            if !status.success() {
                return Err(format!(
                    "No se completó la instalación mediante apt ({status})."
                ));
            }
            Ok(Installed::DebianPackage)
        }
        _ => unreachable!("only installed packages can be upgraded in place"),
    }
}

struct StagedPackage {
    file: PathBuf,
    handed_off: bool,
}

impl StagedPackage {
    fn new(name: &str) -> io::Result<Self> {
        let nonce = getrandom::u64().map_err(|error| io::Error::other(error.to_string()))?;
        let temp = std::env::temp_dir();
        if !temp.is_absolute() {
            return Err(io::Error::other(
                "La carpeta temporal debe tener una ruta absoluta.",
            ));
        }
        let directory = temp.join(format!("crabpdf-update-{nonce:016x}"));
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
        if !self.handed_off {
            let _ = fs::remove_file(&self.file);
            if let Some(dir) = self.file.parent() {
                let _ = fs::remove_dir(dir);
            }
        }
    }
}

fn http_get(url: &str, limit: u64, timeout: &str, sink: &mut impl Write) -> io::Result<u64> {
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
            "--header",
            "Accept: application/vnd.github+json",
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

fn sha256(path: &Path) -> io::Result<String> {
    let output = if cfg!(target_os = "windows") {
        background(windows_system()?.join("certutil.exe"))
            .arg("-hashfile")
            .arg(path)
            .arg("SHA256")
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

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn hashes_a_staged_file_and_removes_it_on_drop() {
        let staged = StagedPackage::new("prueba ñ").unwrap();
        let path = staged.file.clone();
        fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        drop(staged);
        assert!(!path.parent().unwrap().exists());
    }

    #[test]
    fn selects_the_highest_newer_verified_release_for_each_package() {
        let release = |version, package: Package, draft, digest| Release {
            tag_name: format!("v{version}"),
            draft,
            assets: vec![Asset {
                name: package.asset(&Version::parse(version).unwrap()),
                size: 2048,
                state: "uploaded".into(),
                digest,
            }],
        };
        let digest = || Some(format!("sha256:{}", "ab".repeat(32)));
        let current = Version::parse("0.4.3").unwrap();
        for package in [
            Package::WindowsSetup("x86"),
            Package::WindowsPortable("x64"),
            Package::Debian,
            Package::LinuxTar,
        ] {
            let update = find_update(
                vec![
                    release("0.4.5", package, false, digest()),
                    release("0.6.0", package, true, digest()),
                    release("0.5.1", package, false, Some("sha256:bad".into())),
                    release("0.4.10", package, false, digest()),
                    release("1.0.0-rc.1", package, false, digest()),
                    release("0.4.1", package, false, digest()),
                ],
                package,
                &current,
            )
            .unwrap();
            assert_eq!(update.version.to_string(), "0.4.10");
            assert!(
                update
                    .download_url()
                    .ends_with(&package.asset(&update.version))
            );
            assert!(
                find_update(
                    vec![release("0.4.3", package, false, digest())],
                    package,
                    &current
                )
                .is_none()
            );
        }
        assert!(
            find_update(
                vec![release(
                    "0.5.0",
                    Package::WindowsSetup("x64"),
                    false,
                    digest()
                )],
                Package::WindowsSetup("x86"),
                &current
            )
            .is_none()
        );
    }
}
