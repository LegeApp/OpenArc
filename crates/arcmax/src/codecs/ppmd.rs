use anyhow::{Result, anyhow};
use std::io::{Read, Write, Cursor};
use ppmd_rust::{Ppmd7Encoder, Ppmd7Decoder};

/// PPMII decoder for FreeARC compatibility
/// Note: This now uses PPMd7 (PPMdH) from ppmd-rust crate instead of FreeARC's 32-bit PPMD
pub struct PPMIIDecoder {
    order: usize,
    memory_size: usize,
}

impl PPMIIDecoder {
    pub fn new<R: std::io::Read>(mut reader: R, order: usize, memory_size: usize) -> Result<Self> {
        Ok(PPMIIDecoder {
            order,
            memory_size,
        })
    }
    
    pub fn decode(&mut self, output: &mut Vec<u8>, expected_size: usize) -> Result<usize> {
        Err(anyhow!("PPMIIDecoder::decode not yet implemented - use ppmd_decompress instead"))
    }
}

/// Main PPMD decompression function using ppmd-rust crate (PPMd7/PPMdH variant)
/// This is 64-bit compatible unlike the FreeARC PPMD implementation
pub fn ppmd_decompress(input: &[u8], expected_size: usize, order: u8, memory_size: usize) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    // Create a cursor for the input data
    let input_cursor = Cursor::new(input);
    
    // Create a decoder with the input reader and specified parameters
    // PPMd7Decoder::new(reader, order, mem_size)
    let mut decoder = Ppmd7Decoder::new(input_cursor, order as u32, memory_size as u32)
        .map_err(|e| anyhow!("Failed to create PPMd7 decoder: {:?}", e))?;

    // Allocate output buffer and read the decompressed data
    let mut output = vec![0u8; expected_size];
    decoder.read_exact(&mut output)
        .map_err(|e| anyhow!("PPMd7 decompression failed: {}", e))?;

    Ok(output)
}

/// PPMD compression function using ppmd-rust crate (PPMd7/PPMdH variant)
/// This is 64-bit compatible unlike the FreeARC PPMD implementation
pub fn ppmd_compress(input: &[u8], order: u8, memory_size: usize) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    // Allocate output buffer
    let mut output = Vec::new();
    
    // Create an encoder with the output writer and specified parameters
    // Ppmd7Encoder::new(writer, order, mem_size)
    let mut encoder = Ppmd7Encoder::new(&mut output, order as u32, memory_size as u32)
        .map_err(|e| anyhow!("Failed to create PPMd7 encoder: {:?}", e))?;
    
    // Write the input data to the encoder
    encoder.write_all(input)
        .map_err(|e| anyhow!("PPMd7 compression failed: {}", e))?;
    
    // Finish encoding without end marker (7z format stores size separately)
    encoder.finish(false)
        .map_err(|e| anyhow!("PPMd7 finish failed: {}", e))?;

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let params = [(4u8, 4 * 1024 * 1024), (6u8, 8 * 1024 * 1024), (8u8, 16 * 1024 * 1024)];
        for (order, mem) in params {
            let compressed = ppmd_compress(&data, order, mem).unwrap();
            let decompressed = ppmd_decompress(&compressed, data.len(), order, mem).unwrap();
            assert_eq!(data.as_slice(), decompressed.as_slice());
        }
    }
}