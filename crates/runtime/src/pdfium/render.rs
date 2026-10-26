use super::*;

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
