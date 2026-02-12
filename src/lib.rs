//! OpenArc - Media archiver for phone/camera files
//!
//! This crate provides both a library and CLI binary for archiving media files.

pub mod cli;
pub mod archive_tracker;
pub mod backup_catalog;
pub mod hash;
pub mod orchestrator;
pub mod bpg_wrapper;
pub mod image_loader;

// Re-export zstd types from arcmax for FFI use
pub use arcmax::codecs::zstd::{ZstdCodec, ZstdOptions};

// Re-export codecs for FFI use
pub use codecs;
