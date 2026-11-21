//! Local notebooks keyed by the original document's content.
use crate::{
    application::notebook::{self, Note},
    storage::fileops::atomic_write,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const SCHEMA: u32 = 1;
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_NOTES: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotebookKey(String);

/// Called from a disk worker. Hash the source, including for converted formats.
pub fn identify(path: &Path) -> io::Result<NotebookKey> {
    let mut file = File::open(path)?;
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(io::Error::other("El documento debe ser un archivo."));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut remaining = before.len();
    while remaining > 0 {
        let length = remaining.min(buffer.len() as u64) as usize;
        let read = file.read(&mut buffer[..length])?;
        if read == 0 {
            return Err(changed());
        }
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    let after = fs::metadata(path)?;
    let opened = file.metadata()?;
    if before.len() != after.len()
        || before.len() != opened.len()
        || before.modified()? != after.modified()?
        || before.modified()? != opened.modified()?
    {
        return Err(changed());
    }
    Ok(NotebookKey(format!("{:x}", digest.finalize())))
}

pub fn identify_open_document(path: &Path, fingerprint: Option<u64>) -> io::Result<NotebookKey> {
    let check = || fingerprint.is_some() && crate::source::fingerprint(path) == fingerprint;
    if !check() {
        return Err(changed());
    }
    let key = identify(path)?;
    if !check() {
        return Err(changed());
    }
    Ok(key)
}

fn changed() -> io::Error {
    io::Error::other("El documento cambió durante la lectura. Vuelve a abrirlo.")
}

#[derive(Serialize, Deserialize)]
struct Notebook {
    schema: u32,
    next_id: u64,
    notes: Vec<Note>,
}

impl Default for Notebook {
    fn default() -> Self {
        Self {
            schema: SCHEMA,
            next_id: 1,
            notes: Vec::new(),
        }
    }
}

pub enum Change {
    Add(Note),
    Edit { original: Note, replacement: Note },
    Delete(Note),
}

fn path(root: &Path, key: &NotebookKey) -> PathBuf {
    root.join("notebooks").join(format!("{}.json", key.0))
}

fn read(path: &Path) -> io::Result<Notebook> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Notebook::default()),
        Err(error) => return Err(error),
    };
    let mut data = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut data)?;
    if data.len() as u64 > MAX_BYTES {
        return Err(io::Error::other("El cuaderno supera el tamaño permitido."));
    }
    let book: Notebook = serde_json::from_slice(&data).map_err(io::Error::other)?;
    if book.schema != SCHEMA
        || book.next_id == 0
        || book.notes.len() > MAX_NOTES
        || book
            .notes
            .iter()
            .any(|note| !note.is_valid() || note.id == 0 || note.id >= book.next_id)
        || book.notes.windows(2).any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(io::Error::other(
            "El cuaderno tiene un formato desconocido o inválido. Se conserva en el perfil.",
        ));
    }
    Ok(book)
}

pub fn load(root: &Path, key: &NotebookKey) -> io::Result<Vec<Note>> {
    Ok(read(&path(root, key))?.notes)
}

pub fn update(root: &Path, key: &NotebookKey, change: Change) -> io::Result<Vec<Note>> {
    let path = path(root, key);
    fs::create_dir_all(path.parent().expect("notebook has a directory"))?;
    let _lock = acquire(&path.with_extension("lock"))?;
    // Always reconcile edits with the latest on-disk snapshot under the lock.
    let mut book = read(&path)?;
    match change {
        Change::Add(mut note) => {
            if !note.is_valid() || book.notes.len() >= MAX_NOTES {
                return Err(io::Error::other(
                    "La nota es inválida o el cuaderno está lleno.",
                ));
            }
            note.id = book.next_id;
            book.next_id = book
                .next_id
                .checked_add(1)
                .ok_or_else(|| io::Error::other("El cuaderno alcanzó su límite de entradas."))?;
            book.notes.push(note);
        }
        Change::Edit {
            original,
            replacement,
        } => {
            let index = current(&book.notes, &original)?;
            if replacement.id != original.id || !replacement.is_valid() {
                return Err(io::Error::other("La edición de la nota es inválida."));
            }
            book.notes[index] = replacement;
        }
        Change::Delete(original) => {
            let index = current(&book.notes, &original)?;
            book.notes.remove(index);
        }
    }
    let data = serde_json::to_vec(&book).map_err(io::Error::other)?;
    if data.len() as u64 > MAX_BYTES {
        return Err(io::Error::other(
            "El cuaderno alcanzó su límite de almacenamiento.",
        ));
    }
    atomic_write(&path, |output| output.write_all(&data))?;
    Ok(book.notes)
}

fn current(notes: &[Note], original: &Note) -> io::Result<usize> {
    notes
        .iter()
        .position(|note| note == original)
        .ok_or_else(|| {
            io::Error::other(
                "Otra ventana cambió esta entrada. Recarga el cuaderno para revisar el cambio.",
            )
        })
}

