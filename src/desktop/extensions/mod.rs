//! Independently installed native packages. No package code runs at discovery.
pub mod catalog;
mod packages;
mod process;
pub use packages::{InstalledPackage, Inventory, Repository, target};
pub use process::{Invocation, RunningExtension};
