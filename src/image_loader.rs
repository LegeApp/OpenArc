// Image loading with zune-jpeg for JPEG optimization
// Uses the image crate for all other formats (PNG, WebP, TIFF, etc.)

use anyhow::{Context, Result, anyhow};
use std::path::Path;
use std::process::Command;
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

fn is_jpeg2000(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_lowercase().as_str(), "jp2" | "j2k" | "j2c" | "jpc" | "jpt" | "jph" | "jhc"))
        .unwrap_or(false)
}

fn find_opj_decompress() -> Option<std::path::PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let local = dir.join("opj_decompress");
            if local.exists() {
                return Some(local);
            }
        }
    }

    if let Ok(path) = std::env::var("PATH") {
        for p in std::env::split_paths(&path) {
            let cand = p.join("opj_decompress");
            if cand.exists() {
                return Some(cand);
            }
        }
    }

    None
}

fn decode_jpeg2000_via_openjp2(path: &Path) -> Result<DecodedImage> {
    let opj_decompress = find_opj_decompress()
        .ok_or_else(|| anyhow!("opj_decompress not found (build openjp2 tool or add it to PATH)"))?;

    let tmp = tempfile::Builder::new()
        .prefix("openarc-jp2-")
        .suffix(".png")
        .tempfile()?;
    let out_path = tmp.path().to_path_buf();

    let out = Command::new(opj_decompress)
        .arg("-i")
        .arg(path)
        .arg("-o")
        .arg(&out_path)
        .output()
        .with_context(|| format!("Failed to execute openjp2 decompressor for {}", path.display()))?;

    if !out.status.success() {
        return Err(anyhow!(
            "openjp2 decode failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    let img = image::open(&out_path)
        .with_context(|| format!("Failed to read openjp2 output: {}", out_path.display()))?;
    Ok(DecodedImage { img })
}

/// Load an image from a file
/// Uses zune-jpeg for JPEG files (faster), image crate for everything else
pub fn load_image(path: &Path) -> Result<DecodedImage> {
    if is_jpeg(path) {
        // Use zune-jpeg for faster JPEG decoding
        decode_jpeg_from_file(path)
    } else if is_jpeg2000(path) {
        decode_jpeg2000_via_openjp2(path)
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
