//! Native export adapter; only the runtime document owner calls it.
use super::*;

impl<'a> PdfiumEngine<'a> {
    pub fn prepare_print(&mut self, path: &Path, range: &str) -> Result<(), ReaderError> {
        let _guard = self
            .mark_store
            .as_deref()
            .map(crate::annotations::lock)
            .transpose()?;
        self.refresh_marks()?;
        if path.exists() {
            return Err(ReaderError::new(
                ErrorKind::File,
                "La copia de impresión ya existe.",
            ));
        }
        let original = self.document()?;
        if !original
            .permissions()
            .can_print_high_quality()
            .unwrap_or(false)
        {
            return Err(ReaderError::new(
                ErrorKind::Protected,
                "Este PDF restringe la impresión de alta calidad.",
            ));
        }
        let range =
            crate::application::printing::pages(range, original.pages().len().max(0) as usize)
                .map_err(|message| ReaderError::new(ErrorKind::File, message))?;
        let mut prepared = self.pdfium.create_new_pdf().map_err(map_render_error)?;
        prepared
            .pages_mut()
            .copy_pages_from_document(original, &range, 0)
            .map_err(map_render_error)?;
        crate::fileops::atomic_write(path, |output| {
            prepared
                .save_to_writer(output)
                .map_err(std::io::Error::other)
        })
        .map_err(|error| {
            ReaderError::new(ErrorKind::File, "No se pudo preparar la impresión.")
                .with_details(error.to_string())
        })
    }

    pub fn save_copy(&mut self, path: &Path) -> Result<(), ReaderError> {
        let _guard = self
            .mark_store
            .as_deref()
            .map(crate::annotations::lock)
            .transpose()?;
        self.refresh_marks()?;
        for index in 0..self.djvu_words.len() {
            self.ensure_djvu_text(index)?;
        }
        if self
            .source
            .as_ref()
            .and_then(|p| p.canonicalize().ok())
            .zip(path.canonicalize().ok())
            .is_some_and(|(a, b)| a == b)
        {
            return Err(ReaderError::new(
                ErrorKind::File,
                "Elige otro archivo para guardar la copia; el documento de origen está abierto.",
            ));
        }
        let document = self.document()?;
        crate::fileops::atomic_write(path, |file| {
            document.save_to_writer(file).map_err(std::io::Error::other)
        })
        .map_err(|e| {
            ReaderError::new(
                ErrorKind::File,
                "No se pudo guardar la copia del documento.",
            )
            .with_details(e.to_string())
        })
    }
}
