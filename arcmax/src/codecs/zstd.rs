//! Zstandard compression/decompression for arcmax
//!
//! Provides Zstd compression suitable for FreeARC archives.

use std::io::{Read, Write};

/// Compress data using Zstandard
pub fn compress_zstd(data: &[u8], level: i32) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), level)?;
    encoder.write_all(data)?;
    encoder.finish()
}

/// Compress data using Zstandard with default compression level (3)
pub fn compress_zstd_default(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    compress_zstd(data, 3)
}

/// Decompress Zstandard data
pub fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = zstd::stream::Decoder::new(data)?;
    let mut output = Vec::new();
    decoder.read_to_end(&mut output)?;
    Ok(output)
}

/// Decompress Zstandard data with a maximum output size
pub fn decompress_zstd_with_limit(data: &[u8], max_size: usize) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = zstd::stream::Decoder::new(data)?;
    let mut output = vec![0u8; max_size];
    let bytes_read = decoder.read(&mut output)?;
    output.truncate(bytes_read);
    Ok(output)
}

/// Format Zstd parameters as a FreeARC-style method string
pub fn format_zstd_method(level: i32) -> String {
    format!("zstd:{}", level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zstd_roundtrip() {
        let original = b"Hello, World! This is a test of Zstandard compression.";
        let compressed = compress_zstd(original, 3).unwrap();
        let decompressed = decompress_zstd(&compressed).unwrap();
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_zstd_levels() {
        let data = b"Testing different compression levels";
        for level in 1..=19 {
            let compressed = compress_zstd(data, level).unwrap();
            let decompressed = decompress_zstd(&compressed).unwrap();
            assert_eq!(data.as_slice(), decompressed.as_slice());
        }
    }
}
