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
