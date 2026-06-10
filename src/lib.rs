//! OpenArc - Media archiver for phone/camera files
//!
//! This crate provides both a library and CLI binary for archiving media files.

pub mod cli;
pub mod archive_tracker;
pub mod backup_catalog;
pub mod file_tracker;
pub mod hash;
pub mod orchestrator;
pub mod bpg_wrapper;
pub mod image_loader;
pub mod interactive;
pub mod interactive_menu;
pub mod phone_backup;

// Re-export codecs for FFI use
pub use codecs;
