// BPG Thumbnail Generation Module
use std::path::Path;
use std::io::BufWriter;
use std::fs::File;
use anyhow::Result;
use image::{DynamicImage, ImageBuffer, Rgba, imageops::FilterType};

use crate::decoder::{decode_file, DecodedImage};
use crate::encoder::BPGEncoder;
use crate::ffi::BPGImageFormat;

/// Thumbnail generator configuration
#[derive(Debug, Clone)]
pub struct ThumbnailConfig {
    pub max_width: u32,
    pub max_height: u32,
    pub quality: u8,
    pub filter: FilterType,
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            max_width: 256,
            max_height: 256,
            quality: 28,
            // Use Triangle (bilinear) for speed - good enough for thumbnails
            filter: FilterType::Triangle,
        }
    }
}

/// Thumbnail generator for BPG images
pub struct ThumbnailGenerator {
    config: ThumbnailConfig,
}

impl ThumbnailGenerator {
    /// Create a new thumbnail generator with default settings
    pub fn new() -> Self {
        Self {
            config: ThumbnailConfig::default(),
        }
    }

    /// Create a thumbnail generator with custom config
    pub fn with_config(config: ThumbnailConfig) -> Self {
        Self { config }
    }

    /// Create a thumbnail generator with specific dimensions
    pub fn with_dimensions(max_width: u32, max_height: u32) -> Self {
        Self {
            config: ThumbnailConfig {
                max_width,
                max_height,
                ..Default::default()
            },
        }
    }

    /// Generate a thumbnail from a BPG file
    pub fn generate_thumbnail(&self, input_path: &Path) -> Result<Vec<u8>> {
        // Decode the full BPG image
        let decoded = decode_file(input_path.to_str().unwrap())?;

        // Calculate new dimensions
        let (new_width, new_height) = self.calculate_dimensions(decoded.width, decoded.height);

        // Convert to RGBA32 for processing
        let rgba_data = decoded.to_rgba32()?;

        // Resize the image
        let thumbnail_data = self.resize_image(
            &rgba_data,
            decoded.width,
            decoded.height,
            new_width,
            new_height,
        )?;

        Ok(thumbnail_data)
    }

    /// Generate a thumbnail and save it as BPG
    pub fn generate_thumbnail_to_file(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        let thumbnail_data = self.generate_thumbnail(input_path)?;

        // Re-encode as BPG
        let decoded = decode_file(input_path.to_str().unwrap())?;
        let (new_width, new_height) = self.calculate_dimensions(decoded.width, decoded.height);

        let encoder = BPGEncoder::with_quality(self.config.quality)?;
        let bpg_data = encoder.encode_from_memory(
            &thumbnail_data,
            new_width,
            new_height,
            new_width * 4,
            BPGImageFormat::RGBA32,
        )?;

        std::fs::write(output_path, bpg_data)?;
        Ok(())
    }

    /// Generate a thumbnail and save it as PNG using fast PNG encoder
    pub fn generate_thumbnail_to_png(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        // Decode and get dimensions in one pass
        let decoded = decode_file(input_path.to_str().unwrap())?;
        let (new_width, new_height) = self.calculate_dimensions(decoded.width, decoded.height);

        // Convert to RGBA32 for processing
        let rgba_data = decoded.to_rgba32()?;

        // Resize the image
        let thumbnail_data = self.resize_image(
            &rgba_data,
            decoded.width,
            decoded.height,
            new_width,
            new_height,
        )?;

        // Use fast png crate for encoding with optimized settings
        let file = File::create(output_path)?;
        let writer = BufWriter::with_capacity(64 * 1024, file); // 64KB buffer

        let mut encoder = png::Encoder::new(writer, new_width, new_height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        encoder.set_filter(png::FilterType::Sub); // Faster filter
        encoder.set_adaptive_filter(png::AdaptiveFilterType::NonAdaptive); // Skip filter selection

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&thumbnail_data)?;

        Ok(())
    }

    /// Calculate thumbnail dimensions while maintaining aspect ratio
    fn calculate_dimensions(&self, orig_width: u32, orig_height: u32) -> (u32, u32) {
        let scale_x = self.config.max_width as f32 / orig_width as f32;
        let scale_y = self.config.max_height as f32 / orig_height as f32;
        let scale = scale_x.min(scale_y).min(1.0); // Don't upscale

        let new_width = (orig_width as f32 * scale) as u32;
        let new_height = (orig_height as f32 * scale) as u32;

        (new_width.max(1), new_height.max(1))
    }

    /// Resize image data using the image crate
    fn resize_image(
        &self,
        data: &[u8],
        src_w: u32,
        src_h: u32,
        dst_w: u32,
        dst_h: u32,
    ) -> Result<Vec<u8>> {
        // Create an ImageBuffer from the raw RGBA data
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(src_w, src_h, data.to_vec())
                .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

        // Convert to DynamicImage for resizing
        let dynamic_img = DynamicImage::ImageRgba8(img);

        // Resize using the configured filter
        let resized = dynamic_img.resize_exact(dst_w, dst_h, self.config.filter);

        // Convert back to raw RGBA data
        Ok(resized.to_rgba8().into_raw())
    }
}

impl Default for ThumbnailGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_dimensions() {
        let generator = ThumbnailGenerator::with_dimensions(100, 100);

        // Test landscape
        let (w, h) = generator.calculate_dimensions(200, 100);
        assert_eq!(w, 100);
        assert_eq!(h, 50);

        // Test portrait
        let (w, h) = generator.calculate_dimensions(100, 200);
        assert_eq!(w, 50);
        assert_eq!(h, 100);

        // Test square
        let (w, h) = generator.calculate_dimensions(200, 200);
        assert_eq!(w, 100);
        assert_eq!(h, 100);

        // Test no upscaling
        let (w, h) = generator.calculate_dimensions(50, 50);
        assert_eq!(w, 50);
        assert_eq!(h, 50);
    }
}
