// BPG Encoder Module
// NOTE: Encoder functionality is NOT available in the in-memory-only modified BPG library
// This module provides stub implementations that return errors

use anyhow::{Result, anyhow};
use crate::ffi::{BPGEncoderConfig, BPGImageFormat};

/// Safe Rust wrapper for BPG encoder (stub - encoder not available in modified library)
pub struct BPGEncoder {
    _phantom: (),
}

impl BPGEncoder {
    /// Create encoder with default configuration
    pub fn new() -> Result<Self> {
        Err(anyhow!("BPG encoding not available - library compiled without encoder support"))
    }

    /// Create encoder with custom quality (0-51, lower is better)
    pub fn with_quality(_quality: u8) -> Result<Self> {
        Err(anyhow!("BPG encoding not available - library compiled without encoder support"))
    }

    /// Create encoder with custom configuration
    pub fn with_config(_config: &BPGEncoderConfig) -> Result<Self> {
        Err(anyhow!("BPG encoding not available - library compiled without encoder support"))
    }

    /// Get default configuration
    pub fn default_config() -> BPGEncoderConfig {
        // Return a reasonable default even though encoding isn't available
        BPGEncoderConfig {
            quality: 28,
            bit_depth: 8,
            lossless: 0,
            chroma_format: 1,
            encoder_type: 0,
            compress_level: 8,
        }
    }

    /// Set encoder configuration
    pub fn set_config(&mut self, _config: &BPGEncoderConfig) -> Result<()> {
        Err(anyhow!("BPG encoding not available - library compiled without encoder support"))
    }

    /// Encode image file to BPG (returns encoded data)
    pub fn encode_from_file(&self, _input_path: &str) -> Result<Vec<u8>> {
        Err(anyhow!("BPG encoding not available - library compiled without encoder support"))
    }

    /// Encode image file to BPG file
    pub fn encode_to_file(&self, _input_path: &str, _output_path: &str) -> Result<()> {
        Err(anyhow!("BPG encoding not available - library compiled without encoder support"))
    }

    /// Encode raw image data to BPG
    pub fn encode_from_memory(
        &self,
        _data: &[u8],
        _width: u32,
        _height: u32,
        _stride: u32,
        _format: BPGImageFormat,
    ) -> Result<Vec<u8>> {
        Err(anyhow!("BPG encoding not available - library compiled without encoder support"))
    }
}

impl Drop for BPGEncoder {
    fn drop(&mut self) {
        // No cleanup needed for stub implementation
    }
}

unsafe impl Send for BPGEncoder {}
unsafe impl Sync for BPGEncoder {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_creation() {
        let encoder = BPGEncoder::new();
        // Should return error since encoding is not supported
        assert!(encoder.is_err());
    }

    #[test]
    fn test_quality_encoder() {
        let encoder = BPGEncoder::with_quality(25);
        // Should return error since encoding is not supported
        assert!(encoder.is_err());
    }

    #[test]
    fn test_default_config() {
        let config = BPGEncoder::default_config();
        assert!(config.quality > 0);
        assert!(config.bit_depth > 0);
    }
}
