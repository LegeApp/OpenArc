//! CPU-based thumbnail generation with fast_image_resize
//!
//! Provides high-performance CPU thumbnailing with optimized fast paths:
//! - JPEG: YCbCr decode → resize → encode (no RGB conversion)
//! - HEIC: YCbCr decode → resize → encode (no RGB conversion)
//! - Other: Universal decode → RGB → resize → encode

pub mod ycbcr;
pub mod jpeg_fast_path;
pub mod heic_fast_path;

use std::path::Path;
use rayon::prelude::*;

/// Generate a single thumbnail using CPU
pub fn generate_thumbnail(
    input_path: &Path,
    output_path: &Path,
    size: u32,
    quality: u8,
) -> Result<(), String> {
    let ext = input_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // Fast path: JPEG → YCbCr → resize YCbCr → encode JPEG
    if ext == "jpg" || ext == "jpeg" {
        return jpeg_fast_path::generate_jpeg_thumbnail(input_path, output_path, size, quality);
    }

    // Fast path: HEIC → YCbCr → resize YCbCr → encode JPEG
    if ext == "heic" || ext == "heif" {
        return heic_fast_path::generate_heic_thumbnail(input_path, output_path, size, quality);
    }

    // Slow path: Universal decode → RGB → resize RGB → encode JPEG
    generate_rgb_path(input_path, output_path, size, quality)
}

/// Generate thumbnail via RGB path (for non-JPEG/HEIC formats)
fn generate_rgb_path(
    input_path: &Path,
    output_path: &Path,
    size: u32,
    quality: u8,
) -> Result<(), String> {
    use fast_image_resize::{Resizer, ResizeAlg, ResizeOptions, PixelType, FilterType};
    use fast_image_resize::images::Image;

    // Decode with image crate (supports PNG, TIFF, WebP, BMP, etc.)
    let img = image::open(input_path)
        .map_err(|e| format!("Image decode failed: {}", e))?;

    // Convert to RGB8
    let rgb_img = img.to_rgb8();
    let width = rgb_img.width();
    let height = rgb_img.height();

    // Calculate aspect-preserving dimensions
    let src_aspect = width as f32 / height as f32;
    let target_aspect = size as f32 / size as f32;

    let (resize_width, resize_height) = if (src_aspect - target_aspect).abs() < 0.01 {
        (size, size)
    } else if src_aspect > target_aspect {
        // Wider - fit to width
        let h = (size as f32 / src_aspect).round() as u32;
        (size, h.max(1))
    } else {
        // Taller - fit to height
        let w = (size as f32 * src_aspect).round() as u32;
        (w.max(1), size)
    };

    // Resize using fast_image_resize
    let mut src_data = rgb_img.into_raw();
    let src_img = Image::from_slice_u8(width, height, &mut src_data, PixelType::U8x3)
        .map_err(|e| format!("Failed to create source image: {:?}", e))?;

    let mut dst = vec![0u8; (resize_width * resize_height * 3) as usize];
    let mut dst_img = Image::from_slice_u8(resize_width, resize_height, &mut dst, PixelType::U8x3)
        .map_err(|e| format!("Failed to create dest image: {:?}", e))?;

    let mut resizer = Resizer::new();
    let opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear));

    resizer
        .resize(&src_img, &mut dst_img, Some(&opts))
        .map_err(|e| format!("Resize failed: {:?}", e))?;

    // Encode as JPEG
    let output_file = std::fs::File::create(output_path)
        .map_err(|e| format!("Create file failed: {:?}: {}", output_path, e))?;

    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(output_file, quality);

    encoder
        .encode(
            &dst,
            resize_width,
            resize_height,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("JPEG encoding failed: {}", e))?;

    Ok(())
}

/// Generate thumbnails in batch using Rayon parallelism
pub fn generate_thumbnail_batch(
    inputs: &[(u64, &Path)], // (source_id, input_path)
    output_dir: &Path,
    size: u32,
    quality: u8,
) -> Vec<(u64, Result<(), String>)> {
    inputs
        .par_iter()
        .map(|(source_id, input_path)| {
            let output_path = output_dir.join(format!("{}.jpg", source_id));
            let result = generate_thumbnail(input_path, &output_path, size, quality);
            (*source_id, result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_cpu_thumbnail_generation() {
        // This test would require actual image files
        // For now, just verify the module compiles
        assert!(true);
    }
}
