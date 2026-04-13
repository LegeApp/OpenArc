use std::io::{Read, Write};
use std::os::raw::c_char;
use anyhow::{Result, anyhow};

// External C++ functions from FreeARC libraries
extern "C" {
    // LZP decompression function
    fn freearc_lzp_decompress(
        input: *const u8,
        input_size: i32,
        output: *mut u8,
        output_size: i32,
        min_match_len: i32,
        hash_size: i32,
    ) -> i32; // Returns actual decompressed size or negative error code

    // LZP compression function
    fn freearc_lzp_compress(
        input: *const u8,
        input_size: i32,
        output: *mut u8,
        output_size: i32,
        min_match_len: i32,
        hash_size: i32,
    ) -> i32; // Returns actual compressed size or negative error code
}

/// LZP (Lempel-Ziv-Pascal) post-processing for FreeARC archives
///
/// LZP is a fast compression algorithm that works as a preprocessor
/// to improve the efficiency of other compression methods.
/// This module implements the LZP post-processing algorithms.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LzpMethod {
    /// No LZP processing
    None,
    /// LZP with default parameters
    Lzp,
    /// LZP with specific hash size (in KB)
    LzpHash(u32),
}

impl LzpMethod {
    /// Parse LZP method from FreeARC-style string
    pub fn from_string(method: &str) -> Option<Self> {
        let method_lower = method.to_lowercase();
        
        match method_lower.as_str() {
            "none" | "" => Some(LzpMethod::None),
            "lzp" => Some(LzpMethod::Lzp),
            s if s.starts_with("lzp:") => {
                // Handle complex parameters like "lzp:64m:24:h20"
                // Extract the first numeric parameter (hash size)
                let param_part = s.strip_prefix("lzp:").unwrap_or("");
                let first_param = param_part.split(':').next().unwrap_or(param_part);

                // Handle units like "64m" (64 megabytes)
                if first_param.ends_with('m') || first_param.ends_with('k') || first_param.ends_with('g') {
                    let num_part = &first_param[..first_param.len()-1];
                    if let Ok(num) = num_part.parse::<u32>() {
                        match first_param.chars().last().unwrap_or(' ') {
                            'k' => Some(LzpMethod::LzpHash(num)),      // Already in KB
                            'm' => Some(LzpMethod::LzpHash(num * 1024)), // Convert MB to KB
                            'g' => Some(LzpMethod::LzpHash(num * 1024 * 1024)), // Convert GB to KB
                            _ => Some(LzpMethod::LzpHash(num)),
                        }
                    } else {
                        None
                    }
                } else {
                    // Pure numeric value
                    s.strip_prefix("lzp:")
                        .and_then(|param| param.parse::<u32>().ok())
                        .map(|hash_size| LzpMethod::LzpHash(hash_size))
                }
            },
            _ => None,
        }
    }
    
    /// Get the method name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            LzpMethod::None => "none",
            LzpMethod::Lzp => "lzp",
            LzpMethod::LzpHash(_) => "lzp",
        }
    }
}

/// LZP post-processor
pub struct LzpProcessor {
    method: LzpMethod,
}

impl LzpProcessor {
    pub fn new(method: LzpMethod) -> Self {
        LzpProcessor { method }
    }
    
    /// Apply LZP post-processing to decompressed data
    pub fn post_process(&self, data: &mut Vec<u8>, _original_size: Option<usize>) -> Result<()> {
        match self.method {
            LzpMethod::None => {
                // No processing needed
                Ok(())
            },
            LzpMethod::Lzp => {
                // Use default hash size of 64KB
                self.apply_lzp_reverse_ffi(data, 64 * 1024)
            },
            LzpMethod::LzpHash(hash_size_kb) => {
                self.apply_lzp_reverse_ffi(data, hash_size_kb * 1024)
            },
        }
    }

    /// Reverse LZP transformation using FFI to FreeARC C++ implementation
    fn apply_lzp_reverse_ffi(&self, data: &mut Vec<u8>, hash_size: u32) -> Result<()> {
        // Use the FFI function to call the FreeARC C++ LZP implementation
        let min_match_len = 32; // Default min match length for LZP

        let result = unsafe {
            freearc_lzp_decompress(
                data.as_ptr(),
                data.len() as i32,
                data.as_mut_ptr(),
                data.capacity() as i32,
                min_match_len as i32,
                hash_size as i32,
            )
        };

        if result < 0 {
            return Err(anyhow!("LZP decompression failed with error code: {}", result));
        }

        let actual_size = result as usize;
        if actual_size <= data.len() {
            data.truncate(actual_size);
        } else {
            return Err(anyhow!("LZP decompression returned size larger than expected: {} > {}", actual_size, data.len()));
        }

        Ok(())
    }
}

/// Convenience function to apply LZP post-processing
pub fn apply_lzp_post_processing(data: &mut Vec<u8>, method: &str, original_size: Option<usize>) -> Result<()> {
    if let Some(lzp_method) = LzpMethod::from_string(method) {
        let processor = LzpProcessor::new(lzp_method);
        processor.post_process(data, original_size)
    } else {
        Ok(())
    }
}

/// Main LZP decompression function using FFI to FreeARC C++ implementation
pub fn lzp_decompress(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut output = vec![0u8; expected_size];
    let min_match_len = 32; // Default min match length
    let hash_size = 18; // Default hash size log (2^18 = 256KB hash table)

    let result = unsafe {
        freearc_lzp_decompress(
            input.as_ptr(),
            input.len() as i32,
            output.as_mut_ptr(),
            expected_size as i32,
            min_match_len,
            hash_size,
        )
    };

    if result < 0 {
        return Err(anyhow!("LZP decompression failed with error code: {}", result));
    }

    let actual_size = result as usize;
    if actual_size <= output.len() {
        output.truncate(actual_size);
    } else {
        return Err(anyhow!("LZP decompression returned size larger than expected: {} > {}", actual_size, expected_size));
    }

    Ok(output)
}

/// LZP compression function using FFI to FreeARC C++ implementation
pub fn lzp_compress(input: &[u8], min_match_len: i32, hash_size_log: i32) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    // Allocate output buffer (worst case: slightly larger than input)
    let max_output_size = input.len() + input.len() / 8 + 1024;
    let mut output = vec![0u8; max_output_size];

    let result = unsafe {
        freearc_lzp_compress(
            input.as_ptr(),
            input.len() as i32,
            output.as_mut_ptr(),
            max_output_size as i32,
            min_match_len,
            hash_size_log,
        )
    };

    if result < 0 {
        return Err(anyhow!("LZP compression failed with error code: {}", result));
    }

    let actual_size = result as usize;
    output.truncate(actual_size);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lzp_method_parsing() {
        assert_eq!(LzpMethod::from_string("none"), Some(LzpMethod::None));
        assert_eq!(LzpMethod::from_string(""), Some(LzpMethod::None));
        assert_eq!(LzpMethod::from_string("lzp"), Some(LzpMethod::Lzp));
        assert_eq!(LzpMethod::from_string("lzp:64"), Some(LzpMethod::LzpHash(64)));
        assert_eq!(LzpMethod::from_string("invalid"), None);
    }

    #[test]
    fn test_lzp_basic() {
        let data = b"LZP roundtrip test payload: 0123456789abcdef0123456789abcdef";
        let compressed = lzp_compress(data, 32, 18).unwrap();
        let decompressed = lzp_decompress(&compressed, data.len()).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }
}