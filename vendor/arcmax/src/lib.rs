//! FreeArc Rust port.
//!
//! The public API is now organized around typed methods, codec traits, and
//! archive/pipeline modules. The old wrapper-style compression functions are no
//! longer exported; temporary C/C++ bridges remain private until their native
//! replacements are ported.

pub mod api;
pub mod archive;
pub mod auto;
pub mod codec;
pub(crate) mod core;
pub mod crypto;
pub mod error;
pub mod filetype;
pub(crate) mod formats;
pub(crate) mod io;
pub mod method;
pub mod srep;
pub mod tar_zst;
pub(crate) mod util;

#[cfg(feature = "ffi-codecs")]
mod codecs;

pub use api::{
    compress, compress_stream, compress_with, decompress, decompress_stream, decompress_with,
    CompressionOptions, DecompressionOptions,
};
pub use error::{ArcError, Result};
pub use method::{CodecPipeline, Method};
