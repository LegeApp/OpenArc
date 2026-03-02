//! YCbCr 4:2:0 resize support using fast_image_resize
//!
//! Resizes YCbCr data by resizing each plane independently.
//! This avoids expensive RGB conversion while maintaining quality.

use fast_image_resize::{Resizer, ResizeAlg, ResizeOptions, PixelType, FilterType};
use fast_image_resize::images::Image;

#[derive(Debug, Clone)]
pub struct YCbCr420Data {
    pub y_plane: Vec<u8>,
    pub cb_plane: Vec<u8>,
    pub cr_plane: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl YCbCr420Data {
    /// Create new YCbCr 4:2:0 data
    pub fn new(
        y_plane: Vec<u8>,
        cb_plane: Vec<u8>,
        cr_plane: Vec<u8>,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            y_plane,
            cb_plane,
            cr_plane,
            width,
            height,
        }
    }

    /// Get total size in bytes
    pub fn total_bytes(&self) -> usize {
        self.y_plane.len() + self.cb_plane.len() + self.cr_plane.len()
    }

    /// Resize to target dimensions, maintaining aspect ratio
    pub fn resize(&self, target_width: u32, target_height: u32) -> Result<Self, String> {
        // Calculate aspect-preserving dimensions
        let src_aspect = self.width as f32 / self.height as f32;
        let target_aspect = target_width as f32 / target_height as f32;

        let (resize_width, resize_height) = if (src_aspect - target_aspect).abs() < 0.01 {
            // Aspect ratios are very similar, use target dimensions directly
            (target_width, target_height)
        } else if src_aspect > target_aspect {
            // Source is wider - fit to width
            let h = (target_width as f32 / src_aspect).round() as u32;
            (target_width, h.max(1))
        } else {
            // Source is taller - fit to height
            let w = (target_height as f32 * src_aspect).round() as u32;
            (w.max(1), target_height)
        };

        self.resize_exact(resize_width, resize_height)
    }

    /// Resize to exact dimensions (no aspect ratio preservation)
    pub fn resize_exact(&self, target_width: u32, target_height: u32) -> Result<Self, String> {
        let mut resizer = Resizer::new();
        let opts = ResizeOptions::new()
            .resize_alg(ResizeAlg::Convolution(FilterType::Bilinear));

        // Resize Y plane (full resolution)
        let y_dst = resize_plane(
            &self.y_plane,
            self.width,
            self.height,
            target_width,
            target_height,
            &mut resizer,
            &opts,
        )?;

        // Calculate chroma dimensions for 4:2:0
        let src_cw = (self.width + 1) / 2;
        let src_ch = (self.height + 1) / 2;
        let dst_cw = (target_width + 1) / 2;
        let dst_ch = (target_height + 1) / 2;

        // Resize chroma planes (half resolution)
        let cb_dst = resize_plane(
            &self.cb_plane,
            src_cw,
            src_ch,
            dst_cw,
            dst_ch,
            &mut resizer,
            &opts,
        )?;

        let cr_dst = resize_plane(
            &self.cr_plane,
            src_cw,
            src_ch,
            dst_cw,
            dst_ch,
            &mut resizer,
            &opts,
        )?;

        Ok(YCbCr420Data {
            y_plane: y_dst,
            cb_plane: cb_dst,
            cr_plane: cr_dst,
            width: target_width,
            height: target_height,
        })
    }
}

/// Resize a single plane using fast_image_resize
fn resize_plane(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    resizer: &mut Resizer,
    opts: &ResizeOptions,
) -> Result<Vec<u8>, String> {
    // FIR requires mutable source data
    let mut src_clone = src.to_vec();

    let src_img = Image::from_slice_u8(src_w, src_h, &mut src_clone, PixelType::U8)
        .map_err(|e| format!("Failed to create source image: {:?}", e))?;

    let mut dst = vec![0u8; (dst_w * dst_h) as usize];
    let mut dst_img = Image::from_slice_u8(dst_w, dst_h, &mut dst, PixelType::U8)
        .map_err(|e| format!("Failed to create dest image: {:?}", e))?;

    resizer
        .resize(&src_img, &mut dst_img, Some(opts))
        .map_err(|e| format!("Resize failed: {:?}", e))?;

    Ok(dst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ycbcr_resize() {
        // Create a simple 4x4 YCbCr image
        let y = vec![128u8; 16];
        let cb = vec![128u8; 4]; // 2x2 for 4:2:0
        let cr = vec![128u8; 4];

        let ycbcr = YCbCr420Data::new(y, cb, cr, 4, 4);

        // Resize to 2x2
        let resized = ycbcr.resize_exact(2, 2).unwrap();

        assert_eq!(resized.width, 2);
        assert_eq!(resized.height, 2);
        assert_eq!(resized.y_plane.len(), 4);
        assert_eq!(resized.cb_plane.len(), 1); // 1x1 for 2x2 image
        assert_eq!(resized.cr_plane.len(), 1);
    }
}
