//! FreeARC Library - Simplified API for compression/decompression
//!
//! This library provides a clean, simple interface to FreeARC compression
//! algorithms using the successfully built C++ library.

use anyhow::{Result, anyhow};

pub mod codecs;
pub mod core;
pub mod formats;

// External C++ functions from FreeARC libraries
// Note: Actual FFI definitions are in the respective codec modules (e.g., codecs/lzma2.rs)

/// Main LZMA2 decompression function using FFI to FreeARC C++ implementation
pub fn lzma2_decompress(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    // Use default parameters for simple decompression interface
    // Defaults: dict_size=32MB, lc=3, lp=0, pb=0 (must match compression params)
    codecs::lzma2::lzma2_decompress(input, expected_size, 32 * 1024 * 1024, 3, 0, 0)
}

/// Main LZMA2 compression function using FFI to FreeARC C++ implementation
pub fn lzma2_compress(input: &[u8], compression_level: i32, dict_size: u32, lc: u32, lp: u32, pb: u32) -> Result<Vec<u8>> {
    codecs::lzma2::lzma2_compress(input, compression_level, dict_size, lc, lp, pb)
}

/// Compression methods available
#[derive(Debug, Clone, Copy)]
pub enum CompressionMethod {
    /// No compression (store)
    Store,
    /// LZMA2 compression
    Lzma2 { level: i32, dict_size: u32 },
}

impl Default for CompressionMethod {
    fn default() -> Self {
        Self::Lzma2 { level: 5, dict_size: 32 * 1024 * 1024 }
    }
}

/// Compress data using specified method
pub fn compress(data: &[u8], method: CompressionMethod) -> Result<Vec<u8>> {
    match method {
        CompressionMethod::Store => Ok(data.to_vec()),
        CompressionMethod::Lzma2 { level, dict_size } => {
            lzma2_compress(data, level, dict_size, 3, 0, 0)
        }
    }
}

/// Decompress data (automatically detects method)
pub fn decompress(compressed_data: &[u8]) -> Result<Vec<u8>> {
    // Try LZMA2 first
    lzma2_decompress(compressed_data, compressed_data.len() * 4)
}

/// Get compression ratio
pub fn compression_ratio(original: usize, compressed: usize) -> f64 {
    if original == 0 {
        return 0.0;
    }
    (compressed as f64) / (original as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_compression() {
        let data = b"Hello, World! This is a test string for compression.";
        
        // Test compression
        let compressed = compress(data, CompressionMethod::default()).unwrap();
        println!("Original: {} bytes", data.len());
        println!("Compressed: {} bytes", compressed.len());
        println!("Ratio: {:.2}%", compression_ratio(data.len(), compressed.len()) * 100.0);
        
        // Test decompression
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(data, decompressed.as_slice());
        println!("Round-trip successful!");
    }
}
