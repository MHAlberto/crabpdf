use crabpdf::app_paths::AppPaths;
use crabpdf::engine::bind_pdfium;
use crabpdf::formats;
use crabpdf::worker::{PdfWorker, WorkerEvent};
use std::path::PathBuf;
use std::time::Duration;

#[test]
#[ignore = "Requires PDFium, format tools and generated EPUB samples"]
fn rejected_but_checksum_valid_cached_pdf_is_rebuilt_once() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = root.join("artifacts/format-samples/book.epub");
    let original = std::fs::read(&source).unwrap();
    let data = root.join("artifacts").join(format!(
        "rejected-cache-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let prepared = formats::prepare_in(&source, &data).unwrap();
    let bogus = b"%PDF-1.7\n%%EOF\n";
    std::fs::write(&prepared, bogus).unwrap();
    let manifest_path = prepared.with_extension("manifest");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["pdf"]["bytes"] = serde_json::json!(bogus.len());
    manifest["pdf"]["crc32"] = serde_json::json!(crc32fast::hash(bogus));
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();

    let pdfium = bind_pdfium().unwrap();
    assert!(pdfium.load_pdf_from_file(&prepared, None).is_err());
    // A second cache lookup accepts its header, EOF, length and CRC.
    assert_eq!(formats::prepare_in(&source, &data).unwrap(), prepared);

    let worker = PdfWorker::spawn_with_paths(|| {}, AppPaths { config: None, data }).unwrap();
    worker.open(71, source.clone()).unwrap();
    loop {
        match worker.events.recv_timeout(Duration::from_secs(45)).unwrap() {
            WorkerEvent::Opened(71, result) => {
                assert!(result.is_ok(), "{result:?}");
                break;
            }
            WorkerEvent::Unavailable(error) => panic!("{error:?}"),
            _ => {}
        }
    }
    assert!(pdfium.load_pdf_from_file(&prepared, None).is_ok());
    assert_eq!(std::fs::read(&source).unwrap(), original);
}
