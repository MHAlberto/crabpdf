use crabpdf::document::{
    ErrorKind, LinkTarget, PageColors, RasterRegion, RenderKey, SearchRequest, Zoom,
};
use crabpdf::engine::{PdfEngine, PdfiumEngine, bind_pdfium};
use crabpdf::worker::{PdfWorker, WorkerEvent};
use std::path::PathBuf;
use std::time::Duration;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn key(page: usize, zoom: Zoom) -> RenderKey {
    RenderKey {
        document_id: 1,
        revision: 0,
        page,
        zoom,
        viewport_width: 600,
        density_milli: 1000,
        bitmap_side_limit: 8192,
        viewport_height: 700,
        rotation: 0,
        colors: crabpdf::document::PageColors::Original,
        region: None,
    }
}

#[test]
#[ignore = "Requires the native PDFium 7881 library"]
fn common_zoom_and_dpi_keep_logical_and_physical_resolution_distinct() {
    let pdfium = bind_pdfium().unwrap();
    let mut engine = PdfiumEngine::new(&pdfium);
    engine.open(&fixture("reading-sample.pdf")).unwrap();
    for zoom in [100, 125, 150, 200] {
        for density in [1000, 1250, 1500, 2000] {
            let request = RenderKey {
                density_milli: density,
                ..key(0, Zoom::Percent(zoom))
            };
            let raster = engine.render(request).unwrap();
            let logical = 800.0 * f32::from(zoom) / 100.0;
            assert!((raster.logical_width - logical).abs() < 0.01);
            if !raster.resolution_limited {
                assert_eq!(
                    raster.width,
                    (logical * density as f32 / 1000.0).round() as usize
                );
            }
            let tile = engine
                .render(RenderKey {
                    region: Some(RasterRegion {
                        x: 0,
                        y: 0,
                        width: 128,
                        height: 128,
                    }),
                    ..request
                })
                .unwrap();
            assert_eq!((tile.width, tile.height), (128, 128));
            assert!(!tile.resolution_limited);
            assert!((tile.logical_width - logical).abs() < 0.01);
        }
    }
}

#[test]
#[ignore = "Requires the native PDFium 7881 library"]
fn library_cache_survives_restart_and_recovers_a_corrupt_cover() {
    let pdfium = bind_pdfium().unwrap();
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "artifacts/cover-test-{}-{}",
        std::env::consts::ARCH,
        std::process::id()
    ));
    let first =
        crabpdf::library::load(&pdfium, &fixture("reading-sample.pdf"), Some(&directory)).unwrap();
    assert!(first.width <= 192 && first.height <= 256);
    let file = std::fs::read_dir(&directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let second =
        crabpdf::library::load(&pdfium, &fixture("reading-sample.pdf"), Some(&directory)).unwrap();
    assert_eq!(first.rgba, second.rgba);
    std::fs::write(&file, b"corrupt cached PNG").unwrap();
    let repaired =
        crabpdf::library::load(&pdfium, &fixture("reading-sample.pdf"), Some(&directory)).unwrap();
    assert_eq!(first.rgba, repaired.rgba);
    assert!(std::fs::read(&file).unwrap().starts_with(b"\x89PNG"));
}

