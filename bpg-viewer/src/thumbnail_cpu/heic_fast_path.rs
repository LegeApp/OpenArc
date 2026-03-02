//! HEIC fast path: decode → YCbCr resize → encode
//!
//! HEIC natively decodes to YCbCr, making this very efficient.

use super::ycbcr::YCbCr420Data;
use std::path::Path;

/// Generate thumbnail from HEIC using YCbCr fast path
pub fn generate_heic_thumbnail(
    input_path: &Path,
    output_path: &Path,
    size: u32,
    quality: u8,
) -> Result<(), String> {
    // 1. Decode HEIC to YCbCr 4:2:0
    let ycbcr = decode_heic_to_ycbcr(input_path)?;

    // 2. Resize YCbCr directly
    let resized = ycbcr.resize(size, size)?;

    // 3. Encode to JPEG (reuse JPEG encoder)
    super::jpeg_fast_path::encode_ycbcr_to_jpeg(&resized, output_path, quality)?;

    Ok(())
}

/// Decode HEIC file to YCbCr 4:2:0
fn decode_heic_to_ycbcr(path: &Path) -> Result<YCbCr420Data, String> {
    use codecs::heic::HeicCodec;

    let mut decoder = HeicCodec::new()
        .map_err(|e| format!("HEIC codec init failed: {}", e))?;

    let ycbcr = decoder
        .decode_file_ycbcr420(path)
        .map_err(|e| format!("HEIC YCbCr decode failed: {}", e))?;

    Ok(YCbCr420Data::new(
        ycbcr.y_plane,
        ycbcr.cb_plane,
        ycbcr.cr_plane,
        ycbcr.width,
        ycbcr.height,
    ))
}

