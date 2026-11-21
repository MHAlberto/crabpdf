//! Optional recognition test through the SDK and real Tesseract.
use crabpdf::{
    desktop::extensions::{Invocation, Repository, RunningExtension},
    document::Bounds,
    engine::{PdfEngine, PdfiumEngine, bind_pdfium},
};
use crabpdf_extension_api::Region;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[test]
#[ignore = "Requires PDFium, the text-ocr package and Tesseract with English data"]
fn recognizes_text_from_a_pdf_region() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let data = root.join(format!("artifacts/text-ocr-proof-{}", std::process::id()));
    let repo = Repository::new(&data);
    let package_source = root.join("artifacts/extensions/text-ocr");
    repo.install(&package_source).unwrap();
    let package = &repo.list().unwrap()[0];
    repo.enable(package, true).unwrap();
    let package = repo.list().unwrap().remove(0);
    let pdfium = bind_pdfium().unwrap();
    let mut engine = PdfiumEngine::with_data_directory(&pdfium, data);
    engine
        .open(&root.join("tests/fixtures/reading-sample.pdf"))
        .unwrap();
    let crop = Bounds {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    };
    let image = engine.capture_selection(0, crop, 0).unwrap();
    let input = Invocation {
        command: package
            .manifest
            .commands
            .iter()
            .find(|command| command.id == "recognize-eng")
            .unwrap()
            .clone(),
        page: 1,
        region: Region {
            left: crop.left,
            top: crop.top,
            right: crop.right,
            bottom: crop.bottom,
        },
        text: String::new(),
        image: Some((image.png, image.width, image.height)),
    };
    let running = RunningExtension::spawn(repo, package, input, || {}).unwrap();
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        if let Some(result) = running.poll() {
            let result = result.unwrap();
            assert!(
                result.text.to_lowercase().contains("reader"),
                "Unexpected local OCR: {}",
                result.text
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Tesseract did not complete in time"
        );
        std::thread::sleep(Duration::from_millis(30));
    }
}
