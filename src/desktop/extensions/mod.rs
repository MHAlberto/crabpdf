//! Independently installed native packages. No package code runs at discovery.
mod packages;
mod process;
pub use packages::{InstalledPackage, Repository, target};
pub use process::{Invocation, RunningExtension};
