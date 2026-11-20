//! Fetch and verify an explicitly selected package from the official catalog.
use super::{
    Repository,
    packages::{MAX_FILES, MAX_PACKAGE_BYTES, bad, identifier},
    target,
};
use crate::desktop::downloads::{StagedPackage, http_get, is_sha256, sha256};
use crabpdf_extension_api::{
    API_VERSION, Manifest,
    catalog::{
        CATALOG_ENDPOINT, CATALOG_VERSION, Catalog, CatalogAsset, CatalogEntry, MAX_CATALOG_BYTES,
        MAX_DOWNLOAD_BYTES, valid_code,
    },
};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read},
    path::Path,
};
use zip::ZipArchive;

pub const CATALOG_URL: &str = crabpdf_extension_api::catalog::CATALOG_URL;

pub struct CatalogPackage {
    pub entry: CatalogEntry,
    pub asset: CatalogAsset,
}

pub fn lookup(code: &str) -> io::Result<CatalogPackage> {
    if !valid_code(code) {
        return Err(bad("Escribe un código de seis cifras."));
    }
    let body = fetch()?;
    select(&body, code, &target(), &host_version()?)
}

fn host_version() -> io::Result<semver::Version> {
    semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(bad)
}

fn fetch() -> io::Result<Vec<u8>> {
    let mut body = Vec::new();
    let catalog_url = format!("{CATALOG_ENDPOINT}?crabpdf={}", env!("CARGO_PKG_VERSION"));
    http_get(&catalog_url, MAX_CATALOG_BYTES, "25", &mut body)
        .map_err(|error| bad(format!("No se pudo consultar el catálogo. {error}")))?;
    Ok(body)
}

pub fn updates(installed: Vec<(String, String)>) -> io::Result<Vec<CatalogPackage>> {
    available_updates(&fetch()?, &installed, &target(), &host_version()?)
}

fn available_updates(
    body: &[u8],
    installed: &[(String, String)],
    target: &str,
    host: &semver::Version,
) -> io::Result<Vec<CatalogPackage>> {
    let mut newest = std::collections::HashMap::new();
    for (id, version) in installed {
        let Ok(version) = semver::Version::parse(version) else {
            continue;
        };
        let current = newest.entry(id.as_str()).or_insert_with(|| version.clone());
        if version > *current {
            *current = version;
        }
    }
    let mut updates = Vec::new();
    for entry in parse_catalog(body)?.extensions {
        if let Some(current) = newest.get(entry.id.as_str())
            && semver::Version::parse(&entry.version).map_err(bad)? > *current
            && compatible_host(&entry, host)?
            && entry.assets.iter().any(|asset| asset.target == target)
        {
            updates.push(package_for(entry, target)?);
        }
    }
    Ok(updates)
}

fn parse_catalog(body: &[u8]) -> io::Result<Catalog> {
    let catalog: Catalog =
        serde_json::from_slice(body).map_err(|_| bad("El catálogo devolvió datos inválidos."))?;
    let mut codes = HashSet::new();
    let mut ids = HashSet::new();
    if catalog.schema != CATALOG_VERSION || catalog.extensions.len() > 128 {
        return Err(bad("El catálogo usa un formato incompatible."));
    }
    for entry in &catalog.extensions {
        if !valid_code(&entry.code)
            || !codes.insert(&entry.code)
            || !identifier(&entry.id)
            || !ids.insert(&entry.id)
            || entry.name.trim().is_empty()
            || entry.name.len() > 120
            || entry.description.len() > 2000
            || semver::Version::parse(&entry.version).is_err()
            || semver::VersionReq::parse(&entry.host_version).is_err()
        {
            return Err(bad(format!(
                "Ficha inválida o duplicada en el catálogo: {}.",
                entry.code
            )));
        }
        let mut targets = HashSet::new();
        if entry.assets.len() > 16 {
            return Err(bad("La ficha tiene demasiadas descargas."));
        }
        for asset in &entry.assets {
            if !targets.insert(&asset.target)
                || !(1..=MAX_DOWNLOAD_BYTES).contains(&asset.bytes)
                || !is_sha256(&asset.sha256)
            {
                return Err(bad("La ficha tiene descargas duplicadas o inválidas."));
            }
            package_url(asset)?;
        }
    }
    Ok(catalog)
}

