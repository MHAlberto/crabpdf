use super::downloads::{StagedPackage, http_get, is_sha256, sha256};
use semver::Version;
use serde::Deserialize;
#[cfg(windows)]
use std::{fs, path::PathBuf};
use std::{
    fs::File,
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
                if running.as_deref() == Some(std::path::Path::new("/opt/crabpdf/crabpdf")) {
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
            #[cfg(windows)]
            staged
                .mark_for_cleanup()
                .map_err(|error| format!("No se pudo preparar el instalador. {error}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn hashes_a_staged_file_and_removes_it_on_drop() {
        let staged = StagedPackage::new("prueba ñ").unwrap();
        let path = staged.file.clone();
        std::fs::write(&path, b"abc").unwrap();
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
