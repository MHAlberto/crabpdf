use super::*;

/// Ask PDFium for the page transform once, including its CropBox, nonzero origin
/// and intrinsic /Rotate. Deriving an affine basis avoids four FFI calls per glyph.
pub(super) struct PageTransform {
    origin: (f32, f32),
    x: (f32, f32),
    y: (f32, f32),
}
impl PageTransform {
    pub(super) fn new(page: &PdfPage<'_>) -> Result<Self, ReaderError> {
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
    pub(super) fn bounds(&self, rect: PdfRect) -> Bounds {
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
