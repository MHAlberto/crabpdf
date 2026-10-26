//! Native export adapter; only the runtime document owner calls it.
use super::*;

impl<'a> PdfiumEngine<'a> {
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
