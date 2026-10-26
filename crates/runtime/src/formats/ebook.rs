//! CHM/Kindle resource reconstruction before layout conversion.
use super::{
    MAX_ENTRIES, archive_entries, convert, extension, extract_entry, import_error, io_error,
    limit_error, natural_cmp, run, safe_relative,
};
use crate::document::ReaderError;
use std::{
    fs,
    path::{Path, PathBuf},
};
pub(super) fn import_chm(source: &Path, output: &Path, work: &Path) -> Result<(), ReaderError> {
    let entries = archive_entries(source, work)?;
    let resources = work.join("help");
    fs::create_dir(&resources).map_err(io_error)?;
    let mut html = Vec::new();
    let mut contents = Vec::new();
    for entry in entries
        .iter()
        .filter(|entry| !entry.directory && entry.size > 0)
    {
        let relative = safe_relative(&entry.name, true)?;
        if relative
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(['#', '$']))
        {
            continue;
        }
        let path = resources.join(relative);
        extract_entry(source, entry, &path, work)?;
        match extension(&path).as_str() {
            "html" | "htm" | "xhtml" => html.push(path),
            "hhc" => contents.push(path),
            _ => {}
        }
    }
    html.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
    // The CHM table of contents defines chapter order. A fallback includes unlisted HTML topics.
    let mut ordered = Vec::new();
    for content in contents {
        let text = String::from_utf8_lossy(&fs::read(&content).map_err(io_error)?).into_owned();
        for tag in text
            .split('<')
            .filter_map(|tag| tag.split_once('>').map(|(tag, _)| tag))
        {
            if !tag.to_lowercase().starts_with("param ") {
                continue;
            }
            let attributes = attributes(tag);
            if attributes
                .iter()
                .any(|(key, value)| key == "name" && value.eq_ignore_ascii_case("local"))
                && let Some((_, local)) = attributes.iter().find(|(key, _)| key == "value")
            {
                let relative = safe_relative(local.split('#').next().unwrap_or(local), true)?;
                let path = resources.join(relative);
                if html.contains(&path) && !ordered.contains(&path) {
                    ordered.push(path);
                }
            }
        }
    }
    for path in html {
        if !ordered.contains(&path) {
            ordered.push(path);
        }
    }
    convert(&ordered, output, work)
}

pub(super) fn make_epub(
    opf: &Path,
    files: &[PathBuf],
    epub: &Path,
    work: &Path,
) -> Result<(), ReaderError> {
    let parent = opf
        .parent()
        .ok_or_else(|| import_error("Paquete Kindle inválido."))?;
    let package = work.join("epub");
    fs::create_dir_all(package.join("META-INF")).map_err(io_error)?;
    fs::write(package.join("mimetype"), "application/epub+zip").map_err(io_error)?;
    let opf_name = opf
        .file_name()
        .ok_or_else(|| import_error("Paquete Kindle inválido."))?
        .to_string_lossy();
    fs::write(package.join("META-INF/container.xml"), format!("<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/{opf_name}\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>")).map_err(io_error)?;
    for file in files {
        if let Ok(relative) = file.strip_prefix(parent) {
            let target = package.join("OEBPS").join(relative);
            if let Some(dir) = target.parent() {
                fs::create_dir_all(dir).map_err(io_error)?;
            }
            fs::copy(file, target).map_err(io_error)?;
        }
    }
    run(
        "7z",
        &[
            "a".into(),
            "-tzip".into(),
            "-mx=0".into(),
            epub.as_os_str().into(),
            "mimetype".into(),
            "META-INF".into(),
            "OEBPS".into(),
        ],
        &package,
        None,
    )
}

fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let bytes = tag.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && (bytes[index].is_ascii_whitespace() || bytes[index] == b'/') {
            index += 1;
        }
        let start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        let key = tag[start..index].to_lowercase();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let quote = bytes
            .get(index)
            .copied()
            .filter(|byte| *byte == b'\'' || *byte == b'"');
        if quote.is_some() {
            index += 1;
        }
        let start = index;
        while index < bytes.len()
            && quote.map_or(!bytes[index].is_ascii_whitespace(), |quote| {
                bytes[index] != quote
            })
        {
            index += 1;
        }
        fields.push((key, tag[start..index].replace("&amp;", "&")));
        if quote.is_some() && index < bytes.len() {
            index += 1;
        }
    }
    fields
}
pub(super) fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), ReaderError> {
    for entry in fs::read_dir(directory).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let metadata = entry.metadata().map_err(io_error)?;
        if metadata.is_file() {
            files.push(entry.path());
        } else if metadata.is_dir() {
            collect_files(&entry.path(), files)?;
        }
        if files.len() > MAX_ENTRIES {
            return Err(limit_error());
        }
    }
    Ok(())
}
