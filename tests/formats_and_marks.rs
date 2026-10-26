use crabpdf::document::{MarkupKind, PageColors, RenderKey, TextMarkup, Zoom};
use crabpdf::engine::{PdfEngine, PdfiumEngine, bind_pdfium};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn key(page: usize) -> RenderKey {
    RenderKey {
        document_id: 1,
        revision: 0,
        page,
        zoom: Zoom::Percent(100),
        viewport_width: 800,
        viewport_height: 800,
        density_milli: 1000,
        bitmap_side_limit: 1024,
        rotation: 0,
        colors: PageColors::Original,
        region: None,
    }
}
fn isolated_file(name: &str) -> PathBuf {
    let directory = root().join(format!(
        "artifacts/marks-test-{}-{}",
        std::env::consts::ARCH,
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let output = directory.join(name);
    std::fs::copy(root().join("tests/fixtures/geometry-sample.pdf"), &output).unwrap();
    output
}

#[test]
#[ignore = "Requires PDFium and packaged format decoders; generate New-FormatSamples.ps1"]
fn basic_formats_open_render_and_keep_source_unchanged() {
    let pdfium = bind_pdfium().unwrap();
    let image = image::RgbaImage::from_pixel(32, 32, image::Rgba([90, 150, 220, 255]));
    for name in ["image.ico", "image.tga"] {
        image
            .save(root().join("artifacts/format-samples").join(name))
            .unwrap();
    }
    let bmp = std::fs::read(root().join("artifacts/format-samples/image.bmp")).unwrap();
    std::fs::write(
        root().join("artifacts/format-samples/image.dib"),
        &bmp[14..],
    )
    .unwrap();
    let mut errors = Vec::new();
    for (name, text, pages) in [
        ("book.epub", true, 2),
        ("book.fb2", true, 1),
        ("book.fb2z", true, 1),
        ("book.fb2.zip", true, 1),
        ("book.fbz", true, 1),
        ("book.zfb2", true, 1),
        ("book.xps", true, 2),
        ("book.oxps", true, 2),
        ("text.txt", true, 1),
        ("text.log", true, 1),
        ("text.nfo", true, 1),
        ("markdown.md", true, 1),
        ("help.chm", true, 1),
        ("book.djvu", true, 1),
        ("book.djv", true, 1),
        ("comic.cbz", false, 3),
        ("comic.cbr", false, 3),
        ("comic.rar", false, 3),
        ("comic.cb7", false, 3),
        ("comic.cbt", false, 3),
        ("comic.zip", false, 3),
        ("comic.7z", false, 3),
        ("comic.tar", false, 3),
        ("image.jpg", false, 1),
        ("image.bmp", false, 1),
        ("image.gif", false, 1),
        ("image.tif", false, 1),
        ("multi.tiff", false, 2),
        ("image.svg", false, 1),
        ("image.ico", false, 1),
        ("image.tga", false, 1),
        ("image.dib", false, 1),
    ] {
        let input = root().join("artifacts/format-samples").join(name);
        let before = std::fs::read(&input).unwrap();
        let mut engine = PdfiumEngine::new(&pdfium);
        let result = (|| {
            let info = engine
                .open(&input)
                .map_err(|error| format!("{}: {:?}", error, error.details))?;
            if info.page_count < pages {
                return Err(format!("Expected {pages} pages, got {}", info.page_count));
            }
            let image = engine.render(key(0)).map_err(|error| error.to_string())?;
            if image.width == 0
                || image.height == 0
                || image
                    .rgba
                    .chunks_exact(4)
                    .all(|p| p[..3] == [255, 255, 255])
            {
                return Err("Empty render".into());
            }
            if text
                && engine
                    .page_content(0)
                    .map_err(|error| error.to_string())?
                    .glyphs
                    .is_empty()
            {
                return Err("Lost selectable text".into());
            }
            if name == "book.epub" {
                let details = engine.details().unwrap();
                if details.outline.len() != 2 || details.outline[1].page != Some(1) {
                    return Err("EPUB table of contents lost".into());
                }
                let text: String = engine
                    .page_content(0)
                    .map_err(|error| error.to_string())?
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.text.as_str())
                    .collect();
                if !text.contains("capítulo") {
                    return Err(format!("EPUB Unicode text damaged: {text}"));
                }
                if !details
                    .properties
                    .iter()
                    .any(|(name, value)| name == "Título" && value == "CrabPDF EPUB")
                {
                    return Err("EPUB title lost".into());
                }
            }
            if name.starts_with("comic.") {
                let blue = engine.render(key(0)).unwrap();
                let green = engine.render(key(1)).unwrap();
                let pixel = |image: &crabpdf::document::PageRaster| {
                    <[u8; 3]>::try_from(&image.rgba[(image.width * image.height / 2) * 4..][..3])
                        .unwrap()
                };
                if pixel(&blue) == pixel(&green) {
                    return Err("Comic page order/content did not change".into());
                }
                if pixel(&blue)[2] <= pixel(&blue)[0] {
                    return Err("Natural order did not put page1 first".into());
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            errors.push(format!("{name}: {error}"));
        }
        assert_eq!(
            std::fs::read(&input).unwrap(),
            before,
            "Modified source {name}"
        );
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
#[ignore = "Requires PDFium and libmobi format decoders"]
fn kindle_huffman_kf8_and_drm_are_distinguished() {
    let pdfium = bind_pdfium().unwrap();
    let samples = root().join("native/format-sources/libmobi/tests/samples");
    for name in [
        "sample-unicode-huffdic.mobi",
        "sample-unicode-uncompressed.mobi",
        "sample-cp1252.mobi",
        "sample-textread.mobi",
    ] {
        let mut engine = PdfiumEngine::new(&pdfium);
        let info = engine
            .open(&samples.join(name))
            .unwrap_or_else(|error| panic!("{name}: {error} {:?}", error.details));
        assert!(info.page_count > 0);
        assert!(!engine.page_content(0).unwrap().glyphs.is_empty(), "{name}");
        assert!(!engine.render(key(0)).unwrap().rgba.is_empty());
    }
    let mut engine = PdfiumEngine::new(&pdfium);
    assert_eq!(
        engine
            .open(&samples.join("sample-drm-v1.mobi"))
            .unwrap_err()
            .kind,
        crabpdf::document::ErrorKind::Protected
    );
}

#[test]
#[ignore = "Requires PDFium"]
fn highlights_and_underlines_persist_export_and_undo_in_crop_rotated_pages() {
    let pdfium = bind_pdfium().unwrap();
    let source = isolated_file("marks.pdf");
    let bytes = std::fs::read(&source).unwrap();
    let mut engine = PdfiumEngine::new(&pdfium);
    let info = engine.open(&source).unwrap();
    let colors = [
        [255, 230, 50],
        [90, 210, 100],
        [80, 170, 255],
        [255, 120, 190],
    ];
    let mut baseline = Vec::new();
    for page in 0..info.page_count.min(4) {
        baseline.push(engine.render(key(page)).unwrap().rgba);
        let content = engine.page_content(page).unwrap();
        let first = content
            .glyphs
            .iter()
            .find(|glyph| glyph.bounds.is_some() && glyph.text.chars().any(char::is_alphanumeric))
            .unwrap()
            .index;
        let mark = TextMarkup {
            page,
            start: first,
            end: first + 4,
            kind: if page % 2 == 0 {
                MarkupKind::Highlight
            } else {
                MarkupKind::Underline
            },
            color: colors[page % 4],
        };
        assert_eq!(engine.add_markup(mark).unwrap(), page + 1);
        let changed = engine.render(key(page)).unwrap();
        assert_ne!(
            changed.rgba, baseline[page],
            "No visible markup on page {page}"
        );
        let selected: Vec<_> = content
            .glyphs
            .iter()
            .filter(|glyph| glyph.index >= first && glyph.index <= first + 4)
            .filter_map(|glyph| glyph.bounds)
            .collect();
        let left = selected
            .iter()
            .map(|rect| rect.left)
            .fold(f32::INFINITY, f32::min);
        let right = selected
            .iter()
            .map(|rect| rect.right)
            .fold(f32::NEG_INFINITY, f32::max);
        let top = selected
            .iter()
            .map(|rect| rect.top)
            .fold(f32::INFINITY, f32::min);
        let bottom = selected
            .iter()
            .map(|rect| rect.bottom)
            .fold(f32::NEG_INFINITY, f32::max);
        let (x0, x1) = (
            (left * changed.width as f32).floor().max(0.0) as usize,
            (right * changed.width as f32)
                .ceil()
                .min(changed.width as f32) as usize,
        );
        let (y0, y1) = (
            (top * changed.height as f32).floor().max(0.0) as usize,
            (bottom * changed.height as f32)
                .ceil()
                .min(changed.height as f32) as usize,
        );
        let mut changed_pixels = 0;
        let mut columns = std::collections::HashSet::new();
        let mut rows = std::collections::HashSet::new();
        for y in y0..y1 {
            for x in x0..x1 {
                let index = (y * changed.width + x) * 4;
                if baseline[page][index..index + 3] != changed.rgba[index..index + 3] {
                    changed_pixels += 1;
                    columns.insert(x);
                    rows.insert(y);
                }
            }
        }
        if page % 2 == 0 {
            assert!(
                changed_pixels as f32 / ((x1 - x0) * (y1 - y0)) as f32 > 0.20,
                "Highlight doesn't cover the word on page {page}"
            );
        } else {
            let covered = if x1 - x0 >= y1 - y0 {
                columns.len() as f32 / (x1 - x0) as f32
            } else {
                rows.len() as f32 / (y1 - y0) as f32
            };
            assert!(
                covered > 0.60,
                "Underline doesn't span the word on page {page}"
            );
        }
    }
    let count = baseline.len();
    drop(engine);
    let mut engine = PdfiumEngine::new(&pdfium);
    engine.open(&source).unwrap();
    assert_eq!(
        engine
            .details()
            .unwrap()
            .properties
            .iter()
            .find(|(name, _)| name == "Marcas locales")
            .unwrap()
            .1,
        count.to_string()
    );
    let export = source.with_file_name("annotated-copy.pdf");
    engine.save_copy(&export).unwrap();
    let document = pdfium.load_pdf_from_file(&export, None).unwrap();
    for (page, color) in colors.iter().enumerate().take(count) {
        let page = document.pages().get(page as i32).unwrap();
        let annotation = page
            .annotations()
            .iter()
            .find(|annotation| annotation.creator().as_deref() == Some("CrabPDF"))
            .unwrap();
        let actual = annotation.stroke_color().unwrap();
        assert_eq!([actual.red(), actual.green(), actual.blue()], *color);
        assert!(annotation.is_printed());
        assert!(!page.text().unwrap().all().is_empty());
    }
    drop(document);
    for page in (0..count).rev() {
        assert_eq!(engine.undo_markup().unwrap(), (page, page));
        assert_eq!(engine.render(key(page)).unwrap().rgba, baseline[page]);
    }
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    assert!(
        engine
            .add_markup(TextMarkup {
                page: 0,
                start: usize::MAX,
                end: usize::MAX,
                kind: MarkupKind::Highlight,
                color: [255, 255, 0]
            })
            .is_err()
    );
    assert!(engine.save_copy(Path::new(&source)).is_err());
}

#[test]
#[ignore = "Requires PDFium"]
fn multiple_open_copies_do_not_overwrite_each_others_marks() {
    let pdfium = bind_pdfium().unwrap();
    let source = isolated_file("shared.pdf");
    let mut first = PdfiumEngine::new(&pdfium);
    let mut second = PdfiumEngine::new(&pdfium);
    first.open(&source).unwrap();
    second.open(&source).unwrap();
    let content = first.page_content(0).unwrap();
    let index = content
        .glyphs
        .iter()
        .find(|glyph| glyph.bounds.is_some() && glyph.text.chars().any(char::is_alphanumeric))
        .unwrap()
        .index;
    let mark = TextMarkup {
        page: 0,
        start: index,
        end: index + 4,
        kind: MarkupKind::Highlight,
        color: [255, 230, 50],
    };
    assert_eq!(first.add_markup(mark.clone()).unwrap(), 1);
    assert_eq!(
        second
            .add_markup(TextMarkup {
                color: [80, 170, 255],
                ..mark
            })
            .unwrap(),
        2
    );
    assert_eq!(first.refresh_marks().unwrap(), 2);
    assert_eq!(first.undo_markup().unwrap(), (0, 1));
    assert_eq!(second.refresh_marks().unwrap(), 1);
}

#[test]
#[ignore = "Requires PDFium"]
fn export_refreshes_marks_from_an_independent_open_document() {
    let pdfium = bind_pdfium().unwrap();
    let source = isolated_file("export-external-marks.pdf");
    let original = std::fs::read(&source).unwrap();
    let mut writer = PdfiumEngine::new(&pdfium);
    let mut exporter = PdfiumEngine::new(&pdfium);
    writer.open(&source).unwrap();
    exporter.open(&source).unwrap();
    let content = writer.page_content(0).unwrap();
    let index = content
        .glyphs
        .iter()
        .find(|g| g.bounds.is_some() && g.text.chars().any(char::is_alphanumeric))
        .unwrap()
        .index;
    writer
        .add_markup(TextMarkup {
            page: 0,
            start: index,
            end: index + 4,
            kind: MarkupKind::Highlight,
            color: [255, 230, 50],
        })
        .unwrap();
    let export = source.with_file_name("external-marks-copy.pdf");
    exporter.save_copy(&export).unwrap();
    let reopened = pdfium.load_pdf_from_file(&export, None).unwrap();
    assert_eq!(
        reopened
            .pages()
            .get(0)
            .unwrap()
            .annotations()
            .iter()
            .filter(|a| a.creator().as_deref() == Some("CrabPDF"))
            .count(),
        1
    );
    assert_eq!(std::fs::read(&source).unwrap(), original);
}

#[test]
#[ignore = "Requires PDFium"]
fn refresh_marks_on_another_page_changes_the_render_revision() {
    let pdfium = bind_pdfium().unwrap();
    let source = isolated_file("marks-multiple-pages.pdf");
    let mut first = PdfiumEngine::new(&pdfium);
    let mut second = PdfiumEngine::new(&pdfium);
    assert!(first.open(&source).unwrap().page_count > 1);
    second.open(&source).unwrap();
    let previous = second.render(key(0)).unwrap().rgba;
    let mark_for = |engine: &mut PdfiumEngine<'_>, page| {
        let content = engine.page_content(page).unwrap();
        let start = content
            .glyphs
            .iter()
            .find(|g| g.bounds.is_some() && g.text.chars().any(char::is_alphanumeric))
            .unwrap()
            .index;
        TextMarkup {
            page,
            start,
            end: start + 4,
            kind: MarkupKind::Highlight,
            color: [255, 230, 50],
        }
    };
    let first_mark = mark_for(&mut first, 0);
    first.add_markup(first_mark).unwrap();
    let second_mark = mark_for(&mut second, 1);
    second.add_markup(second_mark).unwrap();
    assert_eq!(second.annotation_state().count, 2);
    assert!(second.annotation_state().revision >= 2);
    assert_ne!(second.render(key(0)).unwrap().rgba, previous);
}

#[test]
#[ignore = "Requires PDFium"]
fn damaged_local_marks_do_not_prevent_reading_or_get_overwritten() {
    let pdfium = bind_pdfium().unwrap();
    let source = isolated_file("damaged-store.pdf");
    let store = crabpdf::annotations::path(&source).unwrap();
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    let corrupt = b"marks = [broken";
    std::fs::write(&store, corrupt).unwrap();
    let mut engine = PdfiumEngine::new(&pdfium);
    engine.open(&source).unwrap();
    assert!(!engine.details().unwrap().can_annotate);
    assert!(!engine.render(key(0)).unwrap().rgba.is_empty());
    assert!(!engine.page_content(0).unwrap().glyphs.is_empty());
    assert_eq!(std::fs::read(store).unwrap(), corrupt);
}
