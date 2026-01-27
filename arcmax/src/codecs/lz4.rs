use std::io::Read;
use anyhow::{Result, anyhow};

/// LZ4 decompression algorithm implementation
///
/// LZ4 is a fast compression algorithm that uses LZ77-based compression
pub fn lz4_decompress(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    // Use the lz4 crate for LZ4 decompression
    // Provide the expected size as the uncompressed size hint
    let result = lz4::block::decompress(input, Some(expected_size as i32))
        .map_err(|e| anyhow!("LZ4 decompression failed: {}", e))?;

    // Resize to expected size if needed
    let mut result = result;
    if result.len() < expected_size {
        result.resize(expected_size, 0);
    } else if result.len() > expected_size {
        result.truncate(expected_size);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz4_decompression() {
        // Test with some dummy data
        let original = b"Hello, LZ4! This is a test string for LZ4 decompression.";
        let compressed = lz4::block::compress(original, None).expect("LZ4 compression failed");

        let decompressed = lz4_decompress(&compressed, original.len()).unwrap();
        assert_eq!(original, decompressed.as_slice());
    }
}