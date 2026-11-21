//! Folder packages, immutable versions and per-package atomic activation.
use crate::fileops::atomic_write;
use crabpdf_extension_api::{API_VERSION, ExtensionRuntime, Manifest};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

pub(super) const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub(super) const MAX_FILES: usize = 4096;
const MAX_INSTALLED: usize = 256;
const MAX_VERSIONS: usize = 64;

#[derive(Default)]
pub struct Inventory {
    pub packages: Vec<InstalledPackage>,
    pub issues: Vec<PackageIssue>,
    pub over_limit: bool,
}

pub struct PackageIssue {
    pub id: String,
    pub version: Option<String>,
    pub detail: String,
}

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
pub(super) fn bad(message: impl std::fmt::Display) -> io::Error {
    io::Error::other(message.to_string())
}
pub(super) fn identifier(value: &str) -> bool {
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
    pub fn has_installed(&self) -> bool {
        fs::read_dir(self.root.join("installed")).is_ok_and(|mut entries| entries.next().is_some())
    }

    fn lock(&self) -> io::Result<File> {
        fs::create_dir_all(&self.root)?;
        plain(&self.root)?;
        let file = open_lock(&self.root.join("packages.lock"))?;
        file.lock()?;
        Ok(file)
    }

    fn package_lock(&self, id: &str) -> io::Result<File> {
        if !identifier(id) {
            return Err(bad("Identificador de paquete inválido."));
        }
        let locks = self.root.join("locks");
        fs::create_dir_all(&locks)?;
        plain(&locks)?;
        let file = open_lock(&locks.join(format!("{id}.lock")))?;
        file.try_lock().map_err(|_| {
            bad("La extensión está en uso. Vuelve a intentarlo al terminar la operación.")
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
            || (manifest.commands.is_empty() && manifest.timer.is_none())
            || manifest.commands.len() > 16
        {
            return Err(bad("Identidad, API o comandos incompatibles."));
        }
        let version = semver::Version::parse(&manifest.version).map_err(bad)?;
        if version.to_string() != manifest.version {
            return Err(bad("Versión de paquete inválida."));
        }
        let host = semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(bad)?;
        let host_requirement = semver::VersionReq::parse(&manifest.host_version).map_err(bad)?;
        if !host_requirement.matches(&host) {
            return Err(bad(format!(
                "La extensión requiere CrabPDF {} y esta versión es {}.",
                manifest.host_version, host
            )));
        }
        let current_target = target();
        if manifest.target != current_target {
            return Err(bad(format!(
                "La extensión se compiló para {} y este CrabPDF usa {}.",
                manifest.target, current_target
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
        if let Some(timer) = manifest.timer {
            if !manifest.commands.is_empty()
                || !manifest.executable.is_empty()
                || !(1..=180).contains(&timer.focus_minutes)
                || !(1..=60).contains(&timer.break_minutes)
            {
                return Err(bad("La configuración del temporizador es inválida."));
            }
            return Ok(manifest);
        }
        let executable = safe_relative(&manifest.executable)?;
        if manifest.runtime == ExtensionRuntime::Wasi
            && executable.extension().and_then(|ext| ext.to_str()) != Some("wasm")
        {
            return Err(bad(
                "Las extensiones WASI deben indicar un ejecutable .wasm.",
            ));
        }
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
    pub(super) fn model_data(&self, id: &str) -> io::Result<PathBuf> {
        if !identifier(id) {
            return Err(bad("Identificador de paquete inválido."));
        }
        let data = self.root.join("data");
        fs::create_dir_all(&data)?;
        plain(&data)?;
        let data = data.join(id);
        fs::create_dir_all(&data)?;
        plain(&data)?;
        fs::canonicalize(data)
    }

    fn activation(&self, id: &str) -> PathBuf {
        self.root.join("enabled").join(format!("{id}.json"))
    }
    fn active_version(&self, id: &str) -> io::Result<Option<String>> {
        let path = self.activation(id);
        match plain(path.parent().expect("activation has a parent")) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => {
                result?;
            }
        }
        match plain(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => {
                result?;
            }
        }
        let mut bytes = Vec::new();
        File::open(&path)?.take(257).read_to_end(&mut bytes)?;
        if bytes.len() > 256 {
            return Err(bad("Registro de activación inválido."));
        }
        let version: String = serde_json::from_slice(&bytes).map_err(bad)?;
        semver::Version::parse(&version).map_err(bad)?;
        Ok(Some(version))
    }

    pub fn list(&self) -> io::Result<Vec<InstalledPackage>> {
        Ok(self.inventory()?.packages)
    }

    pub fn inventory(&self) -> io::Result<Inventory> {
        let _lock = self.lock()?;
        let mut inventory = Inventory::default();
        let mut slots = 0;
        for (index, group) in self.groups()?.into_iter().enumerate() {
            if index == MAX_INSTALLED || slots == MAX_INSTALLED {
                inventory.over_limit = true;
                break;
            }
            let Some(id) = group.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !identifier(id) {
                continue;
            }
            let versions = match plain(&group).and_then(|_| fs::read_dir(&group)) {
                Ok(versions) => versions,
                Err(error) => {
                    inventory.issues.push(PackageIssue {
                        id: id.into(),
                        version: None,
                        detail: error.to_string(),
                    });
                    continue;
                }
            };
            let active = self.active_version(id).unwrap_or_else(|error| {
                inventory.issues.push(PackageIssue {
                    id: id.into(),
                    version: None,
                    detail: format!(
                        "Registro de activación dañado. Pulsa Activar para repararlo. {error}"
                    ),
                });
                None
            });
            for (index, entry) in versions.enumerate() {
                if index == MAX_VERSIONS || slots == MAX_INSTALLED {
                    inventory.over_limit = true;
                    break;
                }
                slots += 1;
                let entry = entry?;
                let path = entry.path();
                let Some(version) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                let inspected = self.inspect(&path).and_then(|manifest| {
                    if manifest.id != id || manifest.version != version {
                        return Err(bad("La identidad del paquete difiere de su carpeta."));
                    }
                    Ok(manifest)
                });
                match inspected {
                    Ok(manifest) => inventory.packages.push(InstalledPackage {
                        manifest,
                        enabled: active.as_deref() == Some(version),
                        directory: path,
                    }),
                    Err(error) => inventory.issues.push(PackageIssue {
                        id: id.into(),
                        version: semver::Version::parse(version).ok().map(|_| version.into()),
                        detail: format!("{version}: {error}"),
                    }),
                }
            }
        }
        inventory.packages.sort_by(|a, b| {
            (&a.manifest.id, &a.manifest.version).cmp(&(&b.manifest.id, &b.manifest.version))
        });
        Ok(inventory)
    }

    fn groups(&self) -> io::Result<Vec<PathBuf>> {
        let installed = self.root.join("installed");
        match plain(&installed) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            result => {
                result?;
                fs::read_dir(installed)?
                    .take(MAX_INSTALLED + 1)
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect()
            }
        }
    }

    pub fn installed_ids(&self) -> io::Result<std::collections::HashSet<String>> {
        Ok(self
            .groups()?
            .into_iter()
            .filter_map(|path| {
                if !plain(&path).is_ok_and(|metadata| metadata.is_dir())
                    || fs::read_dir(&path).ok()?.next().is_none()
                {
                    return None;
                }
                Some(path.file_name()?.to_str()?.to_owned())
            })
            .collect())
    }

    pub fn is_active(&self, package: &InstalledPackage) -> bool {
        self.active_version(&package.manifest.id)
            .ok()
            .flatten()
            .as_deref()
            == Some(&package.manifest.version)
            && self
                .package_dir(&package.manifest.id, &package.manifest.version)
                .and_then(|path| plain(&path))
                .is_ok_and(|metadata| metadata.is_dir())
    }

    fn check_capacity(&self, parent: &Path) -> io::Result<()> {
        let mut groups = self.groups()?;
        // A pre-existing failed install may have left an empty group.
        while groups.len() >= MAX_INSTALLED {
            let removed = groups
                .iter()
                .filter(|group| {
                    plain(group).is_ok_and(|metadata| metadata.is_dir())
                        && fs::remove_dir(group).is_ok()
                })
                .count();
            if removed == 0 {
                break;
            }
            groups = self.groups()?;
        }
        if groups.len() > MAX_INSTALLED
            || (groups.len() == MAX_INSTALLED && !groups.iter().any(|group| group == parent))
        {
            return Err(bad("Se alcanzó el límite de extensiones instaladas."));
        }
        let mut slots = 0;
        for group in groups {
            // Broken entries still occupy space. Never follow a link while counting.
            let count = if plain(&group).is_ok_and(|metadata| metadata.is_dir()) {
                fs::read_dir(&group)?.take(MAX_INSTALLED).count()
            } else {
                1
            };
            if group == parent && count >= MAX_VERSIONS {
                return Err(bad("Se alcanzó el límite de versiones de esta extensión."));
            }
            slots += count;
            if slots >= MAX_INSTALLED {
                return Err(bad("Se alcanzó el límite de extensiones instaladas."));
            }
        }
        Ok(())
    }

    pub fn install(&self, source: &Path) -> io::Result<()> {
        plain(source)?;
        let source = fs::canonicalize(source)?;
        let manifest = self.inspect(&source)?;
        self.install_reviewed(&source, &manifest)
    }

    /// Recheck the displayed manifest against the staged copy before publishing.
    pub(crate) fn install_reviewed(&self, source: &Path, manifest: &Manifest) -> io::Result<()> {
        plain(source)?;
        let source = fs::canonicalize(source)?;
        self.install_into(&manifest.id, &manifest.version, |stage| {
            let mut limits = (0_u64, 0_usize);
            copy_tree(&source, stage, 0, &mut limits)?;
            if self.inspect(stage)? != *manifest {
                return Err(bad(
                    "El manifiesto cambió después de revisarlo. Vuelve a abrir la carpeta.",
                ));
            }
            Ok(())
        })
    }

    /// Populate the final filesystem's staging area directly, then publish by
    /// rename. Only final placement and capacity accounting require the lock.
    pub(super) fn install_into(
        &self,
        id: &str,
        version: &str,
        populate: impl FnOnce(&Path) -> io::Result<()>,
    ) -> io::Result<()> {
        {
            let _lock = self.lock()?;
            self.available_destination(id, version)?;
        }
        let stage = self
            .root
            .join(format!("staging-{:016x}", getrandom::u64().map_err(bad)?));
        fs::create_dir(&stage)?;
        let result = (|| {
            populate(&stage)?;
            let staged = self.inspect(&stage)?;
            if staged.id != id || staged.version != version {
                return Err(bad(
                    "La identidad del paquete cambió durante la instalación.",
                ));
            }
            let _lock = self.lock()?;
            let destination = self.available_destination(id, version)?;
            let installed = self.root.join("installed");
            fs::create_dir_all(&installed)?;
            plain(&installed)?;
            let parent = destination.parent().expect("package has a parent");
            let created = match fs::create_dir(parent) {
                Ok(()) => true,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
                Err(error) => return Err(error),
            };
            let renamed = plain(parent).and_then(|_| fs::rename(&stage, &destination));
            if renamed.is_err() && created {
                let _ = fs::remove_dir(parent);
            }
            renamed
        })();
        if stage.exists() {
            let _ = remove_tree(&stage);
        }
        result
    }

    fn available_destination(&self, id: &str, version: &str) -> io::Result<PathBuf> {
        let destination = self.package_dir(id, version)?;
        match fs::symlink_metadata(&destination) {
            Ok(_) => {
                let message = match self.inspect(&destination) {
                    Ok(installed) if installed.id == id && installed.version == version => {
                        "Esta versión ya está instalada."
                    }
                    _ => {
                        "Desinstala la copia dañada de esta versión en el gestor antes de reinstalarla."
                    }
                };
                return Err(bad(message));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let parent = destination.parent().expect("package has a parent");
        self.check_capacity(parent)?;
        Ok(destination)
    }

    pub fn enable(&self, package: &InstalledPackage, enabled: bool) -> io::Result<()> {
        let _lock = self.lock()?;
        let _package_lock = self.package_lock(&package.manifest.id)?;
        let path = self.package_dir(&package.manifest.id, &package.manifest.version)?;
        let manifest = self.inspect(&path)?;
        if manifest.id != package.manifest.id || manifest.version != package.manifest.version {
            return Err(bad("La identidad del paquete difiere de su carpeta."));
        }
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
        self.remove_version(&package.manifest.id, &package.manifest.version)
    }

    pub fn remove_version(&self, id: &str, version: &str) -> io::Result<()> {
        let _lock = self.lock()?;
        let _package_lock = self.package_lock(id)?;
        let directory = self.package_dir(id, version)?;
        let metadata = plain(&directory)?;
        if metadata.is_dir() {
            fs::remove_dir_all(&directory)?;
        } else if metadata.is_file() {
            fs::remove_file(&directory)?;
        } else {
            return Err(bad("La ruta instalada no corresponde a un paquete."));
        }
        if self.active_version(id).ok().flatten().as_deref() == Some(version) {
            fs::remove_file(self.activation(id))?;
        }
        let group = directory.parent().expect("package has a parent");
        if fs::read_dir(group)?.next().is_none() {
            fs::remove_dir(group)?;
        }
        Ok(())
    }

    /// Keep this package active and its data reserved until its process exits.
    pub(super) fn acquire(&self, package: &InstalledPackage) -> io::Result<File> {
        let _inventory = self.lock()?;
        let lock = self.package_lock(&package.manifest.id)?;
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
fn open_lock(path: &Path) -> io::Result<File> {
    match plain(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        result => {
            result?;
        }
    }
    File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
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
