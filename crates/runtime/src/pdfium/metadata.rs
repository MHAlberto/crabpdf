//! Native metadata adapter; only the runtime document owner calls it.
use super::*;

impl<'a> PdfiumEngine<'a> {
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
}