fn compatible_host(entry: &CatalogEntry, host: &semver::Version) -> io::Result<bool> {
    Ok(entry.api_version == API_VERSION
        && semver::VersionReq::parse(&entry.host_version)
            .map_err(bad)?
            .matches(host))
}

fn select(
    body: &[u8],
    code: &str,
    target: &str,
    host: &semver::Version,
) -> io::Result<CatalogPackage> {
    let entry = parse_catalog(body)?
        .extensions
        .into_iter()
        .find(|entry| entry.code == code)
        .ok_or_else(|| bad(format!("No hay una extensión con el código {code}.")))?;
    if !compatible_host(&entry, host)? {
        return Err(bad(format!(
            "Esta extensión requiere CrabPDF {}.",
            entry.host_version
        )));
    }
    package_for(entry, target)
}

fn package_for(mut entry: CatalogEntry, target: &str) -> io::Result<CatalogPackage> {
    let index = entry
        .assets
        .iter()
        .position(|asset| asset.target == target)
        .ok_or_else(|| bad(format!("La extensión no está disponible para {target}.")))?;
    let asset = entry.assets.swap_remove(index);
    Ok(CatalogPackage { entry, asset })
}

fn package_url(asset: &CatalogAsset) -> io::Result<String> {
    let file = &asset.file;
    if file.len() > 160 || !file.strip_suffix(".zip").is_some_and(identifier) {
        return Err(bad("El nombre de archivo de la descarga es inválido."));
    }
    let base = match asset.release_tag.as_deref() {
        Some(tag) if identifier(tag) => {
            format!("https://github.com/MHAlberto/crabpdf/releases/download/{tag}/{file}")
        }
        None => format!("https://mhalberto.github.io/crabpdf/extensions/packages/{file}"),
        _ => return Err(bad("La etiqueta de la descarga es inválida.")),
    };
    Ok(format!(
        "{base}?sha256={}",
        asset.sha256.to_ascii_lowercase()
    ))
}

pub fn install(repository: &Repository, package: &CatalogPackage) -> io::Result<()> {
    let staging = StagedPackage::new("extension.zip")?;
    let mut output = File::create_new(&staging.file)?;
    let bytes = http_get(
        &package_url(&package.asset)?,
        package.asset.bytes,
        "300",
        &mut output,
    )
    .map_err(|error| bad(format!("No se pudo descargar la extensión. {error}")))?;
    drop(output);
    if bytes != package.asset.bytes
        || !sha256(&staging.file)?.eq_ignore_ascii_case(&package.asset.sha256)
    {
        return Err(bad("El tamaño o SHA-256 no coincide con el catálogo."));
    }
    repository.install_into(&package.entry.id, &package.entry.version, |folder| {
        unpack(File::open(&staging.file)?, folder)?;
        let manifest = repository.inspect(folder)?;
        verify_manifest(&manifest, package)?;
        // A ZIP from Windows may lack Unix permission bits.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if manifest.timer.is_none() {
                fs::set_permissions(
                    folder.join(&manifest.executable),
                    fs::Permissions::from_mode(0o700),
                )?;
            }
        }
        Ok(())
    })
}

fn verify_manifest(manifest: &Manifest, package: &CatalogPackage) -> io::Result<()> {
    let entry = &package.entry;
    if manifest.id != entry.id
        || manifest.version != entry.version
        || manifest.api_version != entry.api_version
        || manifest.target != package.asset.target
        || manifest.name != entry.name
        || manifest.description != entry.description
        || manifest.host_version != entry.host_version
    {
        return Err(bad(
            "El manifiesto descargado difiere de la ficha del catálogo.",
        ));
    }
    Ok(())
}

fn safe_component(component: &str) -> bool {
    if component.is_empty()
        || component.len() > 120
        || component.ends_with('.')
        || !component
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return false;
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !["COM", "LPT"].into_iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|suffix| suffix.len() == 1 && suffix.as_bytes()[0].is_ascii_digit())
        })
}

