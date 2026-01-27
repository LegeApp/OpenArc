// BPG encoding wrapper for FFI use
use anyhow::{Context, Result};
use codecs::bpg::{BPGEncoderConfig, NativeBPGEncoder};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct BpgConfig {
    pub quality: u8,
    pub lossless: bool,
    pub bit_depth: u8,
    pub chroma_format: u8,
    pub encoder_type: u8,
    pub compression_level: u8,
}

impl BpgConfig {
    pub fn to_encoder_config(&self) -> BPGEncoderConfig {
        BPGEncoderConfig {
            quality: self.quality as i32,
            bit_depth: self.bit_depth as i32,
            lossless: if self.lossless { 1 } else { 0 },
            chroma_format: self.chroma_format as i32,
            encoder_type: self.encoder_type as i32,
            compress_level: self.compression_level as i32,
        }
    }
}

pub fn encode_image_to_bpg<P: AsRef<Path>>(input_path: P, output_path: P, config: &BpgConfig) -> Result<()> {
    let input_str = input_path.as_ref().to_str()
        .context("Invalid input path")?;
    let output_str = output_path.as_ref().to_str()
        .context("Invalid output path")?;

    let mut encoder = NativeBPGEncoder::new().context("Failed to create BPG encoder")?;
    let encoder_config = config.to_encoder_config();
    encoder.set_config(&encoder_config).context("Failed to set BPG config")?;
    encoder.encode_to_file(input_str, output_str).context("Failed to encode BPG file")?;
    Ok(())
}
