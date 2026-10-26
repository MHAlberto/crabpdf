//! Native text adapter; only the runtime document owner calls it.
use super::*;

impl<'a> PdfiumEngine<'a> {
    pub(super) fn ensure_djvu_text(&mut self, index: usize) -> Result<(), ReaderError> {
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
}
