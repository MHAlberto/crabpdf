//! CrabPDF desktop composition and stable public facade.
//! The implementations live in the three dependency-ordered workspace libraries.
pub use crabpdf_application as application;
pub use crabpdf_application::{commands, state};
pub use crabpdf_core::{document, layout, reading_position};
pub use crabpdf_runtime::formats::djvu_text;
pub use crabpdf_runtime::storage::profile as config;
pub use crabpdf_runtime::storage::{annotations, fileops};
pub use crabpdf_runtime::{cache, formats, worker};
pub use crabpdf_runtime::{covers as library, paths as app_paths, pdfium as engine};

pub mod branding;
pub mod cli;
pub mod desktop;
pub mod ui;

/// Existing embedding API. The implementation belongs to desktop and runtime.
pub mod platform {
    pub use crate::desktop::{dialogs::pdf_dialog, shell::*};
    pub use crabpdf_runtime::paths::config_path;
    pub use crabpdf_runtime::pdfium::discovery::{library_candidates, library_name};
}
