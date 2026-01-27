//! Core functionality for OpenArc

pub mod filetype;
pub mod archive;
pub mod backup_catalog;
pub mod hash;
pub mod orchestrator;

// Re-exports
pub use filetype::{FileType, detect_file_type};
