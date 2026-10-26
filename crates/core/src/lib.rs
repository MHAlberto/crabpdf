//! Shared values and geometry. No UI, engine, filesystem or process dependencies.
pub mod document;
pub mod layout;
pub mod reading_position;

/// Identity shared by the desktop and document exporters.
pub const APPLICATION_NAME: &str = "CrabPDF";