#[test]
#[ignore = "Requires the native PDFium 7881 library; run scripts/Get-Pdfium.ps1 first"]
fn real_pdf_render_navigation_zoom_and_errors() {
    let pdfium = bind_pdfium().expect("PDFium must be installed for native tests");
    let mut engine = PdfiumEngine::new(&pdfium);
    let info = engine.open(&fixture("reading-sample.pdf")).unwrap();
    assert_eq!(info.page_count, 2);
    let first = engine.render(key(0, Zoom::FitWidth)).unwrap();
    assert_eq!((first.width, first.height), (600, 800));
    assert_eq!(first.rgba.len(), first.width * first.height * 4);
    // Real text and paths yield non-white pixels, including the sample's accent.
    assert!(
        first
            .rgba
            .chunks_exact(4)
            .any(|c| c[0] < 100 && c[1] < 150 && c[2] > c[0])
    );
    let second = engine.render(key(1, Zoom::FitWidth)).unwrap();
    assert_eq!((second.width, second.height), (600, 375));
    let color_offset = (220 * second.width + 120) * 4;
    let pixel = &second.rgba[color_offset..color_offset + 4];
    assert!(
        pixel[2] > 180 && pixel[0] < 70,
        "blue panel must remain blue, got {pixel:?}"
    );
    let enlarged = engine.render(key(1, Zoom::Percent(200))).unwrap();
    assert!(enlarged.width > second.width * 3);
    let mut hidpi = key(0, Zoom::FitWidth);
    hidpi.density_milli = 2000;
    let sharp = engine.render(hidpi).unwrap();
    assert_eq!((sharp.width, sharp.height), (1200, 1600));
    assert_eq!(sharp.logical_width, 600.0);
    assert_eq!(
        engine.render(key(2, Zoom::FitWidth)).err().unwrap().kind,
        ErrorKind::Render
    );
    assert_eq!(
        engine
            .open(&fixture("does-not-exist.pdf"))
            .unwrap_err()
            .kind,
        ErrorKind::File
    );
    assert_eq!(
        engine
            .open(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .unwrap_err()
            .kind,
        ErrorKind::InvalidPdf
    );
    assert_eq!(
        engine
            .open(&fixture("protected-sample.pdf"))
            .unwrap_err()
            .kind,
        ErrorKind::Protected
    );
    // Confirm this fixture is a valid encrypted PDF with a known test password.
    let unlocked = pdfium
        .load_pdf_from_file(&fixture("protected-sample.pdf"), Some("reader-test"))
        .unwrap();
    assert_eq!(unlocked.pages().len(), 2);
    // Failed opens cannot corrupt the previously held native document.
    assert!(engine.render(key(0, Zoom::FitWidth)).is_ok());
}

#[test]
#[ignore = "Requires the native PDFium 7881 library"]
fn worker_loads_and_renders_real_document() {
    let worker = PdfWorker::spawn(|| {}).unwrap();
    worker.open(42, fixture("reading-sample.pdf")).unwrap();
    match worker.events.recv_timeout(Duration::from_secs(15)).unwrap() {
        WorkerEvent::Opened(42, Ok(info)) => assert_eq!(info.page_count, 2),
        WorkerEvent::Unavailable(error) => panic!("{error}"),
        _ => panic!("Expected document metadata"),
    }
    let mut request = key(1, Zoom::FitWidth);
    request.document_id = 42;
    worker.render(request).unwrap();
    loop {
        match worker.events.recv_timeout(Duration::from_secs(15)).unwrap() {
            WorkerEvent::Details(42, Ok(details)) => {
                assert_eq!(details.first_page_size.width, 600.0)
            }
            WorkerEvent::Rendered(received, Ok(page)) => {
                assert_eq!(received, request);
                assert_eq!((page.width, page.height), (600, 375));
                break;
            }
            _ => panic!("Expected document details or a real raster"),
        }
    }
}

#[test]
#[ignore = "Requires the native PDFium 7881 library"]
fn displayed_raster_releases_cpu_copy_and_can_be_rendered_again() {
    let worker = PdfWorker::spawn(|| {}).unwrap();
    worker.open(42, fixture("reading-sample.pdf")).unwrap();
    let mut request = key(1, Zoom::FitWidth);
    request.document_id = 42;
    worker.render(request).unwrap();
    let first = loop {
        match worker.events.recv_timeout(Duration::from_secs(15)).unwrap() {
            WorkerEvent::Opened(42, Ok(_)) | WorkerEvent::Details(42, Ok(_)) => {}
            WorkerEvent::Rendered(received, Ok(page)) => {
                assert_eq!(received, request);
                break page;
            }
            _ => panic!("Expected a successful open, details or raster"),
        }
    };
    let hash = crc32fast::hash(&first.rgba);
    let weak = std::sync::Arc::downgrade(&first);
    drop(first);
    // The worker's CPU cache is the only remaining owner before upload acknowledgement.
    assert!(weak.upgrade().is_some());
    worker.release_render(request).unwrap();
    worker.render(request).unwrap();
    match worker.events.recv_timeout(Duration::from_secs(15)).unwrap() {
        WorkerEvent::Rendered(received, Ok(page)) => {
            assert_eq!(received, request);
            assert!(weak.upgrade().is_none(), "Old RGBA buffer must be freed");
            assert_eq!(crc32fast::hash(&page.rgba), hash);
        }
        _ => panic!("Expected a new raster after release"),
    }
}

#[test]
#[ignore = "Requires the native PDFium 7881 library"]
fn native_text_search_outline_links_password_and_save() {
    let pdfium = bind_pdfium().unwrap();
    let mut engine = PdfiumEngine::new(&pdfium);
    engine.open(&fixture("features-sample.pdf")).unwrap();
    let details = engine.details().unwrap();
    assert_eq!(details.outline.len(), 2);
    assert_eq!(details.outline[1].page, Some(1));
    assert!(
        details
            .properties
            .iter()
            .any(|(name, value)| name == "Título" && value.contains("interactive"))
    );
    let content = engine.page_content(0).unwrap();
    assert!(
        content
            .text(0, usize::MAX)
            .contains("A little room to read.")
    );
    assert_eq!(content.links.len(), 2);
    assert!(
        content
            .links
            .iter()
            .any(|link| matches!(link.target, LinkTarget::Page(1)))
    );
    assert!(content.links.iter().any(
        |link| matches!(&link.target,LinkTarget::Uri(uri) if uri == "https://example.org/reader")
    ));
    let mut search = SearchRequest {
        document_id: 1,
        generation: 1,
        query: "reader".into(),
        match_case: false,
        whole_word: true,
    };
    assert_eq!(engine.search_page(0, &search).unwrap().len(), 3);
    search.match_case = true;
    assert_eq!(engine.search_page(0, &search).unwrap().len(), 1);
    search.query = "read".into();
    assert_eq!(engine.search_page(0, &search).unwrap().len(), 1);
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("artifacts/native-copy.pdf");
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    engine.save_copy(&output).unwrap();
    assert_eq!(
        pdfium
            .load_pdf_from_file(&output, None)
            .unwrap()
            .pages()
            .len(),
        2
    );
    assert!(engine.save_copy(&fixture("features-sample.pdf")).is_err());
    assert!(
        engine
            .open_with_password(&fixture("protected-sample.pdf"), Some("wrong"))
            .is_err()
    );
    assert_eq!(
        engine
            .open_with_password(&fixture("protected-sample.pdf"), Some("reader-test"))
            .unwrap()
            .page_count,
        2
    );
}

#[test]
#[ignore = "Requires the native PDFium 7881 library"]
fn tiles_match_full_native_raster_at_every_rotation() {
    let pdfium = bind_pdfium().unwrap();
    let mut engine = PdfiumEngine::new(&pdfium);
    engine.open(&fixture("reading-sample.pdf")).unwrap();
    for rotation in 0..4 {
        for colors in [
            PageColors::Original,
            PageColors::Inverted,
            PageColors::Grayscale,
        ] {
            let mut request = key(0, Zoom::FitWidth);
            request.rotation = rotation;
            request.colors = colors;
            let full = engine.render(request).unwrap();
            let mut drift = 0u64;
            let mut bytes = 0usize;
            for y in (0..full.height).step_by(256) {
                for x in (0..full.width).step_by(256) {
                    let width = 256.min(full.width - x);
                    let height = 256.min(full.height - y);
                    let tile = engine
                        .render(RenderKey {
                            region: Some(RasterRegion {
                                x: x as u32,
                                y: y as u32,
                                width: width as u32,
                                height: height as u32,
                            }),
                            ..request
                        })
                        .unwrap();
                    assert_eq!((tile.width, tile.height), (width, height));
                    for row in 0..height {
                        let source = &full.rgba[((y + row) * full.width + x) * 4
                            ..((y + row) * full.width + x + width) * 4];
                        let actual = &tile.rgba[row * width * 4..(row + 1) * width * 4];
                        drift += source
                            .iter()
                            .zip(actual)
                            .map(|(a, b)| u64::from(a.abs_diff(*b)))
                            .sum::<u64>();
                        bytes += actual.len();
                    }
                }
            }
            assert!(
                drift as f64 / (bytes as f64) < 2.0,
                "Tile drift at rotation {rotation}: {drift}/{bytes}"
            );
        }
    }
    let zoomed = engine
        .render(RenderKey {
            region: Some(RasterRegion {
                x: 1024,
                y: 1024,
                width: 1024,
                height: 1024,
            }),
            ..key(0, Zoom::Percent(6400))
        })
        .unwrap();
    assert_eq!(zoomed.rgba.len(), 4 * 1024 * 1024);
    assert!(!zoomed.resolution_limited);
    assert_eq!(zoomed.logical_width, 51200.0);
}

#[test]
#[ignore = "Requires the native PDFium 7881 library"]
fn text_bounds_follow_crop_box_and_intrinsic_rotation() {
    let pdfium = bind_pdfium().unwrap();
    let mut engine = PdfiumEngine::new(&pdfium);
    engine.open(&fixture("geometry-sample.pdf")).unwrap();
    let raster = engine.render(key(0, Zoom::FitWidth)).unwrap();
    assert_eq!((raster.width, raster.height), (600, 450));
    let content = engine.page_content(0).unwrap();
    let glyph = content.glyphs.iter().find(|g| g.text == "R").unwrap();
    let bounds = glyph.bounds.unwrap();
    assert!(bounds.left > 0.9 && bounds.top < 0.1, "{bounds:?}");
    let left = (bounds.left * raster.width as f32).floor().max(0.0) as usize;
    let top = (bounds.top * raster.height as f32).floor().max(0.0) as usize;
    let right = (bounds.right * raster.width as f32)
        .ceil()
        .min(raster.width as f32) as usize;
    let bottom = (bounds.bottom * raster.height as f32)
        .ceil()
        .min(raster.height as f32) as usize;
    assert!(
        (top..bottom).any(|y| (left..right).any(|x| raster.rgba[(y * raster.width + x) * 4] < 200)),
        "Text bounds must overlap rendered text"
    );
}

#[test]
#[ignore = "Requires the native PDFium 7881 library"]
fn typed_titles_prefer_import_metadata_and_fall_back_without_display_labels() {
    fn pdf(title: Option<&str>) -> Vec<u8> {
        let stream = "q 0.9 0.95 1 rg 10 10 100 100 re f Q";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Contents 4 0 R /Resources << >> >>".into(),
            format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
            title.map_or_else(|| "<< >>".into(), |title| format!("<< /Title ({title}) >>")),
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
            format!("trailer\n<< /Size 6 /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n")
                .as_bytes(),
        );
        bytes
    }
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("artifacts/metadata-test-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let pdfium = bind_pdfium().unwrap();
    for (index, (native, imported, expected)) in [
        (None, None, None),
        (Some("PDF title"), None, Some("PDF title")),
        (
            Some("PDF title"),
            Some("Imported title"),
            Some("Imported title"),
        ),
        (Some("PDF title"), Some(""), Some("PDF title")),
    ]
    .into_iter()
    .enumerate()
    {
        let path = directory.join(format!("{index}.pdf"));
        std::fs::write(&path, pdf(native)).unwrap();
        if let Some(title) = imported {
            std::fs::write(
                path.with_extension("json"),
                serde_json::to_vec(&crabpdf::formats::ImportInfo {
                    title: title.into(),
                    ..Default::default()
                })
                .unwrap(),
            )
            .unwrap();
        }
        let mut engine = PdfiumEngine::with_data_directory(&pdfium, directory.join("data"));
        engine.open(&path).unwrap();
        let details = engine.details().unwrap();
        assert_eq!(details.title.as_deref(), expected, "case {index}");
        assert_eq!(details.annotation_count, 0);
        assert!(details.annotation_warning.is_none());
    }
}
