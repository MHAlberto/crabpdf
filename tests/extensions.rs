use crabpdf::desktop::extensions::{Invocation, Repository, RunningExtension, target};
use crabpdf_extension_api::{ExtensionCommand, InputKind, Manifest, OutputKind, Region};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn root(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts/extension-tests")
        .join(format!(
            "{name}-{}-{:016x}",
            std::process::id(),
            getrandom::u64().unwrap()
        ));
    fs::create_dir_all(&directory).unwrap();
    directory
}
fn manifest() -> Manifest {
    Manifest {
        runtime: Default::default(),
        api_version: 1,
        id: "org.crabpdf.test".into(),
        name: "Prueba".into(),
        version: "0.1.0".into(),
        host_version: format!("={}", env!("CARGO_PKG_VERSION")),
        target: target(),
        executable: "tool".into(),
        description: "Fixture".into(),
        timer: None,
        panel: None,
        commands: vec![ExtensionCommand {
            id: "latex".into(),
            title: "Ecuación de prueba".into(),
            input: InputKind::Text,
            output: OutputKind::Latex,
        }],
    }
}
fn package(folder: &Path, manifest: &Manifest) {
    fs::create_dir_all(folder).unwrap();
    fs::write(
        folder.join("extension.toml"),
        toml::to_string(manifest).unwrap(),
    )
    .unwrap();
    fs::write(folder.join("tool"), b"test package").unwrap();
}
#[test]
fn versions_activate_independently_and_rejected_updates_preserve_the_previous_package() {
    let directory = root("versions");
    let repository = Repository::new(&directory.join("data"));
    let source = directory.join("source");
    let mut m = manifest();
    package(&source, &m);
    repository.install(&source).unwrap();
    let one = repository.list().unwrap().remove(0);
    assert!(!one.enabled);
    repository.enable(&one, true).unwrap();
    assert!(repository.install(&source).is_err());
    m.version = "0.2.0".into();
    package(&source, &m);
    repository.install(&source).unwrap();
    let versions = repository.list().unwrap();
    let two = versions
        .iter()
        .find(|p| p.manifest.version == "0.2.0")
        .unwrap()
        .clone();
    assert!(
        versions
            .iter()
            .find(|p| p.manifest.version == "0.1.0")
            .unwrap()
            .enabled
    );
    m.version = "0.3.0".into();
    package(&source, &m);
    let mut deep = source.clone();
    for _ in 0..18 {
        deep = deep.join("nested");
    }
    fs::create_dir_all(deep).unwrap();
    assert!(repository.install(&source).is_err());
    assert_eq!(repository.list().unwrap().len(), 2);
    repository.enable(&two, true).unwrap();
    assert_eq!(
        repository
            .list()
            .unwrap()
            .iter()
            .filter(|p| p.enabled)
            .count(),
        1
    );
    repository.remove(&one).unwrap();
    assert!(repository.list().unwrap()[0].enabled);
    repository.enable(&two, false).unwrap();
    repository.remove(&two).unwrap();
    assert!(repository.list().unwrap().is_empty());
    assert!(source.join("tool").exists());
}
#[test]
fn package_validation_rejects_traversal_wrong_platform_and_incompatible_api() {
    let directory = root("invalid");
    let repository = Repository::new(&directory.join("data"));
    let source = directory.join("source");
    for change in 0..5 {
        let mut m = manifest();
        match change {
            0 => m.executable = "../outside".into(),
            1 => m.target = "incompatible".into(),
            2 => m.api_version = 900,
            3 => m.host_version = ">=99".into(),
            _ => m.commands.push(m.commands[0].clone()),
        }
        package(&source, &m);
        assert!(repository.install(&source).is_err());
        assert!(repository.list().unwrap().is_empty());
    }
}
#[test]
fn corrupted_manifest_and_activation_leave_other_extensions_visible_and_recoverable() {
    let directory = root("recover");
    let data = directory.join("data");
    let repository = Repository::new(&data);
    let source = directory.join("source");
    let damaged = manifest();
    package(&source, &damaged);
    repository.install(&source).unwrap();
    let mut healthy = manifest();
    healthy.id = "org.crabpdf.healthy".into();
    package(&source, &healthy);
    repository.install(&source).unwrap();

    let installed = repository
        .list()
        .unwrap()
        .into_iter()
        .find(|p| p.manifest.id == damaged.id)
        .unwrap();
    repository.enable(&installed, true).unwrap();
    fs::write(installed.directory.join("extension.toml"), "[incomplete").unwrap();
    let state = data
        .join("extensions/enabled")
        .join(format!("{}.json", healthy.id));
    fs::write(&state, r#"{"partial":"#).unwrap();

    let inventory = repository.inventory().unwrap();
    assert_eq!(inventory.packages.len(), 1);
    assert_eq!(inventory.packages[0].manifest.id, healthy.id);
    assert!(!inventory.packages[0].enabled);
    assert_eq!(inventory.issues.len(), 2);
    repository.enable(&inventory.packages[0], true).unwrap();

    package(&source, &damaged);
    assert!(repository.install(&source).is_err());
    repository
        .remove_version(&damaged.id, &damaged.version)
        .unwrap();
    repository.install(&source).unwrap();
    let clean = repository.inventory().unwrap();
    assert!(clean.issues.is_empty());
    assert_eq!(clean.packages.len(), 2);
    assert!(
        clean
            .packages
            .iter()
            .find(|p| p.manifest.id == healthy.id)
            .unwrap()
            .enabled
    );
}

#[test]
fn the_install_limit_keeps_legacy_overflow_recoverable() {
    let directory = root("capacity");
    let data = directory.join("data");
    let repository = Repository::new(&data);
    let source = directory.join("source");
    let mut m = manifest();
    for version in 0..64 {
        m.version = format!("0.1.{version}");
        package(&source, &m);
        repository.install(&source).unwrap();
    }
    m.version = "0.1.64".into();
    package(&source, &m);
    assert!(repository.install(&source).is_err());
    assert_eq!(repository.list().unwrap().len(), 64);

    // Profiles created before the fix can contain a 65th revision.
    let overflow = data
        .join("extensions/installed")
        .join(&m.id)
        .join(&m.version);
    package(&overflow, &m);
    let inventory = repository.inventory().unwrap();
    assert!(inventory.over_limit);
    assert_eq!(inventory.packages.len(), 64);
    repository.remove(&inventory.packages[0]).unwrap();
    assert!(!repository.inventory().unwrap().over_limit);
}

#[test]
fn oversized_sdk_messages_are_rejected_before_deserialization() {
    let directory = root("messages");
    let path = directory.join("request.json");
    let file = fs::File::create(&path).unwrap();
    file.set_len(crabpdf_extension_api::MAX_MESSAGE_BYTES + 1)
        .unwrap();
    assert!(crabpdf_extension_api::read_message::<serde_json::Value>(&path).is_err());
}

fn wait(run: &RunningExtension) -> Result<crabpdf_extension_api::ExtensionOutput, String> {
    let started = Instant::now();
    loop {
        if let Some(result) = run.poll() {
            return result;
        }
        assert!(started.elapsed() < Duration::from_secs(8));
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
#[ignore = "Build crabpdf-extension-api example protocol_fixture and set CRABPDF_TEST_EXTENSION"]
fn actual_native_package_runs_returns_latex_and_cancels_without_blocking_the_host() {
    let fixture = PathBuf::from(std::env::var_os("CRABPDF_TEST_EXTENSION").expect("fixture path"));
    let directory = root("process");
    let repository = Repository::new(&directory.join("data"));
    let source = directory.join("source");
    let mut m = manifest();
    m.executable = fixture.file_name().unwrap().to_string_lossy().into_owned();
    for id in ["wait", "wrong_id", "oversize"] {
        let mut command = m.commands[0].clone();
        command.id = id.into();
        m.commands.push(command);
    }
    fs::create_dir_all(&source).unwrap();
    fs::copy(&fixture, source.join(&m.executable)).unwrap();
    fs::write(source.join("extension.toml"), toml::to_string(&m).unwrap()).unwrap();
    repository.install(&source).unwrap();
    let mut installed = repository.list().unwrap().remove(0);
    repository.enable(&installed, true).unwrap();
    installed.enabled = true;
    for id in ["latex", "wrong_id", "oversize", "wait"] {
        let command = m.commands.iter().find(|c| c.id == id).unwrap().clone();
        let input = Invocation {
            command,
            page: 1,
            region: Region {
                left: 0.1,
                top: 0.1,
                right: 0.5,
                bottom: 0.5,
            },
            text: "x²".into(),
            image: None,
        };
        let running =
            RunningExtension::spawn(repository.clone(), installed.clone(), input, || {}).unwrap();
        let scratch = directory
            .join("data/extensions/data")
            .join(&m.id)
            .join(".crabpdf-scratch");
        if id == "wait" {
            let started = Instant::now();
            while !scratch.join("model.download").exists() {
                assert!(started.elapsed() < Duration::from_secs(8));
                std::thread::sleep(Duration::from_millis(20));
            }
            let other_window = Repository::new(&directory.join("data"));
            assert!(other_window.inventory().unwrap().packages[0].enabled);
            assert!(other_window.enable(&installed, false).is_err());
            running.cancel();
        }
        let result = wait(&running);
        assert!(!scratch.exists(), "Invocation left temporary files: {id}");
        if id == "latex" {
            assert_eq!(result.unwrap().text, "x^2");
        } else {
            assert!(result.is_err(), "{id}");
        }
    }
    repository.enable(&installed, false).unwrap();
    let input = Invocation {
        command: m.commands[0].clone(),
        page: 1,
        region: Region {
            left: 0.1,
            top: 0.1,
            right: 0.5,
            bottom: 0.5,
        },
        text: "x²".into(),
        image: None,
    };
    assert!(
        wait(
            &RunningExtension::spawn(repository.clone(), installed.clone(), input, || {}).unwrap()
        )
        .is_err()
    );
    repository.remove(&installed).unwrap();
}

#[test]
#[ignore = "Requires the optional Wasmtime CLI and a wasm32-wasip1 protocol fixture"]
fn wasi_can_use_exchange_but_cannot_read_a_host_file() {
    use crabpdf_extension_api::ExtensionRuntime;
    let module = std::env::var_os("CRABPDF_TEST_WASI_EXTENSION")
        .map(PathBuf::from)
        .unwrap();
    let root = root("ext-wasi");
    let source = root.join("package");
    std::fs::create_dir_all(&source).unwrap();
    let mut manifest = manifest();
    manifest.runtime = ExtensionRuntime::Wasi;
    manifest.executable = "fixture.wasm".into();
    let command = manifest.commands.first_mut().unwrap();
    command.input = InputKind::Text;
    command.output = OutputKind::Text;
    let command = command.clone();
    std::fs::copy(&module, source.join("fixture.wasm")).unwrap();
    std::fs::write(
        source.join("extension.toml"),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let sentinel = root.join("host-only.txt");
    std::fs::write(&sentinel, "not shared").unwrap();
    let repository = Repository::new(&root.join("profile"));
    repository.install(&source).unwrap();
    let package = repository.list().unwrap().remove(0);
    repository.enable(&package, true).unwrap();
    let package = repository.list().unwrap().remove(0);
    let input = Invocation {
        command,
        page: 1,
        region: Region {
            left: 0.0,
            top: 0.0,
            right: 1.0,
            bottom: 1.0,
        },
        text: sentinel
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        image: None,
    };
    let run = RunningExtension::spawn(repository, package, input, || {}).unwrap();
    // A cold Wasmtime process must compile the module before the first request.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        if let Some(result) = run.poll() {
            assert!(result.unwrap().text.contains("archivo externo inaccesible"));
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "WASI fixture timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
