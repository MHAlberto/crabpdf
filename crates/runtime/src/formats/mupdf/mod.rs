//! MuPDF conversion bridge. All scheduling, validation and publication remain Rust.
use super::{
    ImportInfo, SERIAL, extension, import_error, io_error, limit_error, read_bounded, run,
};
use crate::document::ReaderError;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::Ordering as AtomicOrdering,
};
pub(super) fn convert(inputs: &[PathBuf], output: &Path, work: &Path) -> Result<(), ReaderError> {
    let mut parts = Vec::new();
    for (index, batch) in inputs.chunks(32).enumerate() {
        let part = work.join(format!("part-{index:05}.pdf"));
        let mut args: Vec<OsString> = [
            "convert", "-F", "pdf", "-W", "612", "-H", "792", "-S", "12", "-o",
        ]
        .map(OsString::from)
        .into();
        args.push(part.as_os_str().into());
        args.extend(batch.iter().map(|path| path.as_os_str().into()));
        run("mutool", &args, work, None)?;
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(import_error("El documento no contiene páginas legibles."));
    }
    while parts.len() > 1 {
        let mut merged = Vec::new();
        for (index, batch) in parts.chunks(32).enumerate() {
            let part = work.join(format!(
                "merge-{}-{index}.pdf",
                SERIAL.fetch_add(1, AtomicOrdering::Relaxed)
            ));
            let mut args = vec!["merge".into(), "-o".into(), part.as_os_str().into()];
            args.extend(batch.iter().map(|path| path.as_os_str().into()));
            run("mutool", &args, work, None)?;
            merged.push(part);
        }
        for part in parts {
            fs::remove_file(part).map_err(io_error)?;
        }
        parts = merged;
    }
    fs::rename(&parts[0], output).map_err(io_error)?;
    if inputs.iter().all(|path| {
        matches!(
            extension(path).as_str(),
            "epub" | "fb2" | "html" | "htm" | "xhtml"
        )
    }) {
        // A failed metadata lookup never prevents reading the rendered document.
        let _ = extract_import_info(inputs, output, work);
    }
    Ok(())
}

fn extract_import_info(inputs: &[PathBuf], output: &Path, work: &Path) -> Result<(), ReaderError> {
    let script = work.join("import-info.js");
    fs::write(&script, include_str!("import_info.js")).map_err(io_error)?;
    let mut info = ImportInfo::default();
    for batch in inputs.chunks(32) {
        let mut args = vec!["run".into(), script.as_os_str().into()];
        args.extend(batch.iter().map(|path| path.as_os_str().into()));
        run("mutool", &args, work, None)?;
        let part: ImportInfo =
            serde_json::from_slice(&read_bounded(&work.join("tool-out.log"), 1024 * 1024)?)
                .map_err(io_error)?;
        if info.title.is_empty() {
            info.title = part.title;
            info.author = part.author;
        }
        for mut item in part.outline {
            item.page = item.page.map(|page| page + info.pages);
            if info.outline.len() < 10_000 {
                info.outline.push(item);
            }
        }
        info.pages += part.pages;
    }
    let bytes = serde_json::to_vec(&info).map_err(io_error)?;
    if bytes.len() > 1024 * 1024 {
        return Err(limit_error());
    }
    fs::write(output.with_extension("json"), bytes).map_err(io_error)
}
