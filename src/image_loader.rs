// Image loading with zune-jpeg for JPEG optimization
// Uses the image crate for all other formats (PNG, WebP, TIFF, etc.)

use anyhow::{Context, Result, anyhow};
use std::path::Path;
use image::DynamicImage;

/// Decoded image data (wraps image::DynamicImage)
pub struct DecodedImage {
    pub img: DynamicImage,
}

impl DecodedImage {
    /// Convert to RGBA8
    pub fn to_rgba8(self) -> (Vec<u8>, u32, u32) {
        let rgba = self.img.to_rgba8();
        let (width, height) = rgba.dimensions();
        (rgba.into_raw(), width, height)
    }
}

/// Check if file is JPEG based on extension
fn is_jpeg(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_lowercase().as_str(), "jpg" | "jpeg"))
        .unwrap_or(false)
}

/// Load an image from a file
/// Uses zune-jpeg for JPEG files (faster), image crate for everything else
pub fn load_image(path: &Path) -> Result<DecodedImage> {
    if is_jpeg(path) {
        // Use zune-jpeg for faster JPEG decoding
        decode_jpeg_from_file(path)
    } else {
        // Use image crate for all other formats (PNG, WebP, TIFF, BMP, etc.)
        let img = image::open(path)
            .with_context(|| format!("Failed to load image: {}", path.display()))?;
        Ok(DecodedImage { img })
    }
}

/// Decode JPEG using zune-jpeg (faster than image crate's JPEG decoder)
fn decode_jpeg_from_file(path: &Path) -> Result<DecodedImage> {
    use zune_image::JpegDecoder;
    use std::fs;
    use std::io::Cursor;

    let data = fs::read(path)
        .with_context(|| format!("Failed to read JPEG file: {}", path.display()))?;

    let mut decoder = JpegDecoder::new(Cursor::new(&data))
        .map_err(|e| anyhow!("Failed to create JPEG decoder: {}", e))?;

    let (pixels, width, height) = decoder.decode_rgb()
        .map_err(|e| anyhow!("JPEG decode error: {}", e))?;
    
    // Convert to RGB8 ImageBuffer and then to DynamicImage
    use image::{DynamicImage, RgbImage};
    
    let rgb_buf = RgbImage::from_raw(width, height, pixels)
        .ok_or_else(|| anyhow!("Failed to create RGB image buffer"))?;
    let img = DynamicImage::ImageRgb8(rgb_buf);

    Ok(DecodedImage { img })
}
