//! API 1 for local extensions. Each invocation exchanges bounded JSON files.
//! The host supplies selected data only after a user invokes a package command.
pub mod catalog;

use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
};

pub const API_VERSION: u32 = 1;
pub const MAX_MESSAGE_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub api_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub host_version: String,
    pub target: String,
    pub executable: String,
    #[serde(default, skip_serializing_if = "ExtensionRuntime::is_native")]
    pub runtime: ExtensionRuntime,
    pub description: String,
    pub commands: Vec<ExtensionCommand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<TimerSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<HostPanel>,
}

/// A panel contributed by a package and rendered by the reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostPanel {
    ReadingStatistics,
    ReadingGuide,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionRuntime {
    #[default]
    Native,
    Wasi,
}

impl ExtensionRuntime {
    fn is_native(&self) -> bool {
        *self == Self::Native
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimerSpec {
    pub focus_minutes: u32,
    pub break_minutes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionCommand {
    pub id: String,
    pub title: String,
    pub input: InputKind,
    pub output: OutputKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Text,
    Region,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Text,
    Latex,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
impl Region {
    pub fn valid(self) -> bool {
        [self.left, self.top, self.right, self.bottom]
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && self.left < self.right
            && self.top < self.bottom
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionImage {
    pub png_path: PathBuf,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api_version: u32,
    pub request_id: u64,
    pub command: String,
    /// Page number is one-based.
    pub page: usize,
    /// Normalized, unrotated page coordinates, origin at top left.
    pub region: Region,
    pub text: Option<String>,
    pub image: Option<SelectionImage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionOutput {
    pub kind: OutputKind,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub api_version: u32,
    pub request_id: u64,
    pub result: Result<ExtensionOutput, String>,
}

pub fn read_message<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> io::Result<T> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_MESSAGE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MESSAGE_BYTES {
        return Err(io::Error::other("Message exceeds API limit"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

pub fn write_message<T: Serialize>(path: &std::path::Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    if bytes.len() as u64 > MAX_MESSAGE_BYTES {
        return Err(io::Error::other("Message exceeds API limit"));
    }
    let mut file = File::create_new(path)?;
    file.write_all(&bytes)?;
    file.sync_all()
}

/// A Rust package's main entry point. Put resources beside the executable.
/// The working directory is the immutable installed package.
pub fn serve(handler: impl FnOnce(Request) -> Result<ExtensionOutput, String>) -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 || args[0] != "--crabpdf-request" || args[2] != "--crabpdf-response" {
        return Err(io::Error::other(
            "Expected CrabPDF request and response paths",
        ));
    }
    let request: Request = read_message(std::path::Path::new(&args[1]))?;
    let response = Response {
        api_version: API_VERSION,
        request_id: request.request_id,
        result: if request.api_version != API_VERSION
            || !request.region.valid()
            || request.page == 0
        {
            Err("Incompatible or invalid request".into())
        } else {
            handler(request)
        },
    };
    write_message(std::path::Path::new(&args[3]), &response)
}
