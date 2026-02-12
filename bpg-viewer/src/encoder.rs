//! BPG Encoder stub (encoder functionality disabled)
//!
//! The encoder has been disabled but this stub exists for backwards compatibility
//! with code that references it.

use anyhow::{Result, bail};

/// BPG Encoder (disabled stub)
pub struct BPGEncoder {
    _quality: u8,
}

impl BPGEncoder {
    /// Create encoder with quality (stub - always fails)
    pub fn with_quality(_quality: u8) -> Result<Self> {
        bail!("BPG encoder is disabled")
    }

    /// Encode image (stub - always fails)
    pub fn encode(&self, _width: u32, _height: u32, _data: &[u8]) -> Result<Vec<u8>> {
        bail!("BPG encoder is disabled")
    }

    /// Encode from memory (stub - always fails)
    pub fn encode_from_memory(&self, _data: &[u8], _width: u32, _height: u32, _stride: u32, _format: crate::ffi::BPGImageFormat) -> Result<Vec<u8>> {
        bail!("BPG encoder is disabled")
    }
}
