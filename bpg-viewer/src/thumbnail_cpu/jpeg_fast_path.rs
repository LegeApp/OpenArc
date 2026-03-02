//! JPEG fast path: decode → YCbCr resize → encode
//!
//! This path uses TooJpeg with YCbCr 4:2:0 support for direct encoding.

use super::ycbcr::YCbCr420Data;
use std::path::Path;

/// Generate JPEG thumbnail using YCbCr fast path
pub fn generate_jpeg_thumbnail(
    input_path: &Path,
    output_path: &Path,
    size: u32,
    quality: u8,
) -> Result<(), String> {
    // 1. Decode JPEG to YCbCr 4:2:0
    let ycbcr = decode_jpeg_to_ycbcr(input_path)?;

    // 2. Resize YCbCr directly (NO RGB conversion during resize)
    let resized = ycbcr.resize(size, size)?;

    // 3. Encode YCbCr back to JPEG using TooJpeg
    encode_ycbcr_to_jpeg(&resized, output_path, quality)?;

    Ok(())
}

/// Decode JPEG file to YCbCr 4:2:0 planar
fn decode_jpeg_to_ycbcr(path: &Path) -> Result<YCbCr420Data, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Read failed: {}", e))?;

    // Use zune-jpeg for YCbCr decoding
    let ycbcr = zune_image::decode_jpeg_ycbcr(&bytes)
        .map_err(|e| format!("JPEG YCbCr decode failed: {}", e))?;

    Ok(YCbCr420Data::new(
        ycbcr.y_plane,
        ycbcr.cb_plane,
        ycbcr.cr_plane,
        ycbcr.width,
        ycbcr.height,
    ))
}

/// Encode YCbCr 4:2:0 data to JPEG using TooJpeg
///
/// TooJpeg now supports YCbCr 4:2:0 input directly. While it still converts
/// to RGB internally for the DCT step, this avoids an extra conversion on our end.
pub(super) fn encode_ycbcr_to_jpeg(
    ycbcr: &YCbCr420Data,
    output_path: &Path,
    quality: u8,
) -> Result<(), String> {
    // Prepare YCbCr 4:2:0 planar buffer: Y plane + Cb plane + Cr plane
    let y_size = (ycbcr.width * ycbcr.height) as usize;
    let c_width = (ycbcr.width + 1) / 2;
    let c_height = (ycbcr.height + 1) / 2;
    let c_size = (c_width * c_height) as usize;

    let mut ycbcr_buffer = Vec::with_capacity(y_size + 2 * c_size);
    ycbcr_buffer.extend_from_slice(&ycbcr.y_plane);
    ycbcr_buffer.extend_from_slice(&ycbcr.cb_plane);
    ycbcr_buffer.extend_from_slice(&ycbcr.cr_plane);

    // Configure TooJpeg encoder
    let options = toojpeg::EncodeOptions {
        width: ycbcr.width,
        height: ycbcr.height,
        format: toojpeg::ImageFormat::YCbCr420,
        quality,
        baseline: true,
        optimized: true,
        downsample: true,
    };

    // Encode to file
    let mut output_file = std::fs::File::create(output_path)
        .map_err(|e| format!("Create file failed: {:?}: {}", output_path, e))?;

    toojpeg::encode_jpeg(&ycbcr_buffer, options, &mut output_file)
        .map_err(|e| format!("JPEG encoding failed: {}", e))?;

    Ok(())
}
