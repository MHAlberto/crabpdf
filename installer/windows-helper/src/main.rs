use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    fs::{self, File, FileType, Metadata},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

const MARKER: &str = "crabpdf-install.ini";
const MARKER_LIMIT: u64 = 16 * 1024;
const MANIFEST_LIMIT: u64 = 2 * 1024 * 1024;

#[derive(Deserialize)]
struct Manifest {
    files: Vec<ManagedFile>,
}

#[derive(Deserialize)]
struct ManagedFile {
    path: String,
    bytes: u64,
    sha256: String,
}

struct Installation {
    directory: PathBuf,
    backup: PathBuf,
    journal: PathBuf,
    architecture: &'static str,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn read_small(path: &Path, limit: u64) -> io::Result<String> {
    let metadata = plain(path)?.ok_or(io::ErrorKind::NotFound)?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(invalid(format!(
            "Archivo de registro inválido: {}",
            path.display()
        )));
    }
    let mut text = String::new();
    File::open(path)?
        .take(limit + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err(invalid(
            "El archivo de registro supera el tamaño permitido.",
        ));
    }
    Ok(text)
}

fn plain(path: &Path) -> io::Result<Option<Metadata>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    #[cfg(windows)]
    let is_redirect = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT
    };
    #[cfg(not(windows))]
    let is_redirect = metadata.file_type().is_symlink();
    if is_redirect {
        return Err(invalid(format!(
            "Ruta de instalación redirigida: {}",
            path.display()
        )));
    }
    Ok(Some(metadata))
}

fn visit(
    directory: &Path,
    mut entry: impl FnMut(&Path, &Metadata) -> io::Result<()>,
) -> io::Result<()> {
    let mut directories = vec![directory.to_owned()];
    while let Some(parent) = directories.pop() {
        for child in fs::read_dir(parent)? {
            let path = child?.path();
            let metadata = plain(&path)?.ok_or(io::ErrorKind::NotFound)?;
            entry(&path, &metadata)?;
            if metadata.is_dir() {
                directories.push(path);
            }
        }
    }
    Ok(())
}

fn same_kind(a: FileType, b: FileType) -> bool {
    (a.is_dir() && b.is_dir()) || (a.is_file() && b.is_file())
}

fn matching_file(path: &Path, metadata: &Metadata, expected: &ManagedFile) -> io::Result<bool> {
    if !metadata.is_file() || metadata.len() != expected.bytes {
        return Ok(false);
    }
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    io::copy(&mut file, &mut digest)?;
    let actual = format!("{:x}", digest.finalize());
    Ok(actual.eq_ignore_ascii_case(&expected.sha256))
}

impl Installation {
    fn new(path: &Path, architecture: &'static str) -> io::Result<Self> {
        let directory = std::path::absolute(path)?;
        let name = directory
            .file_name()
            .ok_or_else(|| invalid("El destino debe ser una carpeta de CrabPDF."))?;
        let nearby = |suffix: &str| {
            let mut sibling = name.to_os_string();
            sibling.push(suffix);
            directory.with_file_name(sibling)
        };
        Ok(Self {
            backup: nearby(".crabpdf-previous"),
            journal: nearby(".crabpdf-update"),
            directory,
            architecture,
        })
    }

    fn reader(&self, folder: &Path) -> io::Result<()> {
        if !plain(folder)?.is_some_and(|item| item.is_dir()) {
            return Err(invalid("Carpeta de instalación inválida."));
        }
        let marker = read_small(&folder.join(MARKER), MARKER_LIMIT)?;
        let architecture = format!("Architecture={}", self.architecture);
        if marker.lines().any(|line| line == "Product=CrabPDF")
            && marker.lines().any(|line| line == architecture)
        {
            Ok(())
        } else {
            Err(invalid(
                "La carpeta no pertenece a CrabPDF para esta arquitectura.",
            ))
        }
    }

    fn record(&self) -> String {
        format!("{}\n{}", self.architecture, self.directory.display())
    }

