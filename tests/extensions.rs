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
        api_version: 1,
        id: "org.crabpdf.test".into(),
        name: "Prueba".into(),
        version: "0.1.0".into(),
        host_version: ">=0.5.0, <0.6.0".into(),
        target: target(),
        executable: "tool".into(),
        description: "Fixture".into(),
        timer: None,
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
        if id == "wait" {
            std::thread::sleep(Duration::from_millis(150));
            running.cancel();
        }
        let result = wait(&running);
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
