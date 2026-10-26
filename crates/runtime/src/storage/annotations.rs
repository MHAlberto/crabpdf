//! Local text marks are retained separately; exporting a PDF embeds real PDF annotations.
use crate::document::{ErrorKind, ReaderError, TextMarkup};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Store {
    schema: u32,
    prepared_signature: Option<String>,
    marks: Vec<TextMarkup>,
}

pub fn path(source: &Path) -> Option<PathBuf> {
    path_in(source, &crate::formats::data_directory())
}

pub fn path_in(source: &Path, data_directory: &Path) -> Option<PathBuf> {
    Some(data_directory.join("annotations").join(format!(
        "{:016x}.toml",
        crate::formats::fingerprint(source)?
    )))
}

pub fn load(
    path: &Path,
    signature: &str,
    legacy_pdf: bool,
) -> Result<Vec<TextMarkup>, ReaderError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = std::fs::File::open(path).map_err(error)?;
    if file.metadata().map_err(error)?.len() > 4 * 1024 * 1024 {
        return Err(error("Archivo de marcas demasiado grande."));
    }
    use std::io::Read;
    let mut data = String::new();
    file.take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut data)
        .map_err(error)?;
    if data.len() > 4 * 1024 * 1024 {
        return Err(error("Archivo de marcas demasiado grande."));
    }
    let store: Store = toml::from_str(&data).map_err(error)?;
    if store.schema > 2
        || (store.schema >= 2 && store.prepared_signature.as_deref() != Some(signature))
    {
        return Err(error(
            "Las marcas pertenecen a otra paginación; se conservaron sin aplicarse.",
        ));
    }
    if store.schema < 2 && !legacy_pdf && !store.marks.is_empty() {
        return Err(error(
            "Las marcas antiguas de este libro necesitan migración; se conservaron sin aplicarse.",
        ));
    }
    if store.marks.len() > 4096 {
        return Err(error("Demasiadas marcas en el documento."));
    }
    Ok(store.marks)
}

pub fn save(path: &Path, marks: &[TextMarkup], signature: &str) -> Result<(), ReaderError> {
    let parent = path
        .parent()
        .ok_or_else(|| error("Ruta de marcas inválida."))?;
    std::fs::create_dir_all(parent).map_err(error)?;
    let data = toml::to_string(&Store {
        schema: 2,
        prepared_signature: Some(signature.to_owned()),
        marks: marks.to_vec(),
    })
    .map_err(error)?;
    use std::io::Write;
    crate::fileops::atomic_write(path, |file| file.write_all(data.as_bytes())).map_err(error)
}

pub fn lock(path: &Path) -> Result<std::fs::File, ReaderError> {
    let parent = path
        .parent()
        .ok_or_else(|| error("Ruta de marcas inválida."))?;
    std::fs::create_dir_all(parent).map_err(error)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))
        .map_err(error)?;
    file.lock().map_err(error)?;
    Ok(file)
}

fn error(detail: impl std::fmt::Display) -> ReaderError {
    ReaderError::new(ErrorKind::File, "No se pudieron conservar las anotaciones.")
        .with_details(detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_book_marks_and_changed_pagination_are_preserved_without_misplacing_them() {
        let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join("artifacts")
            .join(format!(
                "annotation-schema-{}-{}.toml",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mark = TextMarkup {
            page: 0,
            start: 4,
            end: 8,
            kind: crate::document::MarkupKind::Highlight,
            color: [255, 230, 50],
        };
        let legacy = toml::to_string(&Store {
            schema: 0,
            prepared_signature: None,
            marks: vec![mark.clone()],
        })
        .unwrap();
        std::fs::write(&path, &legacy).unwrap();
        assert!(load(&path, "import-v4:a", false).is_err());
        assert_eq!(load(&path, "pdf:a", true).unwrap(), vec![mark.clone()]);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), legacy);
        save(&path, std::slice::from_ref(&mark), "import-v4:a").unwrap();
        assert_eq!(load(&path, "import-v4:a", false).unwrap(), vec![mark]);
        let saved = std::fs::read(&path).unwrap();
        assert!(load(&path, "import-v4:b", false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
    }
}
