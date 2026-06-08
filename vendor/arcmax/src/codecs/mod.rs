//! Codec modules - C++ FFI implementations from FreeARC

pub mod grzip;
pub mod lzma2;
pub mod lzp;
pub mod ppmd;
pub mod tornado;

// Re-export commonly used functions
pub use grzip::{grzip_compress, grzip_decompress};
pub use lzma2::{lzma2_compress, lzma2_decompress};
pub use lzp::{lzp_compress, lzp_decompress};
pub use ppmd::{ppmd_compress, ppmd_decompress};
pub use tornado::{tornado_compress, tornado_decompress};
