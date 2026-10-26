use crate::document::{
    Bounds, DocumentDetails, DocumentInfo, ErrorKind, Glyph, LinkTarget, MarkupKind, OutlineItem,
    PageColors, PageContent, PageLink, PageRaster, PageSize, ReaderError, RenderKey, SearchHit,
    SearchRequest, TextMarkup,
};
mod bindings;
pub mod discovery;
mod export;
mod geometry;
mod markup;
mod metadata;
mod render;
mod text;

pub use bindings::bind_pdfium;
use geometry::PageTransform;

use pdfium_render::prelude::*;
use std::path::Path;

/// The worker depends on this interface; neither the engine nor its data use egui.
pub trait PdfEngine {
    fn open(&mut self, path: &Path) -> Result<DocumentInfo, ReaderError>;
    fn render(&self, key: RenderKey) -> Result<PageRaster, ReaderError>;
}

pub struct PdfiumEngine<'a> {
    pdfium: &'a Pdfium,
    document: Option<PdfDocument<'a>>,
    source: Option<std::path::PathBuf>,
    marks: Vec<TextMarkup>,
    mark_indices: Vec<usize>,
    mark_store: Option<std::path::PathBuf>,
    djvu_words: Vec<Option<Vec<crate::djvu_text::Word>>>,
    imported_info: Option<crate::formats::ImportInfo>,
    mark_warning: Option<ReaderError>,
    revision: u64,
    data_directory: std::path::PathBuf,
    prepared_signature: Option<String>,
    legacy_pdf: bool,
}

impl<'a> PdfiumEngine<'a> {
    pub fn new(pdfium: &'a Pdfium) -> Self {
        Self::with_data_directory(pdfium, crate::formats::data_directory())
    }

    pub fn with_data_directory(pdfium: &'a Pdfium, data_directory: std::path::PathBuf) -> Self {
        Self {
            pdfium,
            document: None,
            source: None,
            marks: Vec::new(),
            mark_indices: Vec::new(),
            mark_store: None,
            djvu_words: Vec::new(),
            imported_info: None,
            mark_warning: None,
            revision: 0,
            data_directory,
            prepared_signature: None,
            legacy_pdf: false,
        }
    }

    pub fn open_with_password(
        &mut self,
        path: &Path,
        password: Option<&str>,
    ) -> Result<DocumentInfo, ReaderError> {
        let prepared = crate::formats::prepare_in(path, &self.data_directory)?;
        self.open_prepared(path, &prepared, password)
    }

    pub fn open_prepared(
        &mut self,
        path: &Path,
        prepared: &Path,
        password: Option<&str>,
    ) -> Result<DocumentInfo, ReaderError> {
        let document = self
            .pdfium
            .load_pdf_from_file(prepared, password)
            .map_err(map_open_error)?;
        let page_count = document.pages().len() as usize;
        if page_count == 0 {
            return Err(ReaderError::new(
                ErrorKind::Empty,
                "Este documento no contiene páginas.",
            ));
        }
        self.document = Some(document);
        self.source = Some(path.to_owned());
        self.legacy_pdf = matches!(
            crate::formats::Format::detect(path),
            Ok(crate::formats::Format::Pdf)
        );
        self.prepared_signature = match crate::formats::prepared_signature(path, prepared) {
            Ok(signature) => Some(signature),
            Err(error) => {
                self.mark_warning = Some(error);
                None
            }
        };
        self.imported_info = crate::formats::import_info(prepared);
        self.djvu_words = if matches!(
            crate::formats::Format::detect(path),
            Ok(crate::formats::Format::Djvu)
        ) {
            crate::djvu_text::read(&prepared.with_extension("xml"))?
                .into_iter()
                .map(Some)
                .collect()
        } else {
            Vec::new()
        };
        self.marks.clear();
        self.mark_indices.clear();
        self.mark_store = crate::annotations::path_in(path, &self.data_directory);
        if self.prepared_signature.is_some() {
            self.mark_warning = None;
        }
        if let Some(store) = &self.mark_store {
            let marks = match self.prepared_signature.as_deref() {
                Some(signature) => crate::annotations::load(store, signature, self.legacy_pdf),
                None => Ok(Vec::new()),
            };
            let marks = match marks {
                Ok(marks) => marks,
                Err(error) => {
                    self.mark_warning = Some(error);
                    Vec::new()
                }
            };
            for mark in marks {
                let index = match self.apply_markup(&mark) {
                    Ok(index) => index,
                    Err(error) => {
                        self.mark_warning = Some(error);
                        break;
                    }
                };
                self.mark_indices.push(index);
                self.marks.push(mark);
            }
        }
        self.revision = 0;
        Ok(DocumentInfo {
            path: path.to_owned(),
            page_count,
        })
    }

    fn document(&self) -> Result<&PdfDocument<'a>, ReaderError> {
        self.document
            .as_ref()
            .ok_or_else(|| ReaderError::new(ErrorKind::Render, "No hay un documento abierto."))
    }
}

fn map_open_error(error: PdfiumError) -> ReaderError {
    let (kind, message) = match &error {
        PdfiumError::IoError(_)
        | PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::FileError) => (
            ErrorKind::File,
            "No se pudo leer el archivo. Comprueba que exista y tengas permisos.",
        ),
        PdfiumError::PdfiumLibraryInternalError(
            PdfiumInternalError::PasswordError | PdfiumInternalError::SecurityError,
        ) => (
            ErrorKind::Protected,
            "Este PDF necesita una contraseña o utiliza seguridad no compatible.",
        ),
        _ => (
            ErrorKind::InvalidPdf,
            "No se pudo abrir el documento. Puede ser un PDF inválido o estar dañado.",
        ),
    };
    ReaderError::new(kind, message).with_details(error.to_string())
}

fn map_render_error(error: PdfiumError) -> ReaderError {
    ReaderError::new(
        ErrorKind::Render,
        "No se pudo renderizar esta página. Puedes reintentar o pasar a otra página.",
    )
    .with_details(error.to_string())
}
