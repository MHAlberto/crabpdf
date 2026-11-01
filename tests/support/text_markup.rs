//! Original in-memory fixtures exercise visible mark placement, not only annotation presence.
use super::*;
use crabpdf::document::{MarkupKind, TextMarkup};
use pdfium_render::prelude::*;

fn sample(font: &str, size: u16, rotation: u16) -> Vec<u8> {
    let stream = format!(
        "BT /F1 {size} Tf 40 180 Td (ABCpqgy) Tj ET\nBT /F1 {size} Tf 230 180 Td (Column) Tj ET\nBT /F1 {size} Tf 40 110 Td (Second line) Tj ET"
    );
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 360 260] /CropBox [10 10 350 250] /Rotate {rotation} /Contents 5 0 R /Resources << /Font << /F1 4 0 R >> >> >>"
        ),
        format!("<< /Type /Font /Subtype /Type1 /BaseFont /{font} >>"),
        format!(
            "<< /Length {} >>\nstream\n{stream}\nendstream",
            stream.len()
        ),
    ];
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    bytes
}
fn directory(name: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "artifacts/markup-043-{}-{}-{name}",
        std::env::consts::ARCH,
        std::process::id()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn write_image(path: &std::path::Path, raster: &crabpdf::document::PageRaster) {
    image::save_buffer(
        path,
        &raster.rgba,
        raster.width as u32,
        raster.height as u32,
        image::ColorType::Rgba8,
    )
    .unwrap();
}

#[test]
#[ignore = "Requires the native PDFium 7881 library"]
fn underline_stays_below_ink_across_fonts_sizes_crop_rotation_export_and_undo() {
    let pdfium = bind_pdfium().unwrap();
    let dir = directory("underlines");
    for font in ["Helvetica", "Times-Italic", "Courier"] {
        for size in [6, 12, 24] {
            for rotation in [0, 90, 180, 270] {
                let case = format!("{font}-{size}-{rotation}");
                let source = dir.join(format!("{case}.pdf"));
                let bytes = sample(font, size, rotation);
                std::fs::write(&source, &bytes).unwrap();
                let native = pdfium.load_pdf_from_file(&source, None).unwrap();
                let page = native.pages().get(0).unwrap();
                let text = page.text().unwrap();
                let chars = text.chars();
                let bottom = (0..7)
                    .map(|i| chars.get(i).unwrap().tight_bounds().unwrap().bottom().value)
                    .fold(f32::INFINITY, f32::min);
                let mut engine =
                    PdfiumEngine::with_data_directory(&pdfium, dir.join(format!("profile-{case}")));
                engine.open(&source).unwrap();
                let request = key(0, Zoom::Percent(150));
                let baseline = engine.render(request).unwrap();
                engine
                    .add_markup(TextMarkup {
                        page: 0,
                        start: 0,
                        end: 6,
                        kind: MarkupKind::Underline,
                        color: [230, 20, 40],
                    })
                    .unwrap();
                let marked = engine.render(request).unwrap();
                let config = PdfRenderConfig::new()
                    .set_fixed_size(marked.width as i32, marked.height as i32);
                let pixel = |x, y| {
                    page.points_to_pixels(PdfPoints::new(x), PdfPoints::new(y), &config)
                        .unwrap()
                };
                let origin = pixel(40.0, bottom);
                let up = pixel(40.0, bottom + 100.0);
                let axis = ((up.0 - origin.0) as f32, (up.1 - origin.1) as f32);
                let length = axis.0.hypot(axis.1);
                let mut changed = 0;
                for (i, (before, after)) in baseline
                    .rgba
                    .chunks_exact(4)
                    .zip(marked.rgba.chunks_exact(4))
                    .enumerate()
                {
                    if before[..3]
                        .iter()
                        .zip(&after[..3])
                        .any(|(a, b)| a.abs_diff(*b) > 5)
                    {
                        let x = (i % marked.width) as f32 + 0.5;
                        let y = (i / marked.width) as f32 + 0.5;
                        let above = ((x - origin.0 as f32) * axis.0
                            + (y - origin.1 as f32) * axis.1)
                            / length;
                        assert!(above <= 1.0, "Underline crosses the ink: {case}, {above}px");
                        changed += 1;
                    }
                }
                assert!(changed > 10, "No visible underline: {case}");
                let export = dir.join(format!("{case}-export.pdf"));
                engine.save_copy(&export).unwrap();
                let mut exported =
                    PdfiumEngine::with_data_directory(&pdfium, dir.join(format!("export-{case}")));
                exported.open(&export).unwrap();
                assert_eq!(
                    exported.render(request).unwrap().rgba,
                    marked.rgba,
                    "Export changed appearance: {case}"
                );
                drop(engine);
                let mut reopened =
                    PdfiumEngine::with_data_directory(&pdfium, dir.join(format!("profile-{case}")));
                reopened.open(&source).unwrap();
                assert_eq!(
                    reopened.render(request).unwrap().rgba,
                    marked.rgba,
                    "Reopen: {case}"
                );
                reopened.undo_markup().unwrap();
                assert_eq!(
                    reopened.render(request).unwrap().rgba,
                    baseline.rgba,
                    "Undo: {case}"
                );
                assert_eq!(std::fs::read(&source).unwrap(), bytes);
                if size == 12 && rotation == 0 {
                    write_image(&dir.join(format!("{font}-before.png")), &baseline);
                    write_image(&dir.join(format!("{font}-underlined.png")), &marked);
                }
            }
        }
    }
}

#[test]
#[ignore = "Requires the native PDFium 7881 library"]
fn highlights_follow_lines_leave_column_gaps_and_keep_black_ink_readable() {
    let pdfium = bind_pdfium().unwrap();
    let dir = directory("highlights");
    let source = dir.join("columns.pdf");
    std::fs::write(&source, sample("Times-Italic", 12, 0)).unwrap();
    let native = pdfium.load_pdf_from_file(&source, None).unwrap();
    let page = native.pages().get(0).unwrap();
    let text = page.text().unwrap();
    let mut engine = PdfiumEngine::with_data_directory(&pdfium, dir.join("data"));
    engine.open(&source).unwrap();
    let request = key(0, Zoom::Percent(150));
    let before = engine.render(request).unwrap();
    engine
        .add_markup(TextMarkup {
            page: 0,
            start: 0,
            end: text.len() as usize - 1,
            kind: MarkupKind::Highlight,
            color: [255, 230, 50],
        })
        .unwrap();
    let after = engine.render(request).unwrap();
    let config = PdfRenderConfig::new().set_fixed_size(after.width as i32, after.height as i32);
    let rgb = |x, y| {
        let (x, y) = page
            .points_to_pixels(PdfPoints::new(x), PdfPoints::new(y), &config)
            .unwrap();
        let index = (y as usize * after.width + x as usize) * 4;
        &after.rgba[index..index + 3]
    };
    assert_eq!(rgb(180.0, 185.0), &[255, 255, 255], "Column gap painted");
    assert_eq!(rgb(55.0, 150.0), &[255, 255, 255], "Line gap painted");
    let mut tinted = 0;
    let mut black = 0;
    for (old, new) in before.rgba.chunks_exact(4).zip(after.rgba.chunks_exact(4)) {
        if old[..3] == [255, 255, 255] && new[2] < 240 {
            assert!(new[2] > 140, "Highlight too opaque: {new:?}");
            tinted += 1;
        }
        if old[..3].iter().all(|v| *v < 24) {
            assert!(new[..3].iter().all(|v| *v < 29));
            black += 1;
        }
    }
    assert!(
        tinted > 500 && black > 30,
        "Tinted {tinted}, dark ink {black}"
    );
    let export = dir.join("highlight-export.pdf");
    engine.save_copy(&export).unwrap();
    let exported = pdfium.load_pdf_from_file(&export, None).unwrap();
    let exported_page = exported.pages().get(0).unwrap();
    let annotation = exported_page.annotations().get(0).unwrap();
    assert_eq!(
        annotation.attachment_points().len(),
        3,
        "Lines or columns joined incorrectly"
    );
    write_image(&dir.join("highlight.png"), &after);
    engine.undo_markup().unwrap();
    assert_eq!(engine.render(request).unwrap().rgba, before.rgba);
}
