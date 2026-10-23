use crate::document::{
    Bounds, DocumentDetails, DocumentInfo, ErrorKind, Glyph, LinkTarget, MarkupKind, OutlineItem,
    PageColors, PageContent, PageLink, PageRaster, PageSize, ReaderError, RenderKey, SearchHit,
    SearchRequest, TextMarkup,
};
use crate::platform;
use pdfium_render::prelude::*;
use std::path::Path;

/// The worker depends on this interface; neither the engine nor its data use egui.
pub trait PdfEngine {
    fn open(&mut self, path: &Path) -> Result<DocumentInfo, ReaderError>;
    fn render(&self, key: RenderKey) -> Result<PageRaster, ReaderError>;
}

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

    pub fn details(&self) -> Result<DocumentDetails, ReaderError> {
        let document = self.document()?;
        let page = document.pages().get(0).map_err(map_render_error)?;
        let mut title = document
            .metadata()
            .get(PdfDocumentMetadataTagType::Title)
            .map(|tag| tag.value().to_owned());
        let mut properties = vec![
            (
                "Formato".into(),
                self.source
                    .as_deref()
                    .and_then(|path| crate::formats::Format::detect(path).ok())
                    .map_or("PDF", |format| format.label())
                    .into(),
            ),
            ("Páginas".into(), document.pages().len().to_string()),
            ("Versión".into(), format!("{:?}", document.version())),
            ("Marcas locales".into(), self.marks.len().to_string()),
        ];
        for tag in document.metadata().iter() {
            let name = match tag.tag_type() {
                PdfDocumentMetadataTagType::Title => "Título",
                PdfDocumentMetadataTagType::Author => "Autor",
                PdfDocumentMetadataTagType::Subject => "Asunto",
                PdfDocumentMetadataTagType::Keywords => "Palabras clave",
                PdfDocumentMetadataTagType::Creator => "Creador",
                PdfDocumentMetadataTagType::Producer => "Productor",
                PdfDocumentMetadataTagType::CreationDate => "Creación",
                PdfDocumentMetadataTagType::ModificationDate => "Modificación",
            };
            properties.push((name.into(), tag.value().into()));
        }
        let mut outline = document
            .bookmarks()
            .iter()
            .take(10_000)
            .map(|b| {
                let mut depth = 0;
                let mut parent = b.parent();
                while let Some(p) = parent {
                    depth += 1;
                    if depth >= 64 {
                        break;
                    }
                    parent = p.parent();
                }
                OutlineItem {
                    title: b.title().unwrap_or_default(),
                    page: b
                        .destination()
                        .and_then(|d| d.page_index().ok())
                        .map(|p| p as usize),
                    depth,
                }
            })
            .collect::<Vec<_>>();
        if let Some(info) = &self.imported_info {
            if !info.outline.is_empty() {
                outline = info.outline.clone();
            }
            if !info.title.is_empty() {
                title = Some(info.title.clone());
                properties.push(("Título".into(), info.title.clone()));
            }
            if !info.author.is_empty() {
                properties.push(("Autor".into(), info.author.clone()));
            }
        }
        let annotation_warning = self
            .mark_warning
            .as_ref()
            .map(|warning| format!("{warning} El documento sigue disponible para lectura."));
        if let Some(warning) = &annotation_warning {
            properties.push(("Aviso de anotaciones".into(), warning.clone()));
        }
        Ok(DocumentDetails {
            title,
            annotation_count: self.marks.len(),
            annotation_warning,
            first_page_size: PageSize {
                width: page.width().value,
                height: page.height().value,
            },
            outline,
            properties,
            can_copy: document
                .permissions()
                .can_extract_text_and_graphics()
                .unwrap_or(false),
            can_annotate: self.mark_warning.is_none()
                && document
                    .permissions()
                    .can_add_or_modify_text_annotations()
                    .unwrap_or(false),
        })
    }

    pub fn add_markup(&mut self, mark: TextMarkup) -> Result<usize, ReaderError> {
        let _guard = self
            .mark_store
            .as_deref()
            .map(crate::annotations::lock)
            .transpose()?;
        self.refresh_marks()?;
        if self.marks.len() >= 4096 {
            return Err(ReaderError::new(
                ErrorKind::File,
                "Se alcanzó el límite de 4096 marcas.",
            ));
        }
        let index = self.apply_markup(&mark)?;
        self.marks.push(mark.clone());
        if let Some(store) = &self.mark_store
            && let Err(error) = crate::annotations::save(
                store,
                &self.marks,
                self.prepared_signature.as_deref().ok_or_else(|| {
                    ReaderError::new(
                        ErrorKind::File,
                        "Falta la identidad del documento preparado.",
                    )
                })?,
            )
        {
            self.marks.pop();
            self.delete_markup(mark.page, index)?;
            return Err(error);
        }
        self.mark_indices.push(index);
        Ok(self.marks.len())
    }

    pub fn undo_markup(&mut self) -> Result<(usize, usize), ReaderError> {
        let _guard = self
            .mark_store
            .as_deref()
            .map(crate::annotations::lock)
            .transpose()?;
        self.refresh_marks()?;
        let Some(mark) = self.marks.last().cloned() else {
            return Err(ReaderError::new(
                ErrorKind::File,
                "No hay marcas para deshacer.",
            ));
        };
        let index = *self
            .mark_indices
            .last()
            .ok_or_else(|| ReaderError::new(ErrorKind::File, "Marca no disponible."))?;
        // Persist first, but restore the store if the native operation fails.
        if let Some(store) = &self.mark_store {
            crate::annotations::save(
                store,
                &self.marks[..self.marks.len() - 1],
                self.prepared_signature.as_deref().ok_or_else(|| {
                    ReaderError::new(
                        ErrorKind::File,
                        "Falta la identidad del documento preparado.",
                    )
                })?,
            )?;
        }
        if let Err(error) = self.delete_markup(mark.page, index) {
            if let Some(store) = &self.mark_store {
                crate::annotations::save(
                    store,
                    &self.marks,
                    self.prepared_signature.as_deref().ok_or_else(|| {
                        ReaderError::new(
                            ErrorKind::File,
                            "Falta la identidad del documento preparado.",
                        )
                    })?,
                )?;
            }
            return Err(error);
        }
        self.marks.pop();
        self.mark_indices.pop();
        Ok((mark.page, self.marks.len()))
    }

    fn delete_markup(&mut self, page: usize, index: usize) -> Result<(), ReaderError> {
        // Advance even if a native call fails after partially changing the page.
        self.revision += 1;
        let mut page = self
            .document()?
            .pages()
            .get(page as i32)
            .map_err(map_render_error)?;
        let annotations = page.annotations_mut();
        let annotation = annotations.get(index).map_err(map_render_error)?;
        annotations
            .delete_annotation(annotation)
            .map_err(map_render_error)
    }

    pub fn mark_store_path(&self) -> Option<&Path> {
        self.mark_store.as_deref()
    }
    pub fn annotation_state(&self) -> crate::document::AnnotationState {
        crate::document::AnnotationState {
            revision: self.revision,
            count: self.marks.len(),
        }
    }
    pub fn refresh_marks(&mut self) -> Result<usize, ReaderError> {
        let Some(store) = &self.mark_store else {
            return Ok(self.marks.len());
        };
        let signature = self.prepared_signature.as_deref().ok_or_else(|| {
            ReaderError::new(
                ErrorKind::File,
                "Falta la identidad del documento preparado.",
            )
        })?;
        let marks = crate::annotations::load(store, signature, self.legacy_pdf)?;
        if marks == self.marks {
            return Ok(marks.len());
        }
        for index in (0..self.marks.len()).rev() {
            self.delete_markup(self.marks[index].page, self.mark_indices[index])?;
        }
        self.marks.clear();
        self.mark_indices.clear();
        for mark in marks {
            let index = self.apply_markup(&mark)?;
            self.marks.push(mark);
            self.mark_indices.push(index);
        }
        Ok(self.marks.len())
    }

    fn apply_markup(&mut self, mark: &TextMarkup) -> Result<usize, ReaderError> {
        self.revision += 1;
        self.ensure_djvu_text(mark.page)?;
        let document = self.document()?;
        if !document
            .permissions()
            .can_add_or_modify_text_annotations()
            .unwrap_or(false)
            || !document
                .permissions()
                .can_extract_text_and_graphics()
                .unwrap_or(false)
        {
            return Err(ReaderError::new(
                ErrorKind::Protected,
                "El documento no permite anotar o seleccionar texto.",
            ));
        }
        let page_number = i32::try_from(mark.page)
            .map_err(|_| ReaderError::new(ErrorKind::File, "Página fuera de rango."))?;
        let mut page = document
            .pages()
            .get(page_number)
            .map_err(map_render_error)?;
        let (start, end) = (mark.start.min(mark.end), mark.start.max(mark.end));
        let (rects, contents) = {
            let text = page.text().map_err(map_render_error)?;
            if end >= text.len().max(0) as usize || end - start > 20_000 {
                return Err(ReaderError::new(
                    ErrorKind::File,
                    "Selección de texto fuera de rango o demasiado grande.",
                ));
            }
            let rects: Vec<PdfRect> = text
                .segments_subset(start, end - start + 1)
                .iter()
                .map(|segment| segment.bounds())
                .filter(|bounds| bounds.width().value > 0.0 && bounds.height().value > 0.0)
                .collect();
            let contents = text
                .chars()
                .iter()
                .filter(|character| character.index() >= start && character.index() <= end)
                .filter_map(|character| character.unicode_string())
                .collect::<String>();
            (rects, contents)
        };
        if rects.is_empty() || rects.len() > 512 {
            return Err(ReaderError::new(
                ErrorKind::File,
                "Selecciona palabras con texto reconocible para crear una marca.",
            ));
        }
        let bounds = PdfRect::new_from_values(
            rects
                .iter()
                .map(|rect| rect.bottom().value)
                .fold(f32::INFINITY, f32::min),
            rects
                .iter()
                .map(|rect| rect.left().value)
                .fold(f32::INFINITY, f32::min),
            rects
                .iter()
                .map(|rect| rect.top().value)
                .fold(f32::NEG_INFINITY, f32::max),
            rects
                .iter()
                .map(|rect| rect.right().value)
                .fold(f32::NEG_INFINITY, f32::max),
        );
        let index = page.annotations().len();
        let mut annotation = match mark.kind {
            MarkupKind::Highlight => PdfPageAnnotation::Highlight(
                page.annotations_mut()
                    .create_highlight_annotation()
                    .map_err(map_render_error)?,
            ),
            MarkupKind::Underline => PdfPageAnnotation::Underline(
                page.annotations_mut()
                    .create_underline_annotation()
                    .map_err(map_render_error)?,
            ),
        };
        let result = (|| {
            annotation.set_bounds(bounds)?;
            annotation.set_stroke_color(PdfColor::new(
                mark.color[0],
                mark.color[1],
                mark.color[2],
                255,
            ))?;
            annotation.set_creator(crate::branding::NAME)?;
            annotation.set_is_printed(true)?;
            annotation.set_contents(&contents)?;
            for rect in rects {
                // Text markup uses Adobe/PDFium's Z order: upper-left, upper-right,
                // lower-left, lower-right. PdfRect::to_quad_points() describes a
                // perimeter quadrilateral and produces a narrow stripe here.
                let quad = PdfQuadPoints::new_from_values(
                    rect.left().value,
                    rect.top().value,
                    rect.right().value,
                    rect.top().value,
                    rect.left().value,
                    rect.bottom().value,
                    rect.right().value,
                    rect.bottom().value,
                );
                match &mut annotation {
                    PdfPageAnnotation::Highlight(annotation) => {
                        annotation
                            .attachment_points_mut()
                            .create_attachment_point_at_end(quad)?;
                    }
                    PdfPageAnnotation::Underline(annotation) => {
                        annotation
                            .attachment_points_mut()
                            .create_attachment_point_at_end(quad)?;
                    }
                    _ => unreachable!(),
                }
            }
            Ok::<_, PdfiumError>(())
        })();
        if let Err(error) = result {
            page.annotations_mut()
                .delete_annotation(annotation)
                .map_err(map_render_error)?;
            return Err(map_render_error(error));
        }
        Ok(index)
    }

    fn ensure_djvu_text(&mut self, index: usize) -> Result<(), ReaderError> {
        let Some(words) = self.djvu_words.get(index).and_then(Option::as_ref).cloned() else {
            return Ok(());
        };
        let document = self
            .document
            .as_mut()
            .ok_or_else(|| ReaderError::new(ErrorKind::Render, "No hay documento abierto."))?;
        let font = document.fonts_mut().helvetica();
        let mut page = document
            .pages()
            .get(index as i32)
            .map_err(map_render_error)?;
        page.set_content_regeneration_strategy(PdfPageContentRegenerationStrategy::Manual);
        let (width, height) = (page.width().value, page.height().value);
        for word in words {
            let mut object = page
                .objects_mut()
                .create_text_object(
                    PdfPoints::ZERO,
                    PdfPoints::ZERO,
                    &word.text,
                    font,
                    PdfPoints::new(12.0),
                )
                .map_err(map_render_error)?;
            if let Some(text) = object.as_text_object_mut() {
                text.set_render_mode(PdfPageTextRenderMode::Invisible)
                    .map_err(map_render_error)?;
            }
            let bounds = object.bounds().map_err(map_render_error)?;
            if bounds.width().value > 0.0 && bounds.height().value > 0.0 {
                object
                    .scale(
                        word.width * width / bounds.width().value,
                        word.height * height / bounds.height().value,
                    )
                    .map_err(map_render_error)?;
                let bounds = object.bounds().map_err(map_render_error)?;
                object
                    .translate(
                        PdfPoints::new(word.left * width - bounds.left().value),
                        PdfPoints::new(word.bottom * height - bounds.bottom().value),
                    )
                    .map_err(map_render_error)?;
            }
        }
        page.regenerate_content().map_err(map_render_error)?;
        self.djvu_words[index] = None;
        Ok(())
    }

    pub fn page_content(&mut self, index: usize) -> Result<PageContent, ReaderError> {
        self.ensure_djvu_text(index)?;
        let document = self.document()?;
        let page = document
            .pages()
            .get(
                i32::try_from(index)
                    .map_err(|_| ReaderError::new(ErrorKind::Render, "Página fuera de rango."))?,
            )
            .map_err(map_render_error)?;
        let text = page.text().map_err(map_render_error)?;
        let transform = PageTransform::new(&page)?;
        let glyphs = text
            .chars()
            .iter()
            .map(|c| Glyph {
                index: c.index(),
                text: c.unicode_string().unwrap_or_default(),
                bounds: c
                    .loose_bounds()
                    .ok()
                    .map(|r| transform.bounds(r))
                    .filter(|r| r.right >= r.left && r.bottom >= r.top),
            })
            .collect();
        let mut links = Vec::new();
        for link in page.links().iter() {
            let target = link
                .destination()
                .and_then(|d| d.page_index().ok())
                .map(|p| LinkTarget::Page(p as usize))
                .or_else(|| {
                    link.action().and_then(|a| {
                        a.as_uri_action()
                            .and_then(|u| u.uri().ok())
                            .map(LinkTarget::Uri)
                    })
                });
            if let (Some(target), Ok(rect)) = (target, link.rect()) {
                links.push(PageLink {
                    bounds: transform.bounds(rect),
                    target,
                });
            }
        }
        Ok(PageContent { glyphs, links })
    }

    /// Search one page so the worker can yield to visible rendering between pages.
    pub fn search_page(
        &mut self,
        index: usize,
        request: &SearchRequest,
    ) -> Result<Vec<SearchHit>, ReaderError> {
        self.ensure_djvu_text(index)?;
        if request.query.is_empty() {
            return Ok(Vec::new());
        }
        let page = self
            .document()?
            .pages()
            .get(index as i32)
            .map_err(map_render_error)?;
        let transform = PageTransform::new(&page)?;
        let text = page.text().map_err(map_render_error)?;
        let search = text
            .search(
                &request.query,
                &PdfSearchOptions::new()
                    .match_case(request.match_case)
                    .match_whole_word(request.whole_word),
            )
            .map_err(map_render_error)?;
        let mut hits = Vec::new();
        for found in search
            .iter(PdfSearchDirection::SearchForward)
            .take(crate::document::MAX_SEARCH_HITS)
        {
            let mut indices = Vec::new();
            let mut bounds = Vec::new();
            for segment in found.iter() {
                bounds.push(transform.bounds(segment.bounds()));
                if let Ok(chars) = segment.chars() {
                    indices.extend(chars.iter().map(|c| c.index()));
                }
            }
            let start = indices.iter().min().copied().unwrap_or(0);
            let count = indices
                .iter()
                .max()
                .copied()
                .map_or(0, |end| end - start + 1);
            hits.push(SearchHit {
                page: index,
                start,
                count,
                bounds,
            });
        }
        Ok(hits)
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

/// Ask PDFium for the page transform once, including its CropBox, nonzero origin
/// and intrinsic /Rotate. Deriving an affine basis avoids four FFI calls per glyph.
struct PageTransform {
    origin: (f32, f32),
    x: (f32, f32),
    y: (f32, f32),
}
impl PageTransform {
    fn new(page: &PdfPage<'_>) -> Result<Self, ReaderError> {
        let config = PdfRenderConfig::new().set_fixed_size(1_000_000, 1_000_000);
        let point = |x, y| {
            page.points_to_pixels(PdfPoints::new(x), PdfPoints::new(y), &config)
                .map(|(x, y)| (x as f32 / 1_000_000.0, y as f32 / 1_000_000.0))
                .map_err(map_render_error)
        };
        let origin = point(0.0, 0.0)?;
        let x = point(1000.0, 0.0)?;
        let y = point(0.0, 1000.0)?;
        Ok(Self {
            origin,
            x: ((x.0 - origin.0) / 1000.0, (x.1 - origin.1) / 1000.0),
            y: ((y.0 - origin.0) / 1000.0, (y.1 - origin.1) / 1000.0),
        })
    }
    fn bounds(&self, rect: PdfRect) -> Bounds {
        let corners = [
            (rect.left().value, rect.top().value),
            (rect.right().value, rect.top().value),
            (rect.left().value, rect.bottom().value),
            (rect.right().value, rect.bottom().value),
        ]
        .map(|(x, y)| {
            (
                self.origin.0 + x * self.x.0 + y * self.y.0,
                self.origin.1 + x * self.x.1 + y * self.y.1,
            )
        });
        Bounds {
            left: corners.iter().map(|p| p.0).fold(f32::INFINITY, f32::min),
            top: corners.iter().map(|p| p.1).fold(f32::INFINITY, f32::min),
            right: corners
                .iter()
                .map(|p| p.0)
                .fold(f32::NEG_INFINITY, f32::max),
            bottom: corners
                .iter()
                .map(|p| p.1)
                .fold(f32::NEG_INFINITY, f32::max),
        }
    }
}

impl PdfEngine for PdfiumEngine<'_> {
    fn open(&mut self, path: &Path) -> Result<DocumentInfo, ReaderError> {
        self.open_with_password(path, None)
    }

    fn render(&self, key: RenderKey) -> Result<PageRaster, ReaderError> {
        let document = self
            .document
            .as_ref()
            .ok_or_else(|| ReaderError::new(ErrorKind::Render, "No hay un documento abierto."))?;
        let page_index = i32::try_from(key.page)
            .map_err(|_| ReaderError::new(ErrorKind::Render, "Número de página fuera de rango."))?;
        let page = document.pages().get(page_index).map_err(map_render_error)?;
        let sizing = render_sizing(page.width().value, page.height().value, key)?;
        let rotation = match key.rotation % 4 {
            1 => PdfPageRenderRotation::Degrees90,
            2 => PdfPageRenderRotation::Degrees180,
            3 => PdfPageRenderRotation::Degrees270,
            _ => PdfPageRenderRotation::None,
        };
        let scale = sizing.pixel_width as f32
            / if key.rotation.is_multiple_of(2) {
                page.width().value
            } else {
                page.height().value
            };
        let config = PdfRenderConfig::new()
            .scale_page_by_factor(scale)
            .rotate(rotation, key.rotation % 2 == 1)
            .set_maximum_width(sizing.pixel_width as i32)
            .set_maximum_height(sizing.pixel_height as i32)
            .use_grayscale_rendering(key.colors == PageColors::Grayscale);
        let bitmap = if let Some(region) = key.region {
            let width = region
                .width
                .min(sizing.pixel_width.saturating_sub(region.x));
            let height = region
                .height
                .min(sizing.pixel_height.saturating_sub(region.y));
            if width == 0 || height == 0 || width > 1024 || height > 1024 {
                return Err(ReaderError::new(
                    ErrorKind::Render,
                    "Región de renderizado fuera de rango.",
                ));
            }
            let mut bitmap = PdfBitmap::empty(width as i32, height as i32, PdfBitmapFormat::BGRA)
                .map_err(map_render_error)?;
            // PDFium clips to the small destination bitmap. The page's scale remains
            // unchanged; panning never allocates the entire zoomed page.
            page.render_into_bitmap_with_config(
                &mut bitmap,
                &config.set_origin(-(region.x as i32), -(region.y as i32)),
            )
            .map_err(map_render_error)?;
            bitmap
        } else {
            page.render_with_config(&config).map_err(map_render_error)?
        };
        let mut rgba = bitmap.as_rgba_bytes();
        if key.colors == PageColors::Inverted {
            for pixel in rgba.chunks_exact_mut(4) {
                for component in &mut pixel[..3] {
                    *component = 255 - *component;
                }
            }
        }
        Ok(PageRaster {
            width: bitmap.width() as usize,
            height: bitmap.height() as usize,
            rgba,
            logical_width: sizing.logical_width,
            logical_height: sizing.logical_height,
            zoom_percent: sizing.zoom_percent,
            resolution_limited: sizing.limited,
            page_size: PageSize {
                width: page.width().value,
                height: page.height().value,
            },
        })
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

struct Sizing {
    pixel_width: u32,
    pixel_height: u32,
    logical_width: f32,
    logical_height: f32,
    zoom_percent: f32,
    limited: bool,
}

/// Cap pixel allocation, while preserving the requested on-screen size and aspect ratio.
fn render_sizing(width_pt: f32, height_pt: f32, key: RenderKey) -> Result<Sizing, ReaderError> {
    if !width_pt.is_finite() || !height_pt.is_finite() || width_pt <= 0.0 || height_pt <= 0.0 {
        return Err(ReaderError::new(
            ErrorKind::Render,
            "La página tiene dimensiones inválidas.",
        ));
    }
    let (width_pt, height_pt) = if key.rotation.is_multiple_of(2) {
        (width_pt, height_pt)
    } else {
        (height_pt, width_pt)
    };
    let base_width = f64::from(width_pt) * 96.0 / 72.0;
    let (logical_width, logical_height) = crate::layout::logical_page_size(
        PageSize {
            width: width_pt,
            height: height_pt,
        },
        0,
        key.zoom,
        f64::from(key.viewport_width.max(1)),
        f64::from(key.viewport_height.max(1)),
    );
    // Reject pathological PDFs before enormous layout coordinates reach egui.
    if logical_width > 1_000_000.0 || logical_height > 1_000_000.0 {
        return Err(ReaderError::new(
            ErrorKind::Render,
            "Las dimensiones de esta página exceden el límite de lectura.",
        ));
    }
    let density = f64::from(key.density_milli.clamp(500, 8000)) / 1000.0;
    let width = logical_width * density;
    let height = logical_height * density;
    let side_limit = f64::from(key.bitmap_side_limit.clamp(1, 8192));
    let scale = if key.region.is_some() {
        1.0
    } else {
        1.0_f64.min(side_limit / width.max(height)).min(
            (if cfg!(target_pointer_width = "32") {
                4_000_000.0
            } else {
                16_000_000.0
            } / (width * height))
                .sqrt(),
        )
    };
    Ok(Sizing {
        pixel_width: (width * scale).round().max(1.0) as u32,
        pixel_height: (height * scale).round().max(1.0) as u32,
        logical_width: logical_width as f32,
        logical_height: logical_height as f32,
        zoom_percent: (logical_width / base_width * 100.0) as f32,
        limited: scale < 1.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Zoom;

    fn key(zoom: Zoom) -> RenderKey {
        RenderKey {
            document_id: 1,
            revision: 0,
            page: 0,
            zoom,
            viewport_width: 900,
            density_milli: 2000,
            bitmap_side_limit: 8192,
            viewport_height: 700,
            rotation: 0,
            colors: PageColors::Original,
            region: None,
        }
    }

    #[test]
    fn fit_width_hidpi_and_memory_bound() {
        let mut fit = key(Zoom::FitWidth);
        fit.viewport_width = 700;
        let s = render_sizing(600.0, 900.0, fit).unwrap();
        assert_eq!((s.pixel_width, s.pixel_height), (1400, 2100));
        assert_eq!(s.logical_width, 700.0);
        let s = render_sizing(600.0, 900.0, key(Zoom::Percent(400))).unwrap();
        assert!(s.limited);
        assert!(u64::from(s.pixel_width) * u64::from(s.pixel_height) <= 16_010_000);
        assert_eq!(s.logical_width, 3200.0);
        let mut small_gpu = key(Zoom::FitWidth);
        small_gpu.bitmap_side_limit = 512;
        let s = render_sizing(600.0, 900.0, small_gpu).unwrap();
        assert!(s.pixel_width <= 512 && s.pixel_height <= 512 && s.limited);
        assert_eq!(s.logical_width, 900.0);
    }

    #[test]
    fn invalid_geometry_is_an_error() {
        for width in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(render_sizing(width, 800.0, key(Zoom::FitWidth)).is_err());
        }
    }
}
