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
    let mut body = Vec::new();
    http_get(CATALOG_ENDPOINT, MAX_CATALOG_BYTES, "25", &mut body)
        .map_err(|error| bad(format!("No se pudo consultar el catálogo. {error}")))?;
    select(
        &body,
        code,
        &target(),
        &semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(bad)?,
    )
}

fn select(
    body: &[u8],
    code: &str,
    target: &str,
    host: &semver::Version,
) -> io::Result<CatalogPackage> {
    let catalog: Catalog =
        serde_json::from_slice(body).map_err(|_| bad("El catálogo devolvió datos inválidos."))?;
    let mut codes = HashSet::new();
    if catalog.schema != CATALOG_VERSION
        || catalog.extensions.len() > 128
        || catalog
            .extensions
            .iter()
            .any(|entry| !valid_code(&entry.code) || !codes.insert(&entry.code))
    {
        return Err(bad(
            "El catálogo usa un formato incompatible o tiene códigos duplicados.",
        ));
    }
    let mut entry = catalog
        .extensions
        .into_iter()
        .find(|entry| entry.code == code)
        .ok_or_else(|| bad(format!("No hay una extensión con el código {code}.")))?;
    if !identifier(&entry.id)
        || entry.name.trim().is_empty()
        || entry.name.len() > 120
        || entry.description.len() > 2000
        || semver::Version::parse(&entry.version).is_err()
    {
        return Err(bad("La ficha de la extensión es inválida."));
    }
    if entry.api_version != API_VERSION
        || !semver::VersionReq::parse(&entry.host_version)
            .map_err(bad)?
            .matches(host)
    {
        return Err(bad(format!(
            "Esta extensión requiere CrabPDF {}.",
            entry.host_version
        )));
    }
    let mut targets = HashSet::new();
    if entry.assets.len() > 16
        || entry
            .assets
            .iter()
            .any(|asset| !targets.insert(&asset.target))
    {
        return Err(bad("La ficha tiene descargas duplicadas o inválidas."));
    }
    let index = entry
        .assets
        .iter()
        .position(|asset| asset.target == target)
        .ok_or_else(|| bad(format!("La extensión no está disponible para {target}.")))?;
    let asset = entry.assets.swap_remove(index);
    if !(1..=MAX_DOWNLOAD_BYTES).contains(&asset.bytes) || !is_sha256(&asset.sha256) {
        return Err(bad("La descarga carece de tamaño o SHA-256 válido."));
    }
    package_url(&asset)?;
    Ok(CatalogPackage { entry, asset })
}

fn package_url(asset: &CatalogAsset) -> io::Result<String> {
    let file = &asset.file;
    if file.len() > 160 || !file.strip_suffix(".zip").is_some_and(identifier) {
        return Err(bad("El nombre de archivo de la descarga es inválido."));
    }
    match asset.release_tag.as_deref() {
        Some(tag) if identifier(tag) => Ok(format!(
            "https://github.com/MHAlberto/crabpdf/releases/download/{tag}/{file}"
        )),
        None => Ok(format!(
            "https://mhalberto.github.io/crabpdf/extensions/packages/{file}"
        )),
        _ => Err(bad("La etiqueta de la descarga es inválida.")),
    }
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
    let folder = staging.file.with_file_name("package");
    fs::create_dir(&folder)?;
    unpack(File::open(&staging.file)?, &folder)?;
    let manifest = repository.inspect(&folder)?;
    verify_manifest(&manifest, package)?;
    // A ZIP from Windows may lack Unix permission bits.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            folder.join(&manifest.executable),
            fs::Permissions::from_mode(0o700),
        )?;
    }
    repository.install(&folder)
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
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(bad)?;
        let name = entry
            .name()
            .strip_suffix('/')
            .unwrap_or_else(|| entry.name());
        if name.len() > 240
            || name.split('/').count() > 17
            || name.split('/').any(|component| !safe_component(component))
            || !paths.insert(name.to_ascii_lowercase())
            || entry.encrypted()
        {
            return Err(bad(
                "El ZIP contiene una ruta inválida, duplicada o cifrada.",
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
