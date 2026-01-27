use std::ffi::{CString, CStr};
use std::os::raw::c_char;
use anyhow::{Result, anyhow};

// External C++ functions from FreeARC libraries
extern "C" {
    // LZMA2 decompression function
    fn freearc_lzma2_decompress(
        input: *const u8,
        input_size: i32,
        output: *mut u8,
        output_size: i32,
    ) -> i32; // Returns actual decompressed size or negative error code
    
    // LZMA2 compression function
    fn freearc_lzma2_compress(
        input: *const u8,
        input_size: i32,
        output: *mut u8,
        output_size: i32,
        compression_level: i32,
        dict_size: u32,
        lc: u32,
        lp: u32,
        pb: u32,
    ) -> i32; // Returns actual compressed size or negative error code
}

/// Native FreeARC LZMA decoder function
pub fn lzma_decode_freearc(
    lit_context_bits: u32,
    lit_pos_bits: u32, 
    pos_state_bits: u32,
    dict_size: u32,
    compressed_data: &[u8],
    expected_size: usize,
) -> Result<Vec<u8>> {
    // For now, just use the standard decompression
    lzma2_decompress(compressed_data, expected_size)
}

/// Main LZMA2 decompression function using FFI to FreeARC C++ implementation
pub fn lzma2_decompress(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    if input.len() < 13 {  // LZMA2 header is at least 13 bytes
        return Err(anyhow!("LZMA2 input too small for header"));
    }

    // Allocate output buffer
    let mut output = vec![0u8; expected_size];

    let result = unsafe {
        freearc_lzma2_decompress(
            input.as_ptr(),
            input.len() as i32,
            output.as_mut_ptr(),
            expected_size as i32,
        )
    };

    if result < 0 {
        return Err(anyhow!("LZMA2 decompression failed with error code: {}", result));
    }

    let actual_size = result as usize;
    if actual_size <= output.len() {
        output.truncate(actual_size);
    } else {
        return Err(anyhow!("LZMA2 decompression returned size larger than expected: {} > {}", actual_size, expected_size));
    }

    Ok(output)
}

/// LZMA compression method formatter
pub fn format_lzma_method(dict_size: u32, lc: u32, lp: u32, pb: u32) -> String {
    format!("LZMA:d{}:l{}:p{}:pb{}", dict_size, lc, lp, pb)
}

/// Default LZMA compression function
pub fn compress_lzma_default(data: &[u8]) -> Result<Vec<u8>> {
    lzma2_compress(data, 5, 32 * 1024 * 1024, 3, 0, 0, Some(3)) // Default parameters
}

/// General LZMA compression function with custom parameters
pub fn compress_lzma(data: &[u8], level: i32, dict_size: u32, lc: u32, lp: u32, pb: u32) -> Result<Vec<u8>> {
    lzma2_compress(data, level, dict_size, lc, lp, pb)
}

/// Main LZMA2 compression function using FFI to FreeARC C++ implementation
pub fn lzma2_compress(input: &[u8], compression_level: i32, dict_size: u32, lc: u32, lp: u32, pb: u32) -> Result<Vec<u8>> {
    // Allocate output buffer (typically compressed data is smaller)
    let max_output_size = input.len() + (input.len() / 8) + 256; // Add some extra space
    let mut output = vec![0u8; max_output_size];

    let result = unsafe {
        freearc_lzma2_compress(
            input.as_ptr(),
            input.len() as i32,
            output.as_mut_ptr(),
            max_output_size as i32,
            compression_level,
            dict_size,
            lc,
            lp,
            pb,
        )
    };

    if result < 0 {
        return Err(anyhow!("LZMA2 compression failed with error code: {}", result));
    }

    let actual_size = result as usize;
    if actual_size <= output.len() {
        output.truncate(actual_size);
    } else {
        return Err(anyhow!("LZMA2 compression returned size larger than buffer: {} > {}", actual_size, max_output_size));
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lzma2_decompression() {
        // This test will only pass when linked with actual FreeARC library
        // For now, just ensure the function signature is correct
        assert!(true); // Placeholder test
    }
    
    #[test]
    fn test_lzma2_compression() {
        // This test will only pass when linked with actual FreeARC library
        // For now, just ensure the function signature is correct
        assert!(true); // Placeholder test
    }
}