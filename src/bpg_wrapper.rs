// BPG encoding wrapper for FFI use
use anyhow::{Context, Result};
use codecs::bpg::{BPGEncoderConfig, BPGImageFormat, NativeBPGEncoder};
use codecs::heic::HeicCodec;
use std::path::Path;
use crate::image_loader;

#[derive(Debug, Clone)]
pub struct BpgConfig {
    pub quality: u8,
    pub lossless: bool,
    pub bit_depth: u8,
    pub encoder_type: u8,
    pub compression_level: u8,
}

impl BpgConfig {
    pub fn to_encoder_config(&self, chroma_format: i32) -> BPGEncoderConfig {
        BPGEncoderConfig {
            quality: self.quality as i32,
            bit_depth: self.bit_depth as i32,
            lossless: if self.lossless { 1 } else { 0 },
            chroma_format,
            encoder_type: self.encoder_type as i32,
            compress_level: self.compression_level as i32,
            color_space: 3, // YCbCr BT.709 (better for HEIC/HEIF sources)
        }
    }
}

pub fn encode_image_to_bpg<P: AsRef<Path>>(input_path: P, output_path: P, config: &BpgConfig) -> Result<()> {
    let input_path_ref = input_path.as_ref();
    let input_str = input_path_ref.to_str()
        .context("Invalid input path")?;
    let output_str = output_path.as_ref().to_str()
        .context("Invalid output path")?;

    // Check if the input is HEIC/HEIF format
    let extension = input_path_ref.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    let is_heic = matches!(extension.as_str(), "heic" | "heif");
    
    // If HEIC/HEIF, decode to native YCbCr 4:2:0 and encode (optimal path)
    // Pure Rust decoder is always available - no feature flag needed
    if is_heic {
        let mut decoder = HeicCodec::new().context("Failed to create HEIC decoder")?;
        let decoded = decoder.decode_file_ycbcr420(input_path_ref)
            .context("Failed to decode HEIC file to YCbCr 4:2:0")?;
        
        // HEIC always uses YCbCr 4:2:0 (its native format)
        let mut encoder = NativeBPGEncoder::new().context("Failed to create BPG encoder")?;
        let encoder_config = config.to_encoder_config(0); // 0 = YCbCr 4:2:0
        encoder.set_config(&encoder_config).context("Failed to set BPG config")?;
        
        let bpg_data = encoder.encode_from_ycbcr420_planar(
            &decoded.y_plane,
            &decoded.cb_plane,
            &decoded.cr_plane,
            decoded.width,
            decoded.height,
            decoded.y_stride,
            decoded.cb_stride,
            decoded.cr_stride,
        ).context("Failed to encode HEIC to BPG via YCbCr 4:2:0")?;
        
        std::fs::write(output_path.as_ref(), bpg_data)
            .context("Failed to write BPG file")?;
        
        return Ok(());
    }

    // For other formats (JPEG, PNG, WebP, TIFF, etc.), load into memory and encode as RGBA
    let decoded = image_loader::load_image(input_path.as_ref())
        .context("Failed to load image file")?;
    
    let chroma_format = 0; // Default to 4:2:0 (most efficient for photos)
    
    let mut encoder = NativeBPGEncoder::new().context("Failed to create BPG encoder")?;
    let encoder_config = config.to_encoder_config(chroma_format);
    encoder.set_config(&encoder_config).context("Failed to set BPG config")?;
    
    let (data, width, height) = decoded.to_rgba8();
    let stride = width * 4;
    
    let bpg_data = encoder.encode_from_memory(
        &data,
        width,
        height,
        stride,
        BPGImageFormat::RGBA32,
    ).context("Failed to encode image to BPG")?;
    
    std::fs::write(output_path.as_ref(), bpg_data)
        .context("Failed to write BPG file")?;
    
    Ok(())
}
