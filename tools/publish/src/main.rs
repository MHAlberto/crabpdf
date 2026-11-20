//! Publish binaries directly from the completed, verified Actions runs.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use zip::ZipArchive;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const REPO: &str = "MHAlberto/crabpdf";
const MAX_FILE_BYTES: u64 = 384 * 1024 * 1024;

fn require(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

fn output(program: &str, arguments: &[&str]) -> Result<String> {
    let result = Command::new(program).args(arguments).output()?;
    require(
        result.status.success(),
        format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ),
    )?;
    Ok(String::from_utf8(result.stdout)?)
}

fn api(suffix: &str) -> Result<Value> {
    let body = output("gh", &["api", &format!("repos/{REPO}/{suffix}")])?;
    Ok(serde_json::from_str(&body)?)
}

fn sha256(path: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    io::copy(&mut File::open(path)?, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn read_json(path: &Path) -> Result<Value> {
    require(
        fs::metadata(path)?.len() <= 1024 * 1024,
        "Oversized validation report",
    )?;
    Ok(serde_json::from_reader(File::open(path)?)?)
}

fn archived(run: &str, sha: &str, name: &str, path: &Path, files: &[String]) -> Result<()> {
    let history = api(&format!("actions/runs/{run}"))?;
    require(
        history["head_sha"] == sha && history["conclusion"] == "success",
        format!("Run {run} has no successful proof for {sha}"),
    )?;
    let artifacts = api(&format!("actions/runs/{run}/artifacts?per_page=100"))?;
    let artifact = artifacts["artifacts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|artifact| artifact["name"] == name && artifact["expired"] == false)
        .ok_or_else(|| format!("Missing artifact {name}"))?;
    let id = artifact["id"].as_u64().ok_or("Missing artifact ID")?;
    let bytes = artifact["size_in_bytes"]
        .as_u64()
        .ok_or("Missing artifact size")?;
    require(
        bytes > 0 && bytes <= MAX_FILE_BYTES,
        "Invalid artifact size",
    )?;
    let archive = path.with_extension("zip");
    let status = Command::new("gh")
        .args(["api", &format!("repos/{REPO}/actions/artifacts/{id}/zip")])
        .stdin(Stdio::null())
        .stdout(File::create(&archive)?)
        .status()?;
    require(
        status.success(),
        format!("Could not download artifact {name}"),
    )?;
    require(
        fs::metadata(&archive)?.len() == bytes
            && artifact["digest"] == format!("sha256:{}", sha256(&archive)?),
        format!("Artifact digest mismatch for {name}"),
    )?;

    fs::create_dir_all(path)?;
    let mut zip = ZipArchive::new(File::open(&archive)?)?;
    let mut extracted = BTreeMap::new();
    for index in 0..zip.len() {
        let mut item = zip.by_index(index)?;
        let Some(local) = item.enclosed_name() else {
            return Err("Unsafe path in Actions archive".into());
        };
        if !item.is_file() {
            continue;
        }
        let Some(file) = local.file_name().and_then(|part| part.to_str()) else {
            continue;
        };
        if files.iter().any(|name| name == file) {
            require(
                item.size() > 0
                    && item.size() <= MAX_FILE_BYTES
                    && extracted.insert(file.to_owned(), ()).is_none(),
                format!("Invalid or duplicate file {file}"),
            )?;
            let size = item.size();
            let copied = io::copy(&mut item, &mut File::create(path.join(file))?)?;
            require(copied == size, format!("Incomplete archive entry {file}"))?;
        }
    }
    require(
        extracted.len() == files.len(),
        format!("Missing files from {name}"),
    )?;
    println!("Verified workflow artifact {name} ({id}).");
    Ok(())
}

fn win(root: &Path, version: &str, sha: &str, run: &str, arch: &str) -> Result<Vec<PathBuf>> {
    let folder = root.join(format!("win-{arch}"));
    let stem = format!("crabpdf-{version}-win-{arch}");
    let portable = format!("{stem}.zip");
    let installer = format!("{stem}-setup.exe");
    let manifest = format!("{installer}.json");
    let files = [portable, installer, manifest];
    let mut needed = files.to_vec();
    needed.push("validation.json".into());
    archived(
        run,
        sha,
        &format!("crabpdf-windows-{arch}"),
        &folder,
        &needed,
    )?;
    let build = read_json(&folder.join(&files[2]))?;
    let evidence = read_json(&folder.join("validation.json"))?;
    require(
        build["version"] == version
            && build["architecture"] == arch
            && build["source_commit"] == sha
            && build["source_dirty"] == false
            && evidence["architecture"] == arch
            && evidence["installer_sha256"] == sha256(&folder.join(&files[1]))?
            && [
                "installation",
                "upgrade",
                "uninstall",
                "manifest_verified",
                "recovers_interrupted_swap",
                "survives_staging_failure",
            ]
            .iter()
            .all(|key| evidence[*key] == true),
        format!("Windows {arch} installer proof failed"),
    )?;
    Ok(files.iter().map(|file| folder.join(file)).collect())
}

fn unix(root: &Path, version: &str, sha: &str, run: &str, platform: &str) -> Result<Vec<PathBuf>> {
    let folder = root.join(platform);
    let stem = format!("crabpdf-{version}-{platform}");
    let (files, report, check) = if platform == "linux-x64" {
        (
            vec![
                format!("{stem}.tar.gz"),
                format!("crabpdf-{version}-ubuntu24.04-amd64.deb"),
            ],
            format!("crabpdf-{version}-linux-validation.json"),
            "package_hashes_verified",
        )
    } else {
        (
            vec![
                format!("{stem}.dmg"),
                format!("{stem}.zip"),
                format!("{stem}-system-tool-sources.zip"),
            ],
            format!("{stem}-validation.json"),
            "dmg_mounted_and_hashes_verified",
        )
    };
    let mut needed = files.clone();
    needed.push(report.clone());
    archived(run, sha, &format!("crabpdf-{platform}"), &folder, &needed)?;
    let evidence = read_json(&folder.join(report))?;
    require(
        evidence["version"] == version
            && evidence["build_commit"] == sha
            && evidence[check] == true
            && evidence["native_gui_formats_passed"]
                .as_array()
                .is_some_and(|items| items.len() == 4),
        format!("Package proof failed for {platform}"),
    )?;
    let assets = evidence["assets"]
        .as_array()
        .ok_or("Missing package hashes")?;
    require(assets.len() == files.len(), "Unexpected package count")?;
    for name in &files {
        let asset = assets
            .iter()
            .find(|asset| asset["name"] == *name)
            .ok_or("Missing package hash")?;
        let path = folder.join(name);
        require(
            asset["bytes"] == fs::metadata(&path)?.len() && asset["sha256"] == sha256(&path)?,
            format!("Package hash mismatch for {name}"),
        )?;
    }
    if let Some(arch) = platform.strip_prefix("mac-") {
        require(
            evidence["architecture"] == arch,
            "Unexpected Mac architecture",
        )?;
        println!(
            "Verified macOS {arch}, minimum {}.",
            evidence["minimum_macos"]
        );
    }
    Ok(files.iter().map(|file| folder.join(file)).collect())
}

fn verify_assets(release: &Value, expected: &BTreeMap<String, (String, u64)>) -> Result<()> {
    let assets = release["assets"]
        .as_array()
        .ok_or("Missing release assets")?;
    require(assets.len() == expected.len(), "Unexpected release assets")?;
    for asset in assets {
        let name = asset["name"].as_str().ok_or("Missing asset name")?;
        let (hash, bytes) = expected
            .get(name)
            .ok_or_else(|| format!("Unexpected asset {name}"))?;
        require(
            asset["digest"] == format!("sha256:{hash}")
                && asset["size"] == *bytes
                && asset["state"] == "uploaded",
            format!("Release digest mismatch for {name}"),
        )?;
    }
    Ok(())
}

fn number(variable: &str) -> Result<String> {
    let value = env::var(variable)?;
    require(
        value.parse::<u64>().is_ok_and(|id| id > 0),
        format!("Invalid {variable}"),
    )?;
    Ok(value)
}

fn main() -> Result<()> {
    require(
        env::var("GITHUB_REPOSITORY")? == REPO,
        "Unexpected repository",
    )?;
    let version = env::var("RELEASE_VERSION")?;
    let parts: Vec<_> = version.split('.').collect();
    require(
        parts.len() == 3
            && parts.iter().all(|part| {
                part.parse::<u32>().is_ok() && part.bytes().all(|byte| byte.is_ascii_digit())
            }),
        "Invalid release version",
    )?;
    let draft = number("RELEASE_DRAFT")?;
    let ci = number("VERIFIED_CI")?;
    let packages = number("VERIFIED_PACKAGES")?;
    let source_hash = env::var("FORMAT_SOURCES_SHA256")?;
    require(
        source_hash.len() == 64 && source_hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid sources digest",
    )?;
    let tag = format!("v{version}");
    let sha = output("git", &["rev-list", "-n", "1", &tag])?
        .trim()
        .to_owned();
    require(
        Command::new("git")
            .args([
                "diff",
                "--quiet",
                &sha,
                "HEAD",
                "--",
                "Cargo.toml",
                "Cargo.lock",
                "rust-toolchain.toml",
                "build.rs",
                "src",
                "crates",
                "assets",
                "installer",
                "scripts",
                "tests",
                "examples",
            ])
            .status()?
            .success(),
        "Application and packaging inputs differ from the release tag",
    )?;
    let endpoint = format!("releases/{draft}");
    let release = api(&endpoint)?;
    require(
        release["tag_name"] == tag && release["draft"] == true,
        "Unexpected release draft",
    )?;
    let sources = format!("crabpdf-{version}-format-tools-sources.zip");
    let existing = release["assets"].as_array().ok_or("Missing draft assets")?;
    let source = existing
        .iter()
        .find(|asset| asset["name"] == sources)
        .ok_or("Missing sources archive")?;
    let source_bytes = source["size"].as_u64().ok_or("Missing sources size")?;
    require(
        source_bytes > 0
            && source["digest"] == format!("sha256:{source_hash}")
            && source["state"] == "uploaded",
        "Sources archive does not match its verified digest",
    )?;

    let root = PathBuf::from("artifacts/verified-release");
    fs::create_dir_all(&root)?;
    let mut assets = Vec::new();
    for arch in ["x64", "x86"] {
        assets.extend(win(&root, &version, &sha, &ci, arch)?);
    }
    for platform in ["linux-x64", "mac-x64", "mac-arm64"] {
        assets.extend(unix(&root, &version, &sha, &packages, platform)?);
    }
    let mut expected = BTreeMap::new();
    expected.insert(sources, (source_hash, source_bytes));
    for path in &assets {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid release file")?;
        expected.insert(name.to_owned(), (sha256(path)?, fs::metadata(path)?.len()));
    }
    let sums = root.join(format!("SHA256SUMS-{tag}.txt"));
    let text = expected
        .iter()
        .map(|(name, (hash, _))| format!("{hash}  {name}\n"))
        .collect::<String>();
    fs::write(&sums, text)?;
    let name = sums
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid checksums file")?;
    expected.insert(
        name.to_owned(),
        (sha256(&sums)?, fs::metadata(&sums)?.len()),
    );
    assets.push(sums);

    for path in assets {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid release asset")?;
        if let Some(asset) = existing.iter().find(|asset| asset["name"] == name) {
            let (hash, bytes) = &expected[name];
            require(
                asset["digest"] == format!("sha256:{hash}")
                    && asset["size"] == *bytes
                    && asset["state"] == "uploaded",
                format!("Conflicting draft asset {name}"),
            )?;
        } else {
            let path = path.to_str().ok_or("Non-UTF-8 asset path")?;
            output("gh", &["release", "upload", &tag, path, "--repo", REPO])?;
        }
        println!("Verified release asset {name}.");
    }
    verify_assets(&api(&endpoint)?, &expected)?;
    let public: Value = serde_json::from_str(&output(
        "gh",
        &[
            "api",
            "--method",
            "PATCH",
            &format!("repos/{REPO}/{endpoint}"),
            "-F",
            "draft=false",
            "-F",
            "prerelease=true",
        ],
    )?)?;
    require(
        public["draft"] == false && public["tag_name"] == tag,
        "Release did not publish",
    )?;
    println!("Published {}: {}", tag, public["html_url"]);
    Ok(())
}
