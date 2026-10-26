//! Archive listing and extraction with path, encryption and size checks.
use super::{
    Format, MAX_ENTRIES, MAX_ENTRY, MAX_FILE, import_error, io_error, limit_error, read_bounded,
    run,
};
use crate::document::{ErrorKind, ReaderError};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
#[derive(Debug)]
pub(super) struct Entry {
    pub(super) name: String,
    pub(super) size: u64,
    pub(super) directory: bool,
}
pub(super) fn archive_entries(source: &Path, work: &Path) -> Result<Vec<Entry>, ReaderError> {
    run(
        "7z",
        &[
            "l".into(),
            "-slt".into(),
            "-ba".into(),
            "-sccUTF-8".into(),
            "-p-".into(),
            source.as_os_str().into(),
        ],
        work,
        None,
    )?;
    let text = String::from_utf8(read_bounded(&work.join("tool-out.log"), 8 * 1024 * 1024)?)
        .map_err(io_error)?;
    parse_entries(&text, Format::detect(source)? == Format::Chm)
}
pub(super) fn parse_entries(text: &str, chm: bool) -> Result<Vec<Entry>, ReaderError> {
    let mut entries = Vec::new();
    let mut total: u64 = 0;
    for block in text.replace("\r\n", "\n").split("\n\n") {
        let fields: Vec<_> = block
            .lines()
            .filter_map(|line| line.split_once(" = "))
            .collect();
        let field = |name| {
            fields
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| *value)
        };
        let Some(name) = field("Path") else {
            continue;
        };
        if field("Encrypted") == Some("+") {
            return Err(ReaderError::new(
                ErrorKind::Protected,
                "El archivo está protegido con contraseña.",
            ));
        }
        if field("Symbolic Link").is_some_and(|link| !link.is_empty())
            || field("Hard Link").is_some_and(|link| !link.is_empty())
        {
            return Err(import_error("El archivo contiene enlaces no compatibles."));
        }
        let relative = safe_relative(name, chm)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let size = field("Size")
            .unwrap_or("0")
            .parse::<u64>()
            .map_err(io_error)?;
        total = total.saturating_add(size);
        if size > MAX_ENTRY || total > MAX_FILE || entries.len() >= MAX_ENTRIES {
            return Err(limit_error());
        }
        entries.push(Entry {
            name: name.to_owned(),
            size,
            directory: field("Folder") == Some("+")
                || field("Attributes").is_some_and(|value| value.starts_with('D')),
        });
    }
    Ok(entries)
}
pub(super) fn safe_relative(name: &str, chm: bool) -> Result<PathBuf, ReaderError> {
    let name = name.replace('\\', "/");
    let name = if chm {
        name.trim_start_matches('/')
    } else {
        &name
    };
    if name.contains(':')
        || name.contains('\0')
        || name.starts_with('/')
        || name.split('/').any(|part| part == "..")
    {
        return Err(import_error("El archivo contiene una ruta insegura."));
    }
    let path = PathBuf::from(name);
    if path
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(import_error("Ruta del archivo no compatible."));
    }
    Ok(path)
}
pub(super) fn extract_entry(
    source: &Path,
    entry: &Entry,
    output: &Path,
    work: &Path,
) -> Result<(), ReaderError> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    run(
        "7z",
        &[
            "e".into(),
            "-so".into(),
            "-spd".into(),
            "-y".into(),
            "-p-".into(),
            source.as_os_str().into(),
            entry.name.clone().into(),
        ],
        work,
        Some(output),
    )?;
    if output.metadata().map_err(io_error)?.len() != entry.size {
        return Err(import_error(format!(
            "No se pudo extraer íntegramente {}: se esperaban {} bytes, se obtuvieron {}.",
            entry.name,
            entry.size,
            output.metadata().map_err(io_error)?.len()
        )));
    }
    Ok(())
}
