//! Static distribution catalog. Its schema is independent of the execution protocol.
use serde::{Deserialize, Serialize};

pub const CATALOG_VERSION: u32 = 1;
pub const MAX_CATALOG_BYTES: u64 = 256 * 1024;
pub const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;
pub const CATALOG_URL: &str = "https://mhalberto.github.io/crabpdf/#extensiones";
pub const CATALOG_ENDPOINT: &str = "https://mhalberto.github.io/crabpdf/extensions/catalog-v1.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Catalog {
    pub schema: u32,
    pub extensions: Vec<CatalogEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogEntry {
    pub code: String,
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub api_version: u32,
    pub host_version: String,
    pub assets: Vec<CatalogAsset>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogAsset {
    pub target: String,
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    /// If present, the asset is hosted on this tag in CrabPDF's GitHub Releases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_tag: Option<String>,
}

pub fn valid_code(code: &str) -> bool {
    code.len() == 6 && code.bytes().all(|byte| byte.is_ascii_digit()) && !code.starts_with('0')
}