fn unpack(input: impl Read + io::Seek, root: &Path) -> io::Result<()> {
    let mut zip = ZipArchive::new(input).map_err(bad)?;
    if zip.len() > MAX_FILES {
        return Err(bad("El ZIP contiene demasiadas entradas."));
    }
    let mut remaining = MAX_PACKAGE_BYTES;
    let mut paths = HashSet::new();
    let mut directories = HashSet::new();
    let mut files = 0;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(bad)?;
        let name = entry
            .name()
            .strip_suffix('/')
            .unwrap_or_else(|| entry.name());
        if name.len() > 240
            || name.split('/').count() > if entry.is_dir() { 16 } else { 17 }
            || name.split('/').any(|component| !safe_component(component))
            || !paths.insert(name.to_ascii_lowercase())
            || entry.encrypted()
        {
            return Err(bad(
                "El ZIP contiene una ruta inválida, duplicada o cifrada.",
            ));
        }
        let mut directory = if entry.is_dir() {
            Some(name)
        } else {
            files += 1;
            name.rsplit_once('/').map(|(parent, _)| parent)
        };
        while let Some(parent) = directory {
            if !directories.insert(parent.to_owned()) {
                break;
            }
            directory = parent.rsplit_once('/').map(|(prefix, _)| prefix);
        }
        if files + directories.len() > MAX_FILES {
            return Err(bad(
                "La extracción supera el límite de archivos y carpetas.",
            ));
        }
        let mode = entry.unix_mode().unwrap_or_default();
        let file_type = mode & 0o170000;
        let path = root.join(name);
        if entry.is_dir() && matches!(file_type, 0 | 0o040000) {
            fs::create_dir_all(path)?;
            continue;
        }
        let size = entry.size();
        if !entry.is_file() || !matches!(file_type, 0 | 0o100000) || size > remaining {
            return Err(bad("El ZIP contiene archivos especiales o supera 2 GiB."));
        }
        fs::create_dir_all(path.parent().expect("archive path has a parent"))?;
        let output = File::create_new(&path)?;
        if io::copy(&mut entry.by_ref().take(size + 1), &mut &output)? != size {
            return Err(bad("El ZIP contiene un archivo de tamaño inesperado."));
        }
        output.sync_all()?;
        remaining -= size;
        #[cfg(unix)]
        if mode & 0o111 != 0 {
            use std::os::unix::fs::PermissionsExt;
            output.set_permissions(fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_selects_only_the_requested_code_and_compatible_target() {
        let host = semver::Version::new(0, 5, 1);
        let target = "x86_64-pc-windows-msvc";
        let data = serde_json::json!({
            "schema": 1,
            "extensions": [{
                "code": "100001", "id": "org.crabpdf.selection-info",
                "name": "Información de la selección", "version": "0.1.0",
                "description": "", "host_version": ">=0.5.0, <0.6.0", "api_version": 1,
                "assets": [{
                    "target": target, "file": "selection-info-win.zip", "bytes": 123,
                    "sha256": "ac".repeat(32)
                }]
            }]
        });
        let body = serde_json::to_vec(&data).unwrap();
        let found = select(&body, "100001", target, &host).unwrap();
        assert_eq!(found.asset.file, "selection-info-win.zip");
        assert!(select(&body, "100001", "i686-pc-windows-msvc", &host).is_err());
        assert!(select(&body, "100002", target, &host).is_err());
        assert!(select(&body, "100001", target, &semver::Version::new(0, 6, 0)).is_err());

        let installed = vec![("org.crabpdf.selection-info".into(), "0.0.1".into())];
        assert_eq!(
            available_updates(&body, &installed, target, &host)
                .unwrap()
                .len(),
            1
        );
        assert!(
            available_updates(&body, &installed, "i686-pc-windows-msvc", &host)
                .unwrap()
                .is_empty()
        );
        let mut wrong_schema = data.clone();
        wrong_schema["schema"] = 2.into();
        assert!(
            available_updates(
                &serde_json::to_vec(&wrong_schema).unwrap(),
                &installed,
                target,
                &host
            )
            .is_err()
        );

        let mut duplicate = data;
        let entries = duplicate["extensions"].as_array_mut().unwrap();
        entries.push(entries[0].clone());
        assert!(
            select(
                &serde_json::to_vec(&duplicate).unwrap(),
                "100001",
                target,
                &host
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "downloads the released example from the official catalog"]
    fn public_catalog_installs_without_activating() {
        let path = StagedPackage::new("profile").unwrap();
        let repository = Repository::new(&path.file);
        let package = lookup("100001").unwrap();
        install(&repository, &package).unwrap();
        let mut packages = repository.list().unwrap();
        assert_eq!(packages.len(), 1);
        let installed = packages.pop().unwrap();
        assert_eq!(installed.manifest.id, package.entry.id);
        assert!(!installed.enabled);
        assert!(install(&repository, &package).is_err());
        assert_eq!(repository.list().unwrap().len(), 1);
    }
}
