//! Folder packages, immutable versions and per-package atomic activation.
use crate::fileops::atomic_write;
use crabpdf_extension_api::{API_VERSION, Manifest};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_FILES: usize = 4096;

#[derive(Clone, Debug)]
pub struct InstalledPackage {
    pub manifest: Manifest,
    pub directory: PathBuf,
    pub enabled: bool,
}
#[derive(Clone)]
pub struct Repository {
    root: PathBuf,
}

pub fn target() -> String {
    let arch = if cfg!(target_arch = "x86") {
        "i686"
    } else {
        std::env::consts::ARCH
    };
    let platform = if cfg!(windows) {
        "pc-windows-msvc"
    } else if cfg!(target_os = "macos") {
        "apple-darwin"
    } else {
        "unknown-linux-gnu"
    };
    format!("{arch}-{platform}")
}
fn bad(message: impl std::fmt::Display) -> io::Error {
    io::Error::other(message.to_string())
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'.' | b'_')
        })
        && !value.starts_with('.')
        && !value.ends_with('.')
}
fn plain(path: &Path) -> io::Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(bad("El paquete contiene un enlace o punto de reanálisis."));
        }
    }
    if metadata.file_type().is_symlink() {
        return Err(bad("El paquete contiene un enlace."));
    }
    Ok(metadata)
}
fn safe_relative(value: &str) -> io::Result<&Path> {
    let path = Path::new(value);
    if value.is_empty()
        || value.contains(['\\', ':'])
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(bad(
            "El ejecutable debe usar una ruta relativa dentro del paquete.",
        ));
    }
    Ok(path)
}
impl Repository {
    pub fn new(data: &Path) -> Self {
        Self {
            root: data.join("extensions"),
        }
    }
    fn lock(&self) -> io::Result<File> {
        fs::create_dir_all(&self.root)?;
        plain(&self.root)?;
        let path = self.root.join("packages.lock");
        if path.exists() {
            plain(&path)?;
        }
        let file = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        file.try_lock().map_err(|_| {
            bad("Los paquetes están en uso en otra operación. Vuelve a intentarlo.")
        })?;
        Ok(file)
    }
    pub fn inspect(&self, directory: &Path) -> io::Result<Manifest> {
        plain(directory)?;
        let path = directory.join("extension.toml");
        plain(&path)?;
        let mut source = String::new();
        File::open(path)?.take(65537).read_to_string(&mut source)?;
        if source.len() > 65536 {
            return Err(bad("El manifiesto supera 64 KiB."));
        }
        let manifest: Manifest = toml::from_str(&source).map_err(bad)?;
        if manifest.api_version != API_VERSION
            || !identifier(&manifest.id)
            || manifest.name.is_empty()
            || manifest.name.len() > 120
            || manifest.description.len() > 2000
            || manifest.commands.is_empty()
            || manifest.commands.len() > 16
        {
            return Err(bad("Identidad, API o comandos incompatibles."));
        }
        let version = semver::Version::parse(&manifest.version).map_err(bad)?;
        if version.to_string() != manifest.version {
            return Err(bad("Versión de paquete inválida."));
        }
        let host = semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(bad)?;
        if !semver::VersionReq::parse(&manifest.host_version)
            .map_err(bad)?
            .matches(&host)
            || manifest.target != target()
        {
            return Err(bad(format!(
                "El paquete requiere otra versión de CrabPDF o plataforma. Plataforma actual: {}",
                target()
            )));
        }
        let mut ids = std::collections::HashSet::new();
        for command in &manifest.commands {
            if !identifier(&command.id)
                || command.title.is_empty()
                || command.title.len() > 120
                || !ids.insert(&command.id)
            {
                return Err(bad("Identificador o título de comando inválido."));
            }
        }
        let executable = safe_relative(&manifest.executable)?;
        let mut path = directory.to_owned();
        for component in executable.components() {
            path.push(component);
            plain(&path)?;
        }
        if !path.is_file() {
            return Err(bad("Falta el ejecutable del paquete."));
        }
        Ok(manifest)
    }
    fn package_dir(&self, id: &str, version: &str) -> io::Result<PathBuf> {
        if !identifier(id) || semver::Version::parse(version).is_err() {
            return Err(bad("Identificador de paquete inválido."));
        }
        let installed = self.root.join("installed");
        if installed.exists() {
            plain(&installed)?;
        }
        let group = installed.join(id);
        if group.exists() {
            plain(&group)?;
        }
        Ok(group.join(version))
    }
    fn activation(&self, id: &str) -> PathBuf {
        self.root.join("enabled").join(format!("{id}.json"))
    }
    fn active_version(&self, id: &str) -> io::Result<Option<String>> {
        let path = self.activation(id);
        if !path.exists() {
            return Ok(None);
        }
        plain(&path)?;
        let mut bytes = Vec::new();
        File::open(&path)?.take(257).read_to_end(&mut bytes)?;
        if bytes.len() > 256 {
            return Err(bad("Registro de activación inválido."));
        }
        serde_json::from_slice(&bytes).map(Some).map_err(bad)
    }
    pub fn list(&self) -> io::Result<Vec<InstalledPackage>> {
        let _lock = self.lock()?;
        let installed = self.root.join("installed");
        if !installed.exists() {
            return Ok(Vec::new());
        }
        plain(&installed)?;
        let mut packages = Vec::new();
        for (index, group) in fs::read_dir(&installed)?.take(257).enumerate() {
            if index >= 256 {
                return Err(bad("Hay demasiados paquetes instalados."));
            }
            let group = group?.path();
            plain(&group)?;
            for (index, version) in fs::read_dir(&group)?.take(65).enumerate() {
                if index >= 64 {
                    return Err(bad("Hay demasiadas versiones instaladas de un paquete."));
                }
                let path = version?.path();
                let manifest = self.inspect(&path)?;
                if self.package_dir(&manifest.id, &manifest.version)? != path {
                    return Err(bad("La identidad del paquete difiere de su carpeta."));
                }
                let enabled =
                    self.active_version(&manifest.id)?.as_deref() == Some(&manifest.version);
                packages.push(InstalledPackage {
                    manifest,
                    directory: path,
                    enabled,
                });
                if packages.len() > 256 {
                    return Err(bad("Hay demasiados paquetes instalados."));
                }
            }
        }
        packages.sort_by(|a, b| {
            (&a.manifest.id, &a.manifest.version).cmp(&(&b.manifest.id, &b.manifest.version))
        });
        Ok(packages)
    }
    pub fn install(&self, source: &Path) -> io::Result<()> {
        let _lock = self.lock()?;
        plain(source)?;
        let source = fs::canonicalize(source)?;
        let manifest = self.inspect(&source)?;
        let destination = self.package_dir(&manifest.id, &manifest.version)?;
        if destination.exists() {
            return Err(bad("Esta versión ya está instalada."));
        }
        let installed = self.root.join("installed");
        fs::create_dir_all(&installed)?;
        plain(&installed)?;
        let parent = destination.parent().expect("package has a parent");
        fs::create_dir_all(parent)?;
        plain(parent)?;
        let stage = self
            .root
            .join(format!("staging-{:016x}", getrandom::u64().map_err(bad)?));
        fs::create_dir(&stage)?;
        let result = (|| {
            let mut limits = (0_u64, 0_usize);
            copy_tree(&source, &stage, 0, &mut limits)?;
            self.inspect(&stage)?;
            fs::rename(&stage, &destination)
        })();
        // Only the newly created staging directory is cleaned after a failed copy.
        if stage.exists() {
            let _ = remove_tree(&stage);
        }
        result
    }
    pub fn enable(&self, package: &InstalledPackage, enabled: bool) -> io::Result<()> {
        let _lock = self.lock()?;
        let path = self.package_dir(&package.manifest.id, &package.manifest.version)?;
        self.inspect(&path)?;
        let state = self.root.join("enabled");
        fs::create_dir_all(&state)?;
        plain(&state)?;
        let path = self.activation(&package.manifest.id);
        if enabled {
            atomic_write(&path, |out| {
                serde_json::to_writer(out, &package.manifest.version).map_err(bad)
            })
        } else if self.active_version(&package.manifest.id)?.as_deref()
            == Some(&package.manifest.version)
        {
            fs::remove_file(path)
        } else {
            Ok(())
        }
    }
    pub fn remove(&self, package: &InstalledPackage) -> io::Result<()> {
        let _lock = self.lock()?;
        let directory = self.package_dir(&package.manifest.id, &package.manifest.version)?;
        self.inspect(&directory)?;
        if self.active_version(&package.manifest.id)?.as_deref() == Some(&package.manifest.version)
        {
            fs::remove_file(self.activation(&package.manifest.id))?;
        }
        remove_tree(&directory)
    }
    /// Lock out installation/removal until the package's process has exited.
    pub(super) fn acquire(&self, package: &InstalledPackage) -> io::Result<File> {
        let lock = self.lock()?;
        let expected = self.package_dir(&package.manifest.id, &package.manifest.version)?;
        if expected != package.directory
            || self.active_version(&package.manifest.id)?.as_deref()
                != Some(&package.manifest.version)
        {
            return Err(bad("La extensión fue desactivada o cambió de versión."));
        }
        let manifest = self.inspect(&expected)?;
        if serde_json::to_vec(&manifest)? != serde_json::to_vec(&package.manifest)? {
            return Err(bad(
                "El manifiesto cambió. Vuelve a cargar las extensiones.",
            ));
        }
        Ok(lock)
    }
}
fn copy_tree(
    source: &Path,
    target: &Path,
    depth: usize,
    limits: &mut (u64, usize),
) -> io::Result<()> {
    if depth > 16 {
        return Err(bad("El paquete tiene demasiados niveles de carpetas."));
    }
    plain(source)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        limits.1 += 1;
        if limits.1 > MAX_FILES {
            return Err(bad("El paquete contiene demasiados archivos."));
        }
        let from = entry.path();
        let to = target.join(entry.file_name());
        let metadata = plain(&from)?;
        if metadata.is_dir() {
            fs::create_dir(&to)?;
            copy_tree(&from, &to, depth + 1, limits)?;
        } else if metadata.is_file() {
            let remaining = MAX_PACKAGE_BYTES.saturating_sub(limits.0);
            let mut input = File::open(&from)?.take(remaining + 1);
            let mut output = File::create_new(&to)?;
            let bytes = io::copy(&mut input, &mut output)?;
            limits.0 = limits.0.saturating_add(bytes);
            if bytes > remaining {
                return Err(bad("El paquete supera 2 GiB."));
            }
            output.sync_all()?;
            fs::set_permissions(&to, metadata.permissions())?;
        } else {
            return Err(bad("El paquete contiene un archivo especial."));
        }
    }
    Ok(())
}
fn remove_tree(path: &Path) -> io::Result<()> {
    // No link is followed during cleanup, including links introduced after install.
    let metadata = plain(path)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            remove_tree(&entry?.path())?;
        }
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    }
}
