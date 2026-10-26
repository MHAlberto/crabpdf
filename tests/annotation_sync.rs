use crabpdf::document::{MarkupKind, TextMarkup};
use crabpdf::engine::{PdfEngine, PdfiumEngine, bind_pdfium};
use pdfium_render::prelude::*;
use std::path::PathBuf;

#[test]
#[ignore = "Requires PDFium; exercises an independent process and an isolated profile"]
fn export_includes_a_mark_committed_by_another_process() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts")
        .join(format!(
            "annotation-process-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("source.pdf");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/geometry-sample.pdf"),
        &source,
    )
    .unwrap();
    let original = std::fs::read(&source).unwrap();
    let data = directory.join("profile");

    let pdfium = bind_pdfium().unwrap();
    let mut exporter = PdfiumEngine::with_data_directory(&pdfium, data.clone());
    exporter.open(&source).unwrap();

    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "helper_commits_a_markup"])
        .env("CRABPDF_TEST_MARK_SOURCE", &source)
        .env("CRABPDF_TEST_MARK_DATA", &data)
        .status()
        .unwrap();
    assert!(status.success());

    let output = directory.join("export.pdf");
    exporter.save_copy(&output).unwrap();
    let copy = pdfium.load_pdf_from_file(&output, None).unwrap();
    assert_eq!(
        copy.pages()
            .get(0)
            .unwrap()
            .annotations()
            .iter()
            .filter(|annotation| annotation.creator().as_deref() == Some("CrabPDF"))
            .count(),
        1
    );
    assert_eq!(std::fs::read(&source).unwrap(), original);
}

#[test]
#[ignore = "Invoked by the parent test with a private source and profile"]
fn helper_commits_a_markup() {
    let (Some(source), Some(data)) = (
        std::env::var_os("CRABPDF_TEST_MARK_SOURCE"),
        std::env::var_os("CRABPDF_TEST_MARK_DATA"),
    ) else {
        return;
    };
    let pdfium = bind_pdfium().unwrap();
    let mut writer = PdfiumEngine::with_data_directory(&pdfium, PathBuf::from(data));
    writer.open(&PathBuf::from(source)).unwrap();
    let start = writer
        .page_content(0)
        .unwrap()
        .glyphs
        .iter()
        .find(|glyph| glyph.bounds.is_some() && glyph.text.chars().any(char::is_alphanumeric))
        .unwrap()
        .index;
    assert_eq!(
        writer
            .add_markup(TextMarkup {
                page: 0,
                start,
                end: start + 4,
                kind: MarkupKind::Highlight,
                color: [255, 230, 50],
            })
            .unwrap(),
        1
    );
}
