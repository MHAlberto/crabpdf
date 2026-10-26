use super::discovery as platform;
use crate::document::{ErrorKind, ReaderError};
use pdfium_render::prelude::*;

pub fn bind_pdfium() -> Result<Pdfium, ReaderError> {
    let mut failures = Vec::new();
    for path in platform::library_candidates() {
        match Pdfium::bind_to_library(&path) {
            Ok(bindings) => return Ok(Pdfium::new(bindings)),
            // Default reuses process-global bindings only after this checked error.
            Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => {
                return Ok(Pdfium::default());
            }
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => Ok(Pdfium::default()),
        Err(error) => {
            failures.push(format!("Biblioteca del sistema: {error}"));
            Err(ReaderError::new(
                ErrorKind::Library,
                format!(
                    "No se pudo cargar {}. Instala PDFium (build 7881) en native/ o indica su ruta con RUST_READER_PDFIUM y reinicia la aplicación.",
                    platform::library_name()
                ),
            ).with_details(failures.join("\n")))
        }
    }
}
