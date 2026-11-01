//! Geometry derived from the selected ink, not PDFium's potentially oversized text segments.
//! PDF coordinates have their origin at the bottom left; /Rotate is applied by the renderer.
use super::*;

struct Run {
    bounds: PdfRect,
    baseline: f32,
    em: f32,
    angle: f32,
}
impl Run {
    fn joins(&self, next: &Self) -> bool {
        let em = self.em.min(next.em);
        let baseline = (self.baseline - next.baseline).abs();
        let gap = (next.bounds.left().value - self.bounds.right().value)
            .max(self.bounds.left().value - next.bounds.right().value);
        (self.angle - next.angle).abs() < 0.02
            && baseline <= em * 0.18
            && self.em.max(next.em) <= em * 1.5
            && gap <= em * 0.65
    }
    fn include(&mut self, next: &Self) {
        self.bounds = PdfRect::new_from_values(
            self.bounds.bottom().value.min(next.bounds.bottom().value),
            self.bounds.left().value.min(next.bounds.left().value),
            self.bounds.top().value.max(next.bounds.top().value),
            self.bounds.right().value.max(next.bounds.right().value),
        );
        self.em = self.em.max(next.em);
    }
    fn rect(&self, kind: MarkupKind) -> PdfRect {
        let rect = &self.bounds;
        if kind == MarkupKind::Highlight {
            let padding = (self.em * 0.06).clamp(0.2, 2.0);
            PdfRect::new_from_values(
                rect.bottom().value - padding,
                rect.left().value - padding * 0.3,
                rect.top().value + padding,
                rect.right().value + padding * 0.3,
            )
        } else {
            let gap = (self.em * 0.04).clamp(0.25, 1.0);
            // PDFium places its 1pt stroke at quad.bottom + 1, not at the bottom.
            // Keep its entire stroke below the ink, including descenders. A thin
            // quad also avoids covering the letters in other PDF readers.
            let upside_down = self.angle.cos() < -0.9;
            let center = if upside_down {
                rect.top().value + gap + 0.5
            } else {
                rect.bottom().value - gap - 0.5
            };
            PdfRect::new_from_values(
                center - 1.0,
                rect.left().value,
                center + 1.0,
                rect.right().value,
            )
        }
    }
}

fn usable(rect: &PdfRect) -> bool {
    [
        rect.left().value,
        rect.right().value,
        rect.top().value,
        rect.bottom().value,
    ]
    .iter()
    .all(|v| v.is_finite())
        && rect.width().value > 0.0
        && rect.height().value > 0.0
}

pub(super) fn selection_geometry(
    text: &PdfPageText<'_>,
    start: usize,
    end: usize,
    kind: MarkupKind,
) -> Result<(Vec<PdfRect>, String), ReaderError> {
    let chars = text.chars();
    let mut rects = Vec::new();
    let mut contents = String::new();
    let mut run: Option<Run> = None;
    for index in start..=end {
        let character = chars.get(index).map_err(map_render_error)?;
        let value = character.unicode_string().unwrap_or_default();
        contents.push_str(&value);
        if value.contains(['\r', '\n'])
            && let Some(previous) = run.take()
        {
            rects.push(previous.rect(kind));
        }
        if value.is_empty() || value.chars().all(char::is_whitespace) {
            continue;
        }
        let Some(bounds) = character
            .tight_bounds()
            .ok()
            .filter(usable)
            .or_else(|| character.loose_bounds().ok().filter(usable))
        else {
            continue;
        };
        let height = bounds.height().value;
        let font_size = character.scaled_font_size().value;
        let em = if font_size.is_finite() && font_size > 0.0 && font_size < height * 5.0 {
            font_size
        } else {
            height
        };
        let baseline = character
            .origin_y()
            .map_or(bounds.bottom().value, |v| v.value);
        let next = Run {
            bounds,
            baseline,
            em,
            angle: character.angle_radians().unwrap_or(0.0),
        };
        if let Some(previous) = run.as_mut()
            && previous.joins(&next)
        {
            previous.include(&next);
        } else if let Some(previous) = run.replace(next) {
            rects.push(previous.rect(kind));
        }
        // Bound allocation even for adversarial pages with a separate run per glyph.
        if rects.len() > 512 {
            break;
        }
    }
    if let Some(previous) = run {
        rects.push(previous.rect(kind));
    }
    Ok((rects, contents))
}
