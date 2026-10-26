//! Enforce the workspace dependency direction and the UI-free inner libraries.
use std::{collections::BTreeSet, path::Path, process::Command};

#[test]
fn workspace_dependencies_follow_the_selected_architecture() {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--offline",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Cargo metadata");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let packages = metadata["packages"].as_array().unwrap();
    let expected = [
        ("crabpdf-core", vec!["serde"]),
        ("crabpdf-application", vec!["crabpdf-core", "serde"]),
        (
            "crabpdf-runtime",
            vec![
                "crabpdf-core",
                "crabpdf-application",
                "pdfium-render",
                "serde",
                "toml",
                "image",
                "roxmltree",
                "serde_json",
                "crc32fast",
            ],
        ),
    ];
    assert_eq!(metadata["workspace_members"].as_array().unwrap().len(), 4);
    for (name, allowed) in expected {
        let package = packages.iter().find(|p| p["name"] == name).unwrap();
        let actual: BTreeSet<_> = package["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["kind"].is_null())
            .map(|d| d["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            actual,
            allowed.into_iter().collect(),
            "{name} changed its production dependencies"
        );
    }
}

#[test]
fn inner_library_sources_do_not_import_desktop_or_io_services() {
    // A source-level guard complements the dependency graph: std I/O has no Cargo dependency.
    // This intentionally checks qualified service paths, not all possible Rust aliasing.
    fn check(directory: &Path) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                for forbidden in [
                    "std::fs",
                    "fs::",
                    "std::env",
                    "env::",
                    "std::process",
                    "process::",
                    "std::thread",
                    "thread::",
                    "eframe::",
                    "egui::",
                    "rfd::",
                    "pdfium_render::",
                    "crabpdf_runtime::",
                ] {
                    assert!(
                        !source.contains(forbidden),
                        "{} imports {forbidden}",
                        path.display()
                    );
                }
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    check(&root.join("crates/core/src"));
    check(&root.join("crates/application/src"));
}
