pub mod lzma2;
pub mod zstd;
pub mod crc32;
pub mod ppmd;
pub mod dict;
pub mod lzp;
pub mod tornado;
pub mod grzip;
pub mod lz4;

// Re-export commonly used functions
pub use lzma2::*;
pub use zstd::*;
pub use crc32::*;