    fn recover(&self) -> io::Result<()> {
        let backup = plain(&self.backup)?;
        if plain(&self.journal)?.is_none() {
            return if backup.is_some() {
                Err(invalid("La carpeta reservada para recuperación ya existe."))
            } else {
                Ok(())
            };
        }
        if read_small(&self.journal, MARKER_LIMIT)? != self.record() {
            return Err(invalid(
                "El registro de recuperación no pertenece a esta instalación.",
            ));
        }
        if let Some(backup) = backup {
            if !backup.is_dir() {
                return Err(invalid("Copia de recuperación inválida."));
            }
            if plain(&self.directory)?.is_some() {
                self.reader(&self.directory)?;
                visit(&self.backup, |_, _| Ok(()))?;
                fs::remove_dir_all(&self.backup)?;
            } else {
                self.reader(&self.backup)?;
                fs::rename(&self.backup, &self.directory)?;
            }
        }
        fs::remove_file(&self.journal)
    }

    fn preserve_user_files(&self, stage: &Path) -> io::Result<()> {
        let mut known = HashMap::new();
        let manifest = self.directory.join("manifest.json");
        if plain(&manifest)?.is_some()
            && let Ok(contents) = read_small(&manifest, MANIFEST_LIMIT)
            && let Ok(manifest) = serde_json::from_str::<Manifest>(&contents)
        {
            for file in manifest.files {
                if file.sha256.len() == 64 && file.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                    known.insert(file.path.replace('\\', "/").to_lowercase(), file);
                }
            }
        }
        visit(&self.directory, |path, metadata| {
            let relative = path
                .strip_prefix(&self.directory)
                .map_err(io::Error::other)?;
            let destination = stage.join(relative);
            if let Some(existing) = plain(&destination)? {
                if !same_kind(metadata.file_type(), existing.file_type()) {
                    return Err(invalid(format!(
                        "Conflicto con el paquete: {}",
                        relative.display()
                    )));
                }
                return Ok(());
            }
            if metadata.is_dir() {
                return fs::create_dir(&destination);
            }
            let key = relative.to_string_lossy().replace('\\', "/").to_lowercase();
            if let Some(expected) = known.get(&key)
                && matching_file(path, metadata, expected)?
            {
                return Ok(());
            }
            if !metadata.is_file() {
                return Err(invalid(format!(
                    "Archivo de usuario inválido: {}",
                    relative.display()
                )));
            }
            fs::copy(path, destination).map(|_| ())
        })
    }

    fn commit(&self, stage: &Path) -> io::Result<()> {
        let stage = std::path::absolute(stage)?;
        if stage.parent() != self.directory.parent()
            || [&self.directory, &self.backup, &self.journal]
                .iter()
                .any(|reserved| {
                    reserved
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&stage.to_string_lossy())
                })
        {
            return Err(invalid("La carpeta temporal debe estar junto al destino."));
        }
        self.reader(&stage)?;
        visit(&stage, |_, _| Ok(()))?;
        if plain(&self.directory)?.is_none() {
            return fs::rename(stage, &self.directory);
        }
        self.reader(&self.directory)?;
        self.preserve_user_files(&stage)?;

        let mut journal = File::create_new(&self.journal)?;
        let recorded = journal
            .write_all(self.record().as_bytes())
            .and_then(|()| journal.sync_all());
        drop(journal);
        if let Err(error) = recorded {
            let _ = fs::remove_file(&self.journal);
            return Err(error);
        }

        if let Err(error) = fs::rename(&self.directory, &self.backup)
            .and_then(|()| fs::rename(stage, &self.directory))
        {
            self.recover()?;
            return Err(error);
        }
        if let Err(error) = self.recover() {
            eprintln!("Se conservará la copia de recuperación: {error}");
        }
        Ok(())
    }
}

fn run() -> io::Result<()> {
    let mut args = env::args_os().skip(1);
    let architecture = match args.next().as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("x64") => "x64",
        Some("x86") => "x86",
        _ => return Err(invalid("Arquitectura de instalación inválida.")),
    };
    let path = args
        .next()
        .ok_or_else(|| invalid("Falta el destino de instalación."))?;
    let stage: Option<OsString> = args.next();
    if args.next().is_some() {
        return Err(invalid("Demasiados argumentos."));
    }
    let installation = Installation::new(Path::new(&path), architecture)?;
    installation.recover()?;
    if let Some(stage) = stage {
        installation.commit(Path::new(&stage))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    if let Err(error) = run() {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
