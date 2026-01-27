use std::ffi::{CString, CStr};
use std::os::raw::c_char;
use anyhow::{Result, anyhow};

// External C++ functions from FreeARC libraries
extern "C" {
    // GRZip decompression function
    fn freearc_grzip_decompress(
        input: *const u8,
        input_size: i32,
        output: *mut u8,
        output_size: i32,
    ) -> i32; // Returns actual decompressed size or negative error code

    // GRZip compression function
    fn freearc_grzip_compress(
        input: *const u8,
        input_size: i32,
        output: *mut u8,
        output_size: i32,
        mode: i32,
    ) -> i32; // Returns actual compressed size or negative error code
}

/// Main GRZip decompression function using FFI to FreeARC C++ implementation
pub fn grzip_decompress(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    
    // GRZip block header is 28 bytes, but we let the C++ code handle validation
    if input.len() < 28 {
        return Err(anyhow!("GRZip input too small for header (need 28 bytes, got {})", input.len()));
    }

    // Allocate output buffer
    let mut output = vec![0u8; expected_size];

    let result = unsafe {
        freearc_grzip_decompress(
            input.as_ptr(),
            input.len() as i32,
            output.as_mut_ptr(),
            expected_size as i32,
        )
    };

    if result < 0 {
        return Err(anyhow!("GRZip decompression failed with error code: {}", result));
    }

    let actual_size = result as usize;
    if actual_size <= output.len() {
        output.truncate(actual_size);
    } else {
        return Err(anyhow!("GRZip decompression returned size larger than expected: {} > {}", actual_size, expected_size));
    }

    Ok(output)
}

/// GRZip compression function using FFI to FreeARC C++ implementation
/// 
/// Mode flags (can be combined with |):
/// - 0x0: BWT sorting, WFC encoding, delta filter enabled
/// - 0x1: Disable delta filter
/// - 0x2: ST4 sorting instead of BWT
/// - 0x4: MTF encoding instead of WFC
/// - 0x8: Fast BWT sorting
pub fn grzip_compress(input: &[u8], mode: i32) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    // GRZip max block size is 8MB - 512 bytes
    const GRZ_MAX_BLOCK_SIZE: usize = 8 * 1024 * 1024 - 512;
    if input.len() > GRZ_MAX_BLOCK_SIZE {
        return Err(anyhow!("GRZip input exceeds maximum block size of {} bytes", GRZ_MAX_BLOCK_SIZE));
    }

    // Allocate output buffer - GRZip adds a 28-byte header per block
    // Worst case for incompressible data: input + header + some overhead
    let max_output_size = input.len() + input.len() / 4 + 1024;
    let mut output = vec![0u8; max_output_size];

    let result = unsafe {
        freearc_grzip_compress(
            input.as_ptr(),
            input.len() as i32,
            output.as_mut_ptr(),
            max_output_size as i32,
            mode,
        )
    };

    if result < 0 {
        return Err(anyhow!("GRZip compression failed with error code: {}", result));
    }

    let actual_size = result as usize;
    output.truncate(actual_size);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grzip_decompression() {
        let data = b"GRZip roundtrip test payload: The quick brown fox jumps over the lazy dog.";
        let compressed = grzip_compress(data, 0).unwrap();
        let decompressed = grzip_decompress(&compressed, data.len()).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_grzip_modes_roundtrip() {
        let data = (0u8..=255).collect::<Vec<u8>>();
        let modes = [0, 1, 2, 4, 8];
        for mode in modes {
            let compressed = grzip_compress(&data, mode).unwrap();
            let decompressed = grzip_decompress(&compressed, data.len()).unwrap();
            assert_eq!(data.as_slice(), decompressed.as_slice());
        }
    }
}