pub fn export(
    root: &Path,
    key: &NotebookKey,
    destination: &Path,
    document: &str,
    selected: Option<u64>,
) -> io::Result<()> {
    if !destination
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
        return Err(io::Error::other("El cuaderno se exporta a un archivo .md."));
    }
    let mut notes = load(root, key)?;
    if let Some(id) = selected {
        notes.retain(|note| note.id == id);
        if notes.is_empty() {
            return Err(io::Error::other(
                "La entrada ya no existe. Recarga el cuaderno.",
            ));
        }
    }
    let contents = notebook::markdown(document, &notes);
    atomic_write(destination, |output| output.write_all(contents.as_bytes()))
}

/// Existing mark files remain keyed by the legacy path for backwards compatibility.
pub fn pdf_mark_recovery(
    root: &Path,
    source: &Path,
    key: &NotebookKey,
    restore: bool,
) -> io::Result<MarkRecovery> {
    use crate::formats::Format;
    use crate::storage::annotations;
    if Format::detect(source).ok() != Some(Format::Pdf) {
        return Ok(MarkRecovery::Unavailable);
    }
    let identity = crate::source::fingerprint(source).ok_or_else(changed)?;
    let destination = root
        .join("annotations")
        .join(format!("{identity:016x}.toml"));
    let index = root.join("notebooks").join(format!("{}.marks", key.0));
    let legacy = match File::open(&index) {
        Ok(file) => {
            let mut text = String::new();
            file.take(17).read_to_string(&mut text)?;
            if text.len() != 16 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(io::Error::other(
                    "El vínculo de recuperación de marcas es inválido.",
                ));
            }
            Some(text)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };

    // A path-to-content link is created while the marked source is still available.
    if destination.exists() {
        if restore {
            return Err(io::Error::other(
                "El PDF abierto ya tiene un archivo local de marcas. Se conservó.",
            ));
        }
        if legacy.is_none()
            && !annotations::load(&destination, &format!("pdf:{identity:016x}"), true)
                .map_err(io::Error::other)?
                .is_empty()
        {
            fs::create_dir_all(index.parent().expect("mark index has a directory"))?;
            let _lock = acquire(&index.with_extension("marks-lock"))?;
            if !index.exists() {
                atomic_write(&index, |output| write!(output, "{identity:016x}"))?;
            }
        }
        return Ok(MarkRecovery::Unavailable);
    }
    let Some(legacy) = legacy else {
        return Ok(MarkRecovery::Unavailable);
    };
    let old = root.join("annotations").join(format!("{legacy}.toml"));
    let marks =
        annotations::load(&old, &format!("pdf:{legacy}"), true).map_err(io::Error::other)?;
    if marks.is_empty() {
        return Ok(MarkRecovery::Unavailable);
    }
    if !restore {
        return Ok(MarkRecovery::Available(marks.len()));
    }

    if &identify(source)? != key || crate::source::fingerprint(source) != Some(identity) {
        return Err(changed());
    }
    let _index_lock = acquire(&index.with_extension("marks-lock"))?;
    let _lock = acquire(&destination.with_extension("lock"))?;
    if destination.exists() {
        return Err(io::Error::other(
            "Otra ventana ya guardó marcas para este PDF.",
        ));
    }
    annotations::save(&destination, &marks, &format!("pdf:{identity:016x}"))
        .map_err(io::Error::other)?;
    atomic_write(&index, |output| write!(output, "{identity:016x}"))?;
    Ok(MarkRecovery::Restored(marks.len()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkRecovery {
    Unavailable,
    Available(usize),
    Restored(usize),
}

fn acquire(path: &Path) -> io::Result<File> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    lock.try_lock().map_err(|_| {
        io::Error::other("El archivo se está guardando en otra ventana. Vuelve a intentarlo.")
    })?;
    Ok(lock)
}

pub fn pdf_marks(root: &Path, source: &Path) -> io::Result<Vec<crate::document::TextMarkup>> {
    let identity = crate::source::fingerprint(source).ok_or_else(changed)?;
    let path = root
        .join("annotations")
        .join(format!("{identity:016x}.toml"));
    crate::storage::annotations::load(&path, &format!("pdf:{identity:016x}"), true)
        .map_err(io::Error::other)
}

pub fn edit_pdf_mark(
    root: &Path,
    source: &Path,
    key: &NotebookKey,
    mut expected: Vec<crate::document::TextMarkup>,
    index: usize,
    color: Option<[u8; 3]>,
) -> io::Result<()> {
    use crate::storage::annotations;
    if &identify(source)? != key {
        return Err(changed());
    }
    let identity = crate::source::fingerprint(source).ok_or_else(changed)?;
    let path = root
        .join("annotations")
        .join(format!("{identity:016x}.toml"));
    let signature = format!("pdf:{identity:016x}");
    let _lock = acquire(&path.with_extension("lock"))?;
    let actual = annotations::load(&path, &signature, true).map_err(io::Error::other)?;
    if expected != actual || index >= expected.len() {
        return Err(io::Error::other(
            "Las marcas cambiaron en otra ventana. Recarga la lista.",
        ));
    }
    if let Some(color) = color {
        expected[index].color = color;
    } else {
        expected.remove(index);
    }
    annotations::save(&path, &expected, &signature).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::notebook::NoteAnchor,
        document::{MarkupKind, TextMarkup},
    };
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEQUENCE: AtomicU32 = AtomicU32::new(0);

    fn fixture() -> PathBuf {
        let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(format!(
            "artifacts/notebook-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn note(comment: &str) -> Note {
        let mut note = Note::page(NoteAnchor::page(2));
        note.comment = comment.into();
        note
    }

    #[test]
    fn moved_original_keeps_the_notebook_and_different_bytes_keep_it_separate() {
        let root = fixture();
        let old = root.join("original.pdf");
        let moved = root.join("moved.pdf");
        fs::write(&old, b"same PDF bytes").unwrap();
        let key = identify(&old).unwrap();
        update(&root, &key, Change::Add(note("Acento y 日本語"))).unwrap();
        fs::rename(&old, &moved).unwrap();
        assert_eq!(identify(&moved).unwrap(), key);
        let entries = load(&root, &key).unwrap();
        assert_eq!(entries[0].comment, "Acento y 日本語");
        let output = root.join("cuaderno.md");
        export(&root, &key, &output, "Libro", Some(entries[0].id)).unwrap();
        assert!(fs::read_to_string(output).unwrap().contains("## Página 3"));
        fs::write(&moved, b"separate edition").unwrap();
        assert!(load(&root, &identify(&moved).unwrap()).unwrap().is_empty());
    }

    #[test]
    fn refuses_to_attach_to_a_replacement_of_an_open_pdf() {
        let root = fixture();
        let source = root.join("source.pdf");
        fs::write(&source, b"original PDF bytes").unwrap();
        let bound = crate::source::fingerprint(&source);
        assert!(identify_open_document(&source, bound).is_ok());
        fs::write(&source, b"different bytes after replacement").unwrap();
        assert!(identify_open_document(&source, bound).is_err());
    }

    #[test]
    fn distinct_edits_merge_and_stale_edits_preserve_the_latest_note() {
        let root = fixture();
        let source = root.join("source.pdf");
        fs::write(&source, b"same PDF bytes").unwrap();
        let key = identify(&source).unwrap();
        let original = update(&root, &key, Change::Add(note("a")))
            .unwrap()
            .remove(0);
        let after_add = update(&root, &key, Change::Add(note("b"))).unwrap();
        assert_eq!(after_add.len(), 2);
        let mut replacement = original.clone();
        replacement.comment = "a changed".into();
        update(
            &root,
            &key,
            Change::Edit {
                original: original.clone(),
                replacement,
            },
        )
        .unwrap();
        assert!(update(&root, &key, Change::Delete(original)).is_err());
        let final_notes = load(&root, &key).unwrap();
        assert_eq!(final_notes[0].comment, "a changed");
        assert_eq!(final_notes[1].comment, "b");
    }

    #[test]
    fn recovery_restores_registered_marks_for_identical_pdf_and_preserves_old_file() {
        let root = fixture();
        let source = root.join("source.pdf");
        let new = root.join("new.pdf");
        let unrelated = root.join("different.pdf");
        fs::write(&source, b"%PDF-1.7\nfixture").unwrap();
        fs::write(&unrelated, b"%PDF-1.7\nother document").unwrap();
        let original = crate::storage::annotations::path_in(&source, &root).unwrap();
        let signature = crate::formats::prepared_signature(&source, &source).unwrap();
        let marks = vec![TextMarkup {
            page: 1,
            start: 2,
            end: 3,
            kind: MarkupKind::Highlight,
            color: [255, 255, 0],
        }];
        crate::storage::annotations::save(&original, &marks, &signature).unwrap();
        let key = identify(&source).unwrap();
        pdf_mark_recovery(&root, &source, &key, false).unwrap();
        fs::rename(&source, &new).unwrap();
        assert_eq!(
            pdf_mark_recovery(&root, &unrelated, &identify(&unrelated).unwrap(), false).unwrap(),
            MarkRecovery::Unavailable
        );
        assert_eq!(
            pdf_mark_recovery(&root, &new, &key, false).unwrap(),
            MarkRecovery::Available(1)
        );
        assert!(pdf_mark_recovery(&root, &unrelated, &key, true).is_err());
        assert_eq!(
            pdf_mark_recovery(&root, &new, &key, true).unwrap(),
            MarkRecovery::Restored(1)
        );
        let restored = crate::storage::annotations::path_in(&new, &root).unwrap();
        let signature_new = crate::formats::prepared_signature(&new, &new).unwrap();
        assert_eq!(
            crate::storage::annotations::load(&restored, &signature_new, true).unwrap(),
            marks
        );
        assert!(original.is_file());
    }
}
