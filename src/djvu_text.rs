//! Preserve an existing DjVu OCR layer; no OCR recognition or extra service is run.
use crate::document::{ErrorKind, ReaderError};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Word {
    pub text: String,
    pub left: f32,
    pub bottom: f32,
    pub width: f32,
    pub height: f32,
}

pub fn read(path: &Path) -> Result<Vec<Vec<Word>>, ReaderError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    if path.metadata().map_err(error)?.len() > 16 * 1024 * 1024 {
        return Err(error("La capa de texto supera 16 MiB."));
    }
    let mut xml = std::fs::read_to_string(path).map_err(error)?;
    for code in (0..32).filter(|code| ![9, 10, 13].contains(code)) {
        xml = xml
            .replace(&format!("&#{code};"), "")
            .replace(&format!("&#x{code:X};"), "")
            .replace(&format!("&#x{code:x};"), "");
    }
    let document = roxmltree::Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .map_err(error)?;
    let mut pages = Vec::new();
    let mut count = 0;
    for page in document
        .descendants()
        .filter(|node| node.has_tag_name("OBJECT"))
    {
        let number = |name| {
            page.attribute(name)
                .and_then(|text| text.parse::<f32>().ok())
                .filter(|value| value.is_finite() && *value > 0.0)
        };
        let (Some(width), Some(height)) = (number("width"), number("height")) else {
            return Err(error("Geometría de DjVu inválida."));
        };
        let mut words = Vec::new();
        for word in page.descendants().filter(|node| node.has_tag_name("WORD")) {
            let Some(coords) = word.attribute("coords") else {
                continue;
            };
            let coords: Vec<_> = coords
                .split(',')
                .filter_map(|text| text.trim().parse::<f32>().ok())
                .collect();
            if coords.len() != 4 || coords.iter().any(|value| !value.is_finite()) {
                continue;
            }
            let text = word.text().unwrap_or_default().trim();
            if text.is_empty() || coords[2] <= coords[0] || coords[1] <= coords[3] {
                continue;
            }
            let text = format!("{text} ");
            words.push(Word {
                text,
                left: coords[0] / width,
                bottom: 1.0 - coords[1] / height,
                width: (coords[2] - coords[0]) / width,
                height: (coords[1] - coords[3]) / height,
            });
            count += 1;
            if count > 200_000 {
                return Err(error("Demasiadas palabras en la capa DjVu."));
            }
        }
        pages.push(words);
    }
    Ok(pages)
}
fn error(detail: impl std::fmt::Display) -> ReaderError {
    ReaderError::new(
        ErrorKind::InvalidPdf,
        "No se pudo leer la capa de texto DjVu.",
    )
    .with_details(detail.to_string())
}
