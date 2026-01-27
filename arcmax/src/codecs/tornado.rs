use std::ffi::{CString, CStr};
use std::os::raw::c_char;
use anyhow::{Result, anyhow};

// External C++ functions from FreeARC libraries
extern "C" {
    // Tornado decompression function
    fn freearc_tornado_decompress(
        input: *const u8,
        input_size: i32,
        output: *mut u8,
        output_size: i32,
    ) -> i32; // Returns actual decompressed size or negative error code

    // Tornado compression function
    fn freearc_tornado_compress(
        input: *const u8,
        input_size: i32,
        output: *mut u8,
        output_size: i32,
        method_number: i32,
    ) -> i32; // Returns actual compressed size or negative error code
}

/// Main Tornado decompression function using FFI to FreeARC C++ implementation
pub fn tornado_decompress(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    if input.len() < 6 {
        return Err(anyhow!("Tornado input too small for header"));
    }

    // Allocate output buffer
    let mut output = vec![0u8; expected_size];

    let result = unsafe {
        freearc_tornado_decompress(
            input.as_ptr(),
            input.len() as i32,
            output.as_mut_ptr(),
            expected_size as i32,
        )
    };

    if result < 0 {
        return Err(anyhow!("Tornado decompression failed with error code: {}", result));
    }

    let actual_size = result as usize;
    if actual_size <= output.len() {
        output.truncate(actual_size);
    } else {
        return Err(anyhow!("Tornado decompression returned size larger than expected: {} > {}", actual_size, expected_size));
    }

    Ok(output)
}

/// Main Tornado compression function using FFI to FreeARC C++ implementation
pub fn tornado_compress(input: &[u8], method_number: i32) -> Result<Vec<u8>> {
    // FreeArc error codes (see Compression/Common.h)
    const FREEARC_ERRCODE_INVALID_COMPRESSOR: i32 = -2;
    const FREEARC_ERRCODE_OUTBLOCK_TOO_SMALL: i32 = -4;

    let mut max_output_size = (input.len() + (input.len() / 8) + 256).max(4096);
    // Cap growth to avoid pathological allocations during tests.
    let max_cap = 64 * 1024 * 1024;
    for _ in 0..16 {
        let mut output = vec![0u8; max_output_size];

        let result = unsafe {
            freearc_tornado_compress(
                input.as_ptr(),
                input.len() as i32,
                output.as_mut_ptr(),
                max_output_size as i32,
                method_number,
            )
        };

        if result >= 0 {
            let actual_size = result as usize;
            if actual_size <= output.len() {
                output.truncate(actual_size);
                return Ok(output);
            }
            return Err(anyhow!(
                "Tornado compression returned size larger than buffer: {} > {}",
                actual_size,
                max_output_size
            ));
        }

        if result == FREEARC_ERRCODE_OUTBLOCK_TOO_SMALL {
            if max_output_size >= max_cap {
                break;
            }
            max_output_size = (max_output_size.saturating_mul(2)).min(max_cap);
            continue;
        }

        if result == FREEARC_ERRCODE_INVALID_COMPRESSOR {
            return Err(anyhow!(
                "Tornado compression failed: invalid method/parameters (method {}, error code: {})",
                method_number,
                result
            ));
        }

        return Err(anyhow!("Tornado compression failed with error code: {}", result));
    }

    Err(anyhow!(
        "Tornado compression failed: output buffer too small (method {}, tried up to {} bytes)",
        method_number,
        max_output_size
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tornado_decompression() {
        let data = b"Tornado roundtrip test payload: 0123456789abcdef0123456789abcdef";
        let mut last_err: Option<anyhow::Error> = None;
        let compressed = (0..=64)
            .find_map(|method| match tornado_compress(data, method) {
                Ok(c) => Some(c),
                Err(e) => {
                    last_err = Some(e);
                    None
                }
            })
            .unwrap_or_else(|| {
                panic!(
                    "tornado_compress failed for methods 0..=64 (last error: {:?})",
                    last_err
                )
            });
        let decompressed = tornado_decompress(&compressed, data.len()).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_tornado_methods_roundtrip() {
        let data = (0u8..=255).collect::<Vec<u8>>();
        // Probe a small method range; different FreeArc builds expose different method numbers.
        let mut ok_count = 0usize;
        for method in 0..=64 {
            if let Ok(compressed) = tornado_compress(&data, method) {
                let decompressed = tornado_decompress(&compressed, data.len()).unwrap();
                assert_eq!(data.as_slice(), decompressed.as_slice());
                ok_count += 1;
            }
        }
        assert!(ok_count > 0, "no Tornado methods in 0..=64 succeeded");
    }
}