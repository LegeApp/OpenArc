// HEIC/HEIF Decoding via pure Rust heic-decoder crate
// Provides HEIC/HEIF decode support for Apple HEIC and HEIF image formats
// Used by Samsung, Android, and Apple devices
// 
// This implementation uses the heic-decoder crate for 100% safe, native Rust decoding
// without FFI dependencies. This ensures correct BT.601 color space handling.

use std::path::Path;
use anyhow::{Result, anyhow, Context};
use heic_decoder::{HeicDecoder as RustHeicDecoder, DecodedImage as RustDecodedImage};

// Compression format for encoding (kept for API compatibility)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeifCompressionFormat {
    Undefined = 0,
    HEVC = 1,      // H.265 - standard HEIC
    AVC = 2,       // H.264
    JPEG = 3,
    AV1 = 4,       // AVIF
}

/// Decoded HEIC image data (RGB/RGBA format)
#[derive(Debug)]
pub struct DecodedHeicImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub has_alpha: bool,
}

/// Decoded HEIC image in YCbCr planar format (for BPG encoding)
/// This format is optimal for direct encoding to BPG without RGB conversion,
/// preserving color accuracy and avoiding the green tint issue.
#[derive(Debug)]
pub struct DecodedHeicYCbCr {
    pub width: u32,
    pub height: u32,
    pub y_plane: Vec<u8>,
    pub cb_plane: Vec<u8>,
    pub cr_plane: Vec<u8>,
    pub y_stride: u32,
    pub cb_stride: u32,
    pub cr_stride: u32,
}

/// HEIC encoder configuration (kept for API compatibility)
#[derive(Debug, Clone)]
pub struct HeicEncoderConfig {
    /// Quality for lossy encoding (1-100, higher is better)
    pub quality: u8,
    /// Use lossless compression
    pub lossless: bool,
    /// Compression format (HEVC for HEIC, AV1 for AVIF)
    pub format: HeifCompressionFormat,
}

impl Default for HeicEncoderConfig {
    fn default() -> Self {
        Self {
            quality: 90,
            lossless: false,
            format: HeifCompressionFormat::HEVC,
        }
    }
}

/// HEIC Codec using pure Rust heic-decoder
pub struct HeicCodec {
    decoder: RustHeicDecoder,
}

impl HeicCodec {
    /// Create a new HEIC codec
    pub fn new() -> Result<Self> {
        Ok(Self {
            decoder: RustHeicDecoder::new(),
        })
    }

    /// Check if HEIC decoding is available (always true for Rust decoder)
    pub fn is_available() -> bool {
        true
    }

    /// Get decoder version
    pub fn get_version() -> Option<String> {
        Some("heic-decoder-rs 0.1.0 (pure Rust)".to_string())
    }

    /// Decode a HEIC/HEIF file to RGBA data
    pub fn decode_file(&mut self, path: &Path) -> Result<DecodedHeicImage> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read HEIC file: {}", path.display()))?;
        self.decode_from_memory(&data)
    }

    /// Decode from memory buffer
    pub fn decode_from_memory(&mut self, buffer: &[u8]) -> Result<DecodedHeicImage> {
        let decoded = self.decoder.decode(buffer)
            .context("Failed to decode HEIC from memory")?;
        
        Ok(DecodedHeicImage {
            width: decoded.width,
            height: decoded.height,
            data: decoded.data,
            has_alpha: decoded.has_alpha,
        })
    }

    /// Decode a HEIC/HEIF file to YCbCr 4:2:0 planar format (optimal for BPG encoding)
    /// 
    /// This method decodes directly to YCbCr without RGB conversion, ensuring perfect
    /// color accuracy with correct BT.601 color space transform.
    pub fn decode_file_ycbcr420(&mut self, path: &Path) -> Result<DecodedHeicYCbCr> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read HEIC file: {}", path.display()))?;
        
        // Decode to YCbCr frame
        let frame = self.decoder.decode_to_frame(&data)
            .context("Failed to decode HEIC to YCbCr frame")?;
        
        // Convert DecodedFrame to YCbCr planar format
        // The frame contains u16 planes (for bit depths > 8), but BPG expects u8 for 8-bit
        let width = frame.cropped_width();
        let height = frame.cropped_height();
        let bit_depth = frame.bit_depth;
        let shift = if bit_depth > 8 { bit_depth - 8 } else { 0 };
        
        // Extract and crop Y plane
        let mut y_plane = Vec::with_capacity((width * height) as usize);
        let y_start = frame.crop_top;
        let y_end = frame.height - frame.crop_bottom;
        let x_start = frame.crop_left;
        let x_end = frame.width - frame.crop_right;
        
        for y in y_start..y_end {
            for x in x_start..x_end {
                let idx = (y * frame.width + x) as usize;
                let val = frame.y_plane[idx];
                y_plane.push((val >> shift) as u8);
            }
        }
        
        // Extract and crop Cb/Cr planes (4:2:0 subsampled)
        let chroma_width = (width + 1) / 2;
        let chroma_height = (height + 1) / 2;
        let mut cb_plane = Vec::with_capacity((chroma_width * chroma_height) as usize);
        let mut cr_plane = Vec::with_capacity((chroma_width * chroma_height) as usize);
        
        let c_stride = frame.c_stride();
        let crop_top_c = frame.crop_top / 2;
        let crop_bottom_c = frame.crop_bottom / 2;
        let crop_left_c = frame.crop_left / 2;
        let crop_right_c = frame.crop_right / 2;
        
        let chroma_h_full = (frame.height + 1) / 2;
        let chroma_w_full = (frame.width + 1) / 2;
        
        for y in crop_top_c..(chroma_h_full - crop_bottom_c) {
            for x in crop_left_c..(chroma_w_full - crop_right_c) {
                let idx = (y as usize) * c_stride + (x as usize);
                if idx < frame.cb_plane.len() {
                    let cb_val = frame.cb_plane[idx];
                    let cr_val = frame.cr_plane[idx];
                    cb_plane.push((cb_val >> shift) as u8);
                    cr_plane.push((cr_val >> shift) as u8);
                } else {
                    // Fallback for safety
                    cb_plane.push(128);
                    cr_plane.push(128);
                }
            }
        }
        
        Ok(DecodedHeicYCbCr {
            width,
            height,
            y_plane,
            cb_plane,
            cr_plane,
            y_stride: width,
            cb_stride: chroma_width,
            cr_stride: chroma_width,
        })
    }

    /// Decode HEIC and save as PNG (lossless intermediate format)
    pub fn decode_to_png(&mut self, input_path: &Path, output_path: &Path) -> Result<()> {
        let decoded = self.decode_file(input_path)?;

        // Create proper ImageBuffer from decoded data
        use image::{ImageBuffer, Rgb, Rgba, DynamicImage};
        
        let img = if decoded.has_alpha {
            let rgba_buf = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                decoded.width,
                decoded.height,
                decoded.data
            ).ok_or_else(|| anyhow!("Failed to create RGBA image buffer"))?;
            DynamicImage::ImageRgba8(rgba_buf)
        } else {
            let rgb_buf = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(
                decoded.width,
                decoded.height,
                decoded.data
            ).ok_or_else(|| anyhow!("Failed to create RGB image buffer"))?;
            DynamicImage::ImageRgb8(rgb_buf)
        };
        
        img.save(output_path)?;
        Ok(())
    }

    /// Decode HEIC and save as JPEG
    pub fn decode_to_jpeg(&mut self, input_path: &Path, output_path: &Path, quality: u8) -> Result<()> {
        let decoded = self.decode_file(input_path)?;

        use image;
        
        let dynamic_img = if decoded.has_alpha {
            image::DynamicImage::ImageRgba8(
                image::ImageBuffer::from_raw(decoded.width, decoded.height, decoded.data)
                    .ok_or_else(|| anyhow!("Invalid buffer size"))?
            )
        } else {
            image::DynamicImage::ImageRgb8(
                image::ImageBuffer::from_raw(decoded.width, decoded.height, decoded.data)
                    .ok_or_else(|| anyhow!("Invalid buffer size"))?
            )
        };

        // Fast intrinsic conversion to RGB8 (strips alpha efficiently)
        let rgb_img = dynamic_img.into_rgb8();

        let mut output_file = std::fs::File::create(output_path)?;
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output_file, quality);
        rgb_img.write_with_encoder(encoder)?;
        Ok(())
    }

    /// Encode PNG to HEIC (stub - encoding not supported by pure Rust decoder yet)
    pub fn png_to_heic(&mut self, _input_path: &Path, _output_path: &Path, _config: &HeicEncoderConfig) -> Result<()> {
        Err(anyhow!("HEIC encoding not supported - decoding only"))
    }

    /// Encode RGB/RGBA data to HEIC file (stub - encoding not supported)
    pub fn encode_to_file(
        &mut self,
        _data: &[u8],
        _width: u32,
        _height: u32,
        _has_alpha: bool,
        _output_path: &Path,
        _config: &HeicEncoderConfig,
    ) -> Result<()> {
        Err(anyhow!("HEIC encoding not supported - decoding only"))
    }
}

// Safe to send between threads
unsafe impl Send for HeicCodec {}

// Legacy type alias for backward compatibility
pub type HeicDecoder = HeicCodec;

/// Check if a file is a HEIC/HEIF file by extension
pub fn is_heic_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let lower = e.to_lowercase();
            lower == "heic" || lower == "heif" || lower == "hif"
        })
        .unwrap_or(false)
}

/// Decode a HEIC file to raw RGBA data (convenience function)
pub fn decode_heic_file(path: &Path) -> Result<DecodedHeicImage> {
    let mut codec = HeicCodec::new()?;
    codec.decode_file(path)
}

/// Decode HEIC to PNG (convenience function)
pub fn heic_to_png(input: &Path, output: &Path) -> Result<()> {
    let mut codec = HeicCodec::new()?;
    codec.decode_to_png(input, output)
}

/// Decode HEIC to JPEG (convenience function)
pub fn heic_to_jpeg(input: &Path, output: &Path, quality: u8) -> Result<()> {
    let mut codec = HeicCodec::new()?;
    codec.decode_to_jpeg(input, output, quality)
}

/// Encode PNG to HEIC (stub - not supported)
pub fn png_to_heic(_input: &Path, _output: &Path, _quality: u8) -> Result<()> {
    Err(anyhow!("HEIC encoding not supported - decoding only"))
}

/// Encode PNG to HEIC losslessly (stub - not supported)
pub fn png_to_heic_lossless(_input: &Path, _output: &Path) -> Result<()> {
    Err(anyhow!("HEIC encoding not supported - decoding only"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heic_detection() {
        assert!(is_heic_file(Path::new("test.heic")));
        assert!(is_heic_file(Path::new("test.HEIC")));
        assert!(is_heic_file(Path::new("test.heif")));
        assert!(!is_heic_file(Path::new("test.jpg")));
        assert!(!is_heic_file(Path::new("test.png")));
    }

    #[test]
    fn test_availability() {
        let available = HeicCodec::is_available();
        assert!(available, "Pure Rust decoder should always be available");

        if let Some(ver) = HeicCodec::get_version() {
            println!("HEIC decoder version: {}", ver);
        }
    }
}
