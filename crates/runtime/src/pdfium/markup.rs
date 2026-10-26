//! Native markup adapter; only the runtime document owner calls it.
use super::*;

impl<'a> PdfiumEngine<'a> {
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

    pub(super) fn delete_markup(&mut self, page: usize, index: usize) -> Result<(), ReaderError> {
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

    pub(super) fn apply_markup(&mut self, mark: &TextMarkup) -> Result<usize, ReaderError> {
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
            annotation.set_creator(crabpdf_core::APPLICATION_NAME)?;
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
}
