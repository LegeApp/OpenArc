//! Unified image data representation supporting both BGRA and YCbCr formats
//!
//! This module provides a flexible image data abstraction that can hold either:
//! - BGRA8 (interleaved, UI-friendly)
//! - YCbCr 4:2:0 (planar, JPEG/HEIC native format)
//!
//! Benefits:
//! - Avoid YCbCr→BGRA→YCbCr conversion for thumbnails
//! - Keep native format for optimal encoder performance
//! - Single BGRA API for UI compatibility

use anyhow::Result;

/// Pixel buffer abstraction supporting both BGRA and YCbCr
#[derive(Debug, Clone)]
pub enum ImageData {
    /// 4-byte interleaved BGRA (byte order: B, G, R, A)
    /// Most UI frameworks expect this format
    Bgra(Vec<u8>),

    /// YCbCr 4:2:0 planar (Y, Cb, Cr separate buffers)
    /// Native format for JPEG/HEIC - avoids conversion overhead
    Ycbcr420 {
        /// Luma plane (full resolution)
        y: Vec<u8>,
        /// Chroma blue plane (half resolution)
        cb: Vec<u8>,
        /// Chroma red plane (half resolution)
        cr: Vec<u8>,
        /// Width of full-resolution image
        width: u32,
        /// Height of full-resolution image
        height: u32,
        /// Stride of Y plane (usually == width, may be padded)
        y_stride: u32,
        /// Stride of Cb/Cr planes (usually == width/2)
        chroma_stride: u32,
    },
}

impl ImageData {
    /// Create BGRA image data
    pub fn from_bgra(data: Vec<u8>) -> Self {
        ImageData::Bgra(data)
    }

    /// Create YCbCr 4:2:0 image data
    pub fn from_ycbcr420(
        y: Vec<u8>,
        cb: Vec<u8>,
        cr: Vec<u8>,
        width: u32,
        height: u32,
        y_stride: u32,
        chroma_stride: u32,
    ) -> Self {
        ImageData::Ycbcr420 {
            y,
            cb,
            cr,
            width,
            height,
            y_stride,
            chroma_stride,
        }
    }

    /// Convert to BGRA format (allocates new buffer if needed)
    pub fn to_bgra8(&self, width: u32, height: u32) -> Result<Vec<u8>> {
        match self {
            ImageData::Bgra(data) => Ok(data.clone()),
            ImageData::Ycbcr420 {
                y,
                cb,
                cr,
                y_stride,
                chroma_stride,
                ..
            } => {
                // Convert YCbCr 4:2:0 to BGRA
                convert_ycbcr420_to_bgra(
                    y,
                    cb,
                    cr,
                    width,
                    height,
                    *y_stride,
                    *chroma_stride,
                )
            }
        }
    }

    /// Check if data is in BGRA format
    pub fn is_bgra(&self) -> bool {
        matches!(self, ImageData::Bgra(_))
    }

    /// Check if data is in YCbCr format
    pub fn is_ycbcr(&self) -> bool {
        matches!(self, ImageData::Ycbcr420 { .. })
    }

    /// Get format name for debugging
    pub fn format_name(&self) -> &str {
        match self {
            ImageData::Bgra(_) => "BGRA8",
            ImageData::Ycbcr420 { .. } => "YCbCr 4:2:0",
        }
    }
}

/// Convert YCbCr 4:2:0 planar to BGRA interleaved
///
/// Uses BT.601 coefficients for YCbCr→RGB conversion:
/// R = Y + 1.402 * (Cr - 128)
/// G = Y - 0.344 * (Cb - 128) - 0.714 * (Cr - 128)
/// B = Y + 1.772 * (Cb - 128)
fn convert_ycbcr420_to_bgra(
    y_plane: &[u8],
    cb_plane: &[u8],
    cr_plane: &[u8],
    width: u32,
    height: u32,
    y_stride: u32,
    chroma_stride: u32,
) -> Result<Vec<u8>> {
    // Validate input sizes
    let expected_y_size = (height * y_stride) as usize;
    let expected_chroma_size = ((height / 2) * chroma_stride) as usize;

    if y_plane.len() < expected_y_size {
        return Err(anyhow::anyhow!("Y plane too small: expected {}, got {}", expected_y_size, y_plane.len()));
    }
    if cb_plane.len() < expected_chroma_size {
        return Err(anyhow::anyhow!("Cb plane too small: expected {}, got {}", expected_chroma_size, cb_plane.len()));
    }
    if cr_plane.len() < expected_chroma_size {
        return Err(anyhow::anyhow!("Cr plane too small: expected {}, got {}", expected_chroma_size, cr_plane.len()));
    }

    let mut bgra = vec![0u8; (width * height * 4) as usize];

    for row in 0..height {
        for col in 0..width {
            let y_idx = (row * y_stride + col) as usize;
            let chroma_row = row / 2;
            let chroma_col = col / 2;
            let c_idx = (chroma_row * chroma_stride + chroma_col) as usize;

            // Bounds are already validated above, so this is safe
            let y = y_plane[y_idx] as f32;
            let cb = cb_plane[c_idx] as f32 - 128.0;
            let cr = cr_plane[c_idx] as f32 - 128.0;

            // BT.601 conversion
            let r = (y + 1.402 * cr).clamp(0.0, 255.0) as u8;
            let g = (y - 0.344 * cb - 0.714 * cr).clamp(0.0, 255.0) as u8;
            let b = (y + 1.772 * cb).clamp(0.0, 255.0) as u8;

            let bgra_idx = ((row * width + col) * 4) as usize;
            bgra[bgra_idx] = b;
            bgra[bgra_idx + 1] = g;
            bgra[bgra_idx + 2] = r;
            bgra[bgra_idx + 3] = 255; // Alpha
        }
    }

    Ok(bgra)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bgra_passthrough() {
        let data = vec![0u8; 100 * 100 * 4];
        let img_data = ImageData::from_bgra(data.clone());

        assert!(img_data.is_bgra());
        assert!(!img_data.is_ycbcr());
        assert_eq!(img_data.format_name(), "BGRA8");

        let converted = img_data.to_bgra8(100, 100).unwrap();
        assert_eq!(converted, data);
    }

    #[test]
    fn test_ycbcr_creation() {
        let y = vec![128u8; 100 * 100];
        let cb = vec![128u8; 50 * 50];
        let cr = vec![128u8; 50 * 50];

        let img_data = ImageData::from_ycbcr420(
            y.clone(),
            cb.clone(),
            cr.clone(),
            100,
            100,
            100,
            50,
        );

        assert!(!img_data.is_bgra());
        assert!(img_data.is_ycbcr());
        assert_eq!(img_data.format_name(), "YCbCr 4:2:0");
    }

    #[test]
    fn test_ycbcr_to_bgra_conversion() {
        // Gray image (Y=128, Cb=128, Cr=128 should give RGB=128,128,128)
        let y = vec![128u8; 100 * 100];
        let cb = vec![128u8; 50 * 50];
        let cr = vec![128u8; 50 * 50];

        let img_data = ImageData::from_ycbcr420(y, cb, cr, 100, 100, 100, 50);
        let bgra = img_data.to_bgra8(100, 100).unwrap();

        assert_eq!(bgra.len(), 100 * 100 * 4);

        // Check first pixel is gray (B=128, G=128, R=128, A=255)
        assert_eq!(bgra[0], 128); // B
        assert_eq!(bgra[1], 128); // G
        assert_eq!(bgra[2], 128); // R
        assert_eq!(bgra[3], 255); // A
    }
}
