//! JPEG Decoder with YCbCr exposure
//!
//! Wraps upstream zune-jpeg to expose raw YCbCr output via
//! `jpeg_set_out_colorspace(ColorSpace::YCbCr)`, avoiding the
//! automatic YCbCr→RGB conversion.

use std::io::Read;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

/// Decoded YCbCr image (planar format, ready for BPG encoding)
#[derive(Debug, Clone)]
pub struct YCbCrImage {
    pub width: u32,
    pub height: u32,
    /// Full-resolution luma plane
    pub y_plane: Vec<u8>,
    /// Half-resolution chroma-blue plane (4:2:0 subsampled)
    pub cb_plane: Vec<u8>,
    /// Half-resolution chroma-red plane (4:2:0 subsampled)
    pub cr_plane: Vec<u8>,
    /// Stride of Y plane (= width, no padding)
    pub y_stride: u32,
    /// Stride of Cb plane (= (width+1)/2, no padding)
    pub cb_stride: u32,
    /// Stride of Cr plane (= (width+1)/2, no padding)
    pub cr_stride: u32,
}

/// JPEG Decoder — wraps upstream zune-jpeg
///
/// Reads all data up-front, then decodes to RGB or YCbCr on demand.
pub struct JpegDecoder {
    data: Vec<u8>,
}

impl JpegDecoder {
    /// Create a new decoder by reading all bytes from `reader`.
    pub fn new<R: Read>(mut reader: R) -> Result<Self, String> {
        let mut data = Vec::new();
        reader
            .read_to_end(&mut data)
            .map_err(|e| format!("Failed to read JPEG data: {}", e))?;
        Ok(Self { data })
    }

    /// Create a new decoder from an already-loaded byte buffer.
    pub fn from_bytes(data: Vec<u8>) -> Self {
        Self { data }
    }

    // ── helpers ───────────────────────────────────────────────────

    /// Decode with the given options into a pixel buffer + dimensions.
    fn decode_with_options(
        &self,
        options: DecoderOptions,
    ) -> Result<(Vec<u8>, usize, usize, ColorSpace), String> {
        use std::io::Cursor;
        use zune_jpeg::JpegDecoder as Upstream;

        let mut dec = Upstream::new_with_options(Cursor::new(&self.data), options);

        let pixels = dec
            .decode()
            .map_err(|e| format!("JPEG decode error: {:?}", e))?;

        let (w, h) = dec
            .dimensions()
            .ok_or_else(|| "Failed to get JPEG dimensions".to_string())?;

        let cs = dec
            .output_colorspace()
            .ok_or_else(|| "Failed to get JPEG output colorspace".to_string())?;

        Ok((pixels, w, h, cs))
    }

    // ── public API ───────────────────────────────────────────────

    /// Decode to interleaved RGB (for display / thumbnails).
    ///
    /// Grayscale JPEGs are expanded to 3-channel RGB.
    pub fn decode_rgb(&mut self) -> Result<(Vec<u8>, u32, u32), String> {
        // Default options → RGB output
        let opts = DecoderOptions::default();
        let (pixels, w, h, cs) = self.decode_with_options(opts)?;

        match cs {
            ColorSpace::RGB => Ok((pixels, w as u32, h as u32)),
            ColorSpace::Luma => {
                // Expand grayscale → RGB
                let rgb: Vec<u8> = pixels
                    .iter()
                    .flat_map(|&g| [g, g, g])
                    .collect();
                Ok((rgb, w as u32, h as u32))
            }
            other => Err(format!("Unexpected output colorspace: {:?}", other)),
        }
    }

    /// Decode to YCbCr 4:2:0 planar — optimal for BPG encoding.
    ///
    /// Uses `jpeg_set_out_colorspace(YCbCr)` so the upstream decoder
    /// skips the YCbCr→RGB conversion entirely. The interleaved output
    /// is then de-interleaved and subsampled to 4:2:0 planes that can
    /// be passed directly to `NativeBPGEncoder::encode_from_ycbcr420_planar()`.
    ///
    /// For grayscale JPEGs the Cb/Cr planes are filled with 128 (neutral).
    pub fn decode_ycbcr(&mut self) -> Result<YCbCrImage, String> {
        let opts = DecoderOptions::default()
            .jpeg_set_out_colorspace(ColorSpace::YCbCr);

        let (pixels, w, h, cs) = self.decode_with_options(opts)?;

        let width = w as u32;
        let height = h as u32;
        let chroma_w = (width + 1) / 2;
        let chroma_h = (height + 1) / 2;
        let pixel_count = (width * height) as usize;
        let chroma_count = (chroma_w * chroma_h) as usize;

        match cs {
            ColorSpace::YCbCr => {
                // pixels is interleaved [Y,Cb,Cr, Y,Cb,Cr, …]
                // De-interleave into full-res Y, full-res Cb, full-res Cr,
                // then subsample chroma to 4:2:0.
                let mut y_plane = Vec::with_capacity(pixel_count);
                let mut cb_full = Vec::with_capacity(pixel_count);
                let mut cr_full = Vec::with_capacity(pixel_count);

                for chunk in pixels.chunks_exact(3) {
                    y_plane.push(chunk[0]);
                    cb_full.push(chunk[1]);
                    cr_full.push(chunk[2]);
                }

                // Subsample chroma 4:2:0 via 2×2 box averaging
                let mut cb_plane = Vec::with_capacity(chroma_count);
                let mut cr_plane = Vec::with_capacity(chroma_count);

                for cy in 0..chroma_h {
                    for cx in 0..chroma_w {
                        let y0 = (cy * 2) as usize;
                        let x0 = (cx * 2) as usize;
                        let w = width as usize;

                        // Gather up to 4 samples (handle odd dimensions)
                        let mut cb_sum: u32 = 0;
                        let mut cr_sum: u32 = 0;
                        let mut count: u32 = 0;

                        for dy in 0..2u32 {
                            let y = y0 + dy as usize;
                            if y >= height as usize {
                                break;
                            }
                            for dx in 0..2u32 {
                                let x = x0 + dx as usize;
                                if x >= w {
                                    break;
                                }
                                let idx = y * w + x;
                                cb_sum += cb_full[idx] as u32;
                                cr_sum += cr_full[idx] as u32;
                                count += 1;
                            }
                        }

                        cb_plane.push((cb_sum / count) as u8);
                        cr_plane.push((cr_sum / count) as u8);
                    }
                }

                Ok(YCbCrImage {
                    width,
                    height,
                    y_plane,
                    cb_plane,
                    cr_plane,
                    y_stride: width,
                    cb_stride: chroma_w,
                    cr_stride: chroma_w,
                })
            }
            ColorSpace::Luma => {
                // Grayscale — fabricate neutral chroma
                let y_plane = pixels;
                let cb_plane = vec![128u8; chroma_count];
                let cr_plane = vec![128u8; chroma_count];

                Ok(YCbCrImage {
                    width,
                    height,
                    y_plane,
                    cb_plane,
                    cr_plane,
                    y_stride: width,
                    cb_stride: chroma_w,
                    cr_stride: chroma_w,
                })
            }
            other => Err(format!("Unexpected colorspace from YCbCr decode: {:?}", other)),
        }
    }
}

// Re-export useful upstream types
pub use zune_jpeg::JpegDecoder as UpstreamJpegDecoder;
