use anyhow::{anyhow, Result};
use std::io::{Cursor, Read, Write};

use crate::codec::ppmd::{
    h::{PpmdHDecoder, PpmdHEncoder},
    PpmdOptions, PpmdVariant,
};

/// PPMII decoder for FreeArc-compatible PPMd-H streams.
pub struct PPMIIDecoder {
    order: usize,
    memory_size: usize,
}

impl PPMIIDecoder {
    pub fn new<R: std::io::Read>(_reader: R, order: usize, memory_size: usize) -> Result<Self> {
        Ok(PPMIIDecoder { order, memory_size })
    }

    pub fn decode(&mut self, _output: &mut Vec<u8>, _expected_size: usize) -> Result<usize> {
        Err(anyhow!(
            "PPMIIDecoder::decode not yet implemented - use ppmd_decompress instead"
        ))
    }
}

/// Main PPMd-H decompression function.
pub fn ppmd_decompress(
    input: &[u8],
    expected_size: usize,
    order: u8,
    memory_size: usize,
) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let input_cursor = Cursor::new(input);
    let options = PpmdOptions {
        order,
        memory_size,
        variant: PpmdVariant::H,
    };

    let mut decoder = PpmdHDecoder::new(input_cursor, options)
        .map_err(|e| anyhow!("Failed to create PPMd-H decoder: {e}"))?;

    let mut output = vec![0u8; expected_size];
    decoder
        .read_exact(&mut output)
        .map_err(|e| anyhow!("PPMd-H decompression failed: {e}"))?;

    Ok(output)
}

/// PPMd-H compression function.
pub fn ppmd_compress(input: &[u8], order: u8, memory_size: usize) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut output = Vec::new();
    let options = PpmdOptions {
        order,
        memory_size,
        variant: PpmdVariant::H,
    };

    let mut encoder = PpmdHEncoder::new(&mut output, options)
        .map_err(|e| anyhow!("Failed to create PPMd-H encoder: {e}"))?;

    encoder
        .write_all(input)
        .map_err(|e| anyhow!("PPMd-H compression failed: {e}"))?;

    encoder
        .finish(false)
        .map_err(|e| anyhow!("PPMd-H finish failed: {e}"))?;

    Ok(output)
}

#[cfg(all(test, feature = "legacy-ffi-tests"))]
mod tests {
    use super::*;
    use std::os::raw::c_int;

    extern "C" {
        fn freearc_ppmd_decompress(
            input: *mut u8,
            input_size: c_int,
            output: *mut u8,
            output_size: c_int,
            order: c_int,
            memory_size: usize,
        ) -> c_int;

        fn freearc_ppmd_compress(
            input: *mut u8,
            input_size: c_int,
            output: *mut u8,
            output_size: c_int,
            order: c_int,
            memory_size: usize,
        ) -> c_int;
    }

    fn ffi_compress(input: &[u8], order: u8, memory_size: usize) -> Vec<u8> {
        let mut cap = (input.len() + input.len() / 2 + 1024).max(4096);
        for _ in 0..8 {
            let mut src = input.to_vec();
            let mut out = vec![0u8; cap];
            let result = unsafe {
                freearc_ppmd_compress(
                    src.as_mut_ptr(),
                    src.len() as c_int,
                    out.as_mut_ptr(),
                    out.len() as c_int,
                    order as c_int,
                    memory_size,
                )
            };
            if result >= 0 {
                out.truncate(result as usize);
                return out;
            }
            cap *= 2;
        }
        panic!("FreeArc PPMd compression did not fit output buffer");
    }

    fn ffi_decompress(
        input: &[u8],
        expected_size: usize,
        order: u8,
        memory_size: usize,
    ) -> Vec<u8> {
        let mut src = input.to_vec();
        let mut out = vec![0u8; expected_size];
        let result = unsafe {
            freearc_ppmd_decompress(
                src.as_mut_ptr(),
                src.len() as c_int,
                out.as_mut_ptr(),
                out.len() as c_int,
                order as c_int,
                memory_size,
            )
        };
        assert!(result >= 0, "FreeArc PPMd decompression failed: {result}");
        out.truncate(result as usize);
        out
    }

    #[test]
    fn test_ppmd_decompression() {
        let data = b"PPMD roundtrip test payload: Pack my box with five dozen liquor jugs.";
        let order = 6u8;
        let memory_size = 16 * 1024 * 1024;

        let compressed = ppmd_compress(data, order, memory_size).unwrap();
        let decompressed = ppmd_decompress(&compressed, data.len(), order, memory_size).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_ppmd_params_roundtrip() {
        let data = (0u8..=255).collect::<Vec<u8>>();
        let params = [
            (4u8, 4 * 1024 * 1024),
            (6u8, 8 * 1024 * 1024),
            (8u8, 16 * 1024 * 1024),
        ];
        for (order, mem) in params {
            let compressed = ppmd_compress(&data, order, mem).unwrap();
            let decompressed = ppmd_decompress(&compressed, data.len(), order, mem).unwrap();
            assert_eq!(data.as_slice(), decompressed.as_slice());
        }
    }

    #[test]
    #[ignore = "FreeArc C PPMd wrapper currently crashes on this target; kept as a diagnostic compatibility check"]
    fn native_compresses_original_freearc_decompresses() {
        let data = b"FreeArc native PPMd-H compatibility payload ".repeat(512);
        let order = 6u8;
        let memory_size = 16 * 1024 * 1024;

        let compressed = ppmd_compress(&data, order, memory_size).unwrap();
        let decompressed = ffi_decompress(&compressed, data.len(), order, memory_size);

        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    #[ignore = "FreeArc C PPMd wrapper currently crashes on this target; kept as a diagnostic compatibility check"]
    fn original_freearc_compresses_native_decompresses() {
        let data = b"FreeArc original PPMd-H compatibility payload ".repeat(512);
        let order = 6u8;
        let memory_size = 16 * 1024 * 1024;

        let compressed = ffi_compress(&data, order, memory_size);
        let decompressed = ppmd_decompress(&compressed, data.len(), order, memory_size).unwrap();

        assert_eq!(data.as_slice(), decompressed.as_slice());
    }
}
