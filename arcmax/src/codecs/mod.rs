//! Codec modules - C++ FFI implementations from FreeARC

pub mod lzma2;
pub mod tornado;
pub mod ppmd;
pub mod lzp;
pub mod grzip;
pub mod zstd;

// Re-export commonly used functions
pub use lzma2::{lzma2_compress, lzma2_decompress};
pub use tornado::{tornado_compress, tornado_decompress};
pub use ppmd::{ppmd_compress, ppmd_decompress};
pub use lzp::{lzp_compress, lzp_decompress};
pub use grzip::{grzip_compress, grzip_decompress};
pub use zstd::{compress_zstd, decompress_zstd, format_zstd_method};
