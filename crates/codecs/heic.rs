// HEIC/HEIF decoding via bpg-decode (bpg-rs).
// The archival path keeps decoded YUV planes in their original bit depth so the
// BPG encoder can preserve HEIC fidelity instead of forcing an 8-bit RGB path.

use std::path::Path;

use anyhow::{anyhow, ensure, Context, Result};
use bpg_decode::heic::{
    decode_heic, decode_heic_thumbnail, decode_heic_to_frame, get_heic_image_info, DecodedFrame,
};
use bpg_decode::{DecodeOutput, PixelLayout};

// Compression format for encoding (kept for API compatibility)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeifCompressionFormat {
    Undefined = 0,
    HEVC = 1,
    AVC = 2,
    JPEG = 3,
    AV1 = 4,
}

/// Decoded HEIC image data (RGB/RGBA format)
#[derive(Debug)]
pub struct DecodedHeicImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub has_alpha: bool,
}

/// HEIC chroma subsampling format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeicChromaFormat {
    Monochrome = 0,
    YCbCr420 = 1,
    YCbCr422 = 2,
    YCbCr444 = 3,
}

impl HeicChromaFormat {
    pub fn from_decoder_value(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Monochrome),
            1 => Ok(Self::YCbCr420),
            2 => Ok(Self::YCbCr422),
            3 => Ok(Self::YCbCr444),
            _ => Err(anyhow!("Unsupported HEIC chroma format: {value}")),
        }
    }

    pub const fn horizontal_divisor(self) -> u32 {
        match self {
            Self::Monochrome | Self::YCbCr444 => 1,
            Self::YCbCr420 | Self::YCbCr422 => 2,
        }
    }

    pub const fn vertical_divisor(self) -> u32 {
        match self {
            Self::YCbCr420 => 2,
            Self::Monochrome | Self::YCbCr422 | Self::YCbCr444 => 1,
        }
    }

    pub fn plane_dimensions(self, width: u32, height: u32) -> (u32, u32) {
        match self {
            Self::Monochrome => (0, 0),
            Self::YCbCr420 => (width.div_ceil(2), height.div_ceil(2)),
            Self::YCbCr422 => (width.div_ceil(2), height),
            Self::YCbCr444 => (width, height),
        }
    }

    pub const fn to_bpg_chroma_format(self) -> i32 {
        match self {
            Self::Monochrome => 0,
            Self::YCbCr420 => 1,
            Self::YCbCr422 => 2,
            Self::YCbCr444 => 3,
        }
    }
}

/// Decoded HEIC image in planar YUV form, preserving source bit depth.
#[derive(Debug)]
pub struct DecodedHeicYuv {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub chroma_format: HeicChromaFormat,
    pub full_range: bool,
    pub matrix_coeffs: u8,
    pub y_plane: Vec<u16>,
    pub cb_plane: Vec<u16>,
    pub cr_plane: Vec<u16>,
    pub y_stride: u32,
    pub cb_stride: u32,
    pub cr_stride: u32,
    pub alpha_plane: Option<Vec<u16>>,
    pub alpha_stride: Option<u32>,
}

/// Lightweight HEIC header info read without decoding pixel data.
///
/// Backed by `bpg_decode::heic::get_heic_image_info`, which parses the
/// ISOBMFF/HEIF container (`ispe`/`hvcC` boxes) only. Useful for sizing work
/// (e.g. memory reservation) before committing to a full decode — the `image`
/// crate cannot read HEIC, so this is the way to learn real dimensions up front.
#[derive(Debug, Clone, Copy)]
pub struct HeicInfo {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub chroma_format: HeicChromaFormat,
    pub has_alpha: bool,
}

/// HEIC encoder configuration (kept for API compatibility)
#[derive(Debug, Clone)]
pub struct HeicEncoderConfig {
    pub quality: u8,
    pub lossless: bool,
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

/// HEIC codec backed by bpg-decode (pure Rust HEVC decoder from bpg-rs).
pub struct HeicCodec;

impl HeicCodec {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub fn is_available() -> bool {
        true
    }

    pub fn get_version() -> Option<String> {
        Some("bpg-decode (pure Rust HEVC decoder, HEIF container parser)".to_string())
    }

    /// Read HEIC dimensions and basic properties from the container headers
    /// without decoding pixel data.
    pub fn read_info(path: &Path) -> Result<HeicInfo> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read HEIC file: {}", path.display()))?;
        Self::read_info_from_memory(&data)
    }

    /// Like [`read_info`](Self::read_info) but from an in-memory buffer.
    pub fn read_info_from_memory(buffer: &[u8]) -> Result<HeicInfo> {
        let info = get_heic_image_info(buffer)
            .map_err(|e| anyhow!("Failed to read HEIC image info: {e:?}"))?;
        Ok(HeicInfo {
            width: info.width,
            height: info.height,
            bit_depth: info.bit_depth,
            chroma_format: HeicChromaFormat::from_decoder_value(info.chroma_format)?,
            has_alpha: info.has_alpha,
        })
    }

    pub fn decode_file(&mut self, path: &Path) -> Result<DecodedHeicImage> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read HEIC file: {}", path.display()))?;
        self.decode_from_memory(&data)
    }

    pub fn decode_from_memory(&mut self, buffer: &[u8]) -> Result<DecodedHeicImage> {
        let info = get_heic_image_info(buffer)
            .map_err(|err| anyhow!("Failed to inspect HEIC image metadata: {err}"))?;
        let layout = pixel_layout_for_alpha(info.has_alpha);
        let decoded = decode_heic(buffer, layout)
            .map_err(|err| anyhow!("Failed to decode HEIC from memory: {err}"))?;
        Ok(decoded_output_to_image(decoded, info.has_alpha))
    }

    /// Decode a thumbnail/display-friendly HEIC image.
    ///
    /// This path prefers the embedded HEIF thumbnail when present, then falls back
    /// to a normal RGB decode and downsizes to the requested bounds. It is meant
    /// for GUI preview/thumbnail work, not archival preservation.
    pub fn decode_file_preview(
        &mut self,
        path: &Path,
        max_width: u32,
        max_height: u32,
    ) -> Result<DecodedHeicImage> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read HEIC file: {}", path.display()))?;
        self.decode_preview_from_memory(&data, max_width, max_height)
    }

    pub fn decode_preview_from_memory(
        &mut self,
        buffer: &[u8],
        max_width: u32,
        max_height: u32,
    ) -> Result<DecodedHeicImage> {
        let info = get_heic_image_info(buffer)
            .map_err(|err| anyhow!("Failed to inspect HEIC image metadata: {err}"))?;
        let layout = pixel_layout_for_alpha(info.has_alpha);

        let preview = if info.has_thumbnail {
            decode_heic_thumbnail(buffer, layout)
                .map_err(|err| anyhow!("Failed to decode HEIC thumbnail: {err}"))?
        } else {
            None
        };

        let decoded = match preview {
            Some(decoded) => decoded_output_to_image(decoded, info.has_alpha),
            None => {
                let decoded = decode_heic(buffer, layout)
                    .map_err(|err| anyhow!("Failed to decode HEIC from memory: {err}"))?;
                decoded_output_to_image(decoded, info.has_alpha)
            }
        };

        resize_decoded_image(decoded, max_width, max_height)
    }

    /// Decode a HEIC/HEIF file to planar YUV while preserving source bit depth,
    /// chroma subsampling, alpha, and range/matrix metadata.
    pub fn decode_file_yuv(&mut self, path: &Path) -> Result<DecodedHeicYuv> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read HEIC file: {}", path.display()))?;
        self.decode_yuv_from_memory(&data)
    }

    pub fn decode_yuv_from_memory(&mut self, buffer: &[u8]) -> Result<DecodedHeicYuv> {
        let frame = decode_heic_to_frame(buffer)
            .map_err(|err| anyhow!("Failed to decode HEIC to YUV frame: {err}"))?;
        Self::convert_frame(frame)
    }

    /// Compatibility wrapper for older call sites that assumed HEIC always meant 4:2:0.
    pub fn decode_file_ycbcr420(&mut self, path: &Path) -> Result<DecodedHeicYuv> {
        let decoded = self.decode_file_yuv(path)?;
        ensure!(
            decoded.chroma_format == HeicChromaFormat::YCbCr420,
            "Expected 4:2:0 HEIC but decoded {:?}",
            decoded.chroma_format
        );
        Ok(decoded)
    }

    pub fn decode_to_png(&mut self, input_path: &Path, output_path: &Path) -> Result<()> {
        let decoded = self.decode_file(input_path)?;

        use image::{DynamicImage, ImageBuffer, Rgb, Rgba};

        let img = if decoded.has_alpha {
            let rgba_buf = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                decoded.width,
                decoded.height,
                decoded.data,
            )
            .ok_or_else(|| anyhow!("Failed to create RGBA image buffer"))?;
            DynamicImage::ImageRgba8(rgba_buf)
        } else {
            let rgb_buf = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(
                decoded.width,
                decoded.height,
                decoded.data,
            )
            .ok_or_else(|| anyhow!("Failed to create RGB image buffer"))?;
            DynamicImage::ImageRgb8(rgb_buf)
        };

        img.save(output_path)?;
        Ok(())
    }

    pub fn decode_to_jpeg(
        &mut self,
        input_path: &Path,
        output_path: &Path,
        quality: u8,
    ) -> Result<()> {
        let decoded = self.decode_file(input_path)?;

        let dynamic_img = if decoded.has_alpha {
            image::DynamicImage::ImageRgba8(
                image::ImageBuffer::from_raw(decoded.width, decoded.height, decoded.data)
                    .ok_or_else(|| anyhow!("Invalid buffer size"))?,
            )
        } else {
            image::DynamicImage::ImageRgb8(
                image::ImageBuffer::from_raw(decoded.width, decoded.height, decoded.data)
                    .ok_or_else(|| anyhow!("Invalid buffer size"))?,
            )
        };

        let rgb_img = dynamic_img.into_rgb8();
        let mut output_file = std::fs::File::create(output_path)?;
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output_file, quality);
        rgb_img.write_with_encoder(encoder)?;
        Ok(())
    }

    pub fn png_to_heic(
        &mut self,
        _input_path: &Path,
        _output_path: &Path,
        _config: &HeicEncoderConfig,
    ) -> Result<()> {
        Err(anyhow!("HEIC encoding not supported - decoding only"))
    }

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

    fn convert_frame(frame: DecodedFrame) -> Result<DecodedHeicYuv> {
        let chroma_format = HeicChromaFormat::from_decoder_value(frame.chroma_format)?;
        let width = frame.cropped_width();
        let height = frame.cropped_height();

        let y_plane = crop_plane_u16(
            &frame.y_plane,
            frame.y_stride(),
            frame.crop_left,
            frame.crop_top,
            width,
            height,
        );

        let (cb_plane, cr_plane, cb_stride, cr_stride) =
            if chroma_format == HeicChromaFormat::Monochrome {
                (Vec::new(), Vec::new(), 0, 0)
            } else {
                let crop_left_c = frame.crop_left / chroma_format.horizontal_divisor();
                let crop_right_c = frame.crop_right / chroma_format.horizontal_divisor();
                let crop_top_c = frame.crop_top / chroma_format.vertical_divisor();
                let crop_bottom_c = frame.crop_bottom / chroma_format.vertical_divisor();
                let (full_c_width, full_c_height) =
                    chroma_format.plane_dimensions(frame.width, frame.height);
                let crop_width_c = full_c_width - crop_left_c - crop_right_c;
                let crop_height_c = full_c_height - crop_top_c - crop_bottom_c;
                let c_stride = frame.c_stride();

                (
                    crop_plane_u16(
                        &frame.cb_plane,
                        c_stride,
                        crop_left_c,
                        crop_top_c,
                        crop_width_c,
                        crop_height_c,
                    ),
                    crop_plane_u16(
                        &frame.cr_plane,
                        c_stride,
                        crop_left_c,
                        crop_top_c,
                        crop_width_c,
                        crop_height_c,
                    ),
                    crop_width_c,
                    crop_width_c,
                )
            };

        let alpha_plane = frame.alpha_plane.as_ref().map(|alpha| {
            crop_plane_u16(
                alpha,
                frame.y_stride(),
                frame.crop_left,
                frame.crop_top,
                width,
                height,
            )
        });

        let alpha_stride = alpha_plane.as_ref().map(|_| width);

        Ok(DecodedHeicYuv {
            width,
            height,
            bit_depth: frame.bit_depth,
            chroma_format,
            full_range: frame.full_range,
            matrix_coeffs: frame.matrix_coeffs,
            y_plane,
            cb_plane,
            cr_plane,
            y_stride: width,
            cb_stride,
            cr_stride,
            alpha_plane,
            alpha_stride,
        })
    }
}

fn crop_plane_u16(
    src: &[u16],
    src_stride: usize,
    crop_left: u32,
    crop_top: u32,
    width: u32,
    height: u32,
) -> Vec<u16> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let crop_left = crop_left as usize;
    let crop_top = crop_top as usize;
    let width = width as usize;
    let height = height as usize;
    let mut out = Vec::with_capacity(width * height);

    for row in 0..height {
        let start = (crop_top + row) * src_stride + crop_left;
        let end = start + width;
        out.extend_from_slice(&src[start..end]);
    }

    out
}

fn pixel_layout_for_alpha(has_alpha: bool) -> PixelLayout {
    if has_alpha {
        PixelLayout::Rgba8
    } else {
        PixelLayout::Rgb8
    }
}

fn decoded_output_to_image(decoded: DecodeOutput, has_alpha: bool) -> DecodedHeicImage {
    DecodedHeicImage {
        width: decoded.width,
        height: decoded.height,
        data: decoded.data,
        has_alpha,
    }
}

fn resize_decoded_image(
    decoded: DecodedHeicImage,
    max_width: u32,
    max_height: u32,
) -> Result<DecodedHeicImage> {
    let max_width = max_width.max(1);
    let max_height = max_height.max(1);

    if decoded.width <= max_width && decoded.height <= max_height {
        return Ok(decoded);
    }

    let scale_x = max_width as f32 / decoded.width as f32;
    let scale_y = max_height as f32 / decoded.height as f32;
    let scale = scale_x.min(scale_y).min(1.0);
    let new_width = ((decoded.width as f32) * scale).round().max(1.0) as u32;
    let new_height = ((decoded.height as f32) * scale).round().max(1.0) as u32;

    use image::{imageops::FilterType, DynamicImage, ImageBuffer, Rgb, Rgba};

    let resized = if decoded.has_alpha {
        let rgba =
            ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(decoded.width, decoded.height, decoded.data)
                .ok_or_else(|| anyhow!("Failed to create RGBA preview buffer"))?;
        DynamicImage::ImageRgba8(rgba).resize(new_width, new_height, FilterType::Triangle)
    } else {
        let rgb =
            ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(decoded.width, decoded.height, decoded.data)
                .ok_or_else(|| anyhow!("Failed to create RGB preview buffer"))?;
        DynamicImage::ImageRgb8(rgb).resize(new_width, new_height, FilterType::Triangle)
    };

    let data = if decoded.has_alpha {
        resized.to_rgba8().into_raw()
    } else {
        resized.to_rgb8().into_raw()
    };

    Ok(DecodedHeicImage {
        width: new_width,
        height: new_height,
        data,
        has_alpha: decoded.has_alpha,
    })
}

/// Map HEVC matrix coefficients to the BPG encoder's color-space enum.
pub fn matrix_coeffs_to_bpg_color_space(matrix_coeffs: u8) -> i32 {
    match matrix_coeffs {
        1 => 3,     // BT.709
        5 | 6 => 0, // BT.601
        9 => 4,     // BT.2020
        _ => 3,     // Unspecified is most commonly camera-style BT.709 in practice
    }
}

unsafe impl Send for HeicCodec {}

pub type HeicDecoder = HeicCodec;

pub fn is_heic_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let lower = e.to_lowercase();
            lower == "heic" || lower == "heif" || lower == "hif"
        })
        .unwrap_or(false)
}

pub fn decode_heic_file(path: &Path) -> Result<DecodedHeicImage> {
    let mut codec = HeicCodec::new()?;
    codec.decode_file(path)
}

pub fn heic_to_png(input: &Path, output: &Path) -> Result<()> {
    let mut codec = HeicCodec::new()?;
    codec.decode_to_png(input, output)
}

pub fn heic_to_jpeg(input: &Path, output: &Path, quality: u8) -> Result<()> {
    let mut codec = HeicCodec::new()?;
    codec.decode_to_jpeg(input, output, quality)
}

pub fn png_to_heic(_input: &Path, _output: &Path, _quality: u8) -> Result<()> {
    Err(anyhow!("HEIC encoding not supported - decoding only"))
}

pub fn png_to_heic_lossless(_input: &Path, _output: &Path) -> Result<()> {
    Err(anyhow!("HEIC encoding not supported - decoding only"))
}


/// The interleaved full-resolution RGB form of a decoded HEIC image, at the
/// source's own bit depth.
#[derive(Debug, Clone)]
pub struct HeicRgb {
    pub width: u32,
    pub height: u32,
    /// Source bit depth, 8..=16. Samples occupy the low `bit_depth` bits.
    pub bit_depth: u8,
    /// Interleaved RGB, three `u16` per pixel.
    pub data: Vec<u16>,
}

impl DecodedHeicYuv {
    /// Converts to full-resolution interleaved RGB at the source bit depth.
    ///
    /// # Why this exists
    ///
    /// The BPG path used to hand these YCbCr planes to the encoder untouched,
    /// subsampling and all. JPEG XL has no YCbCr path — a VarDCT frame is XYB —
    /// so the conversion has to happen somewhere, and doing it here at the
    /// source's own precision is the only version of it that does not lose
    /// anything the file actually contained:
    ///
    /// * **Chroma is upsampled by replication**, not interpolated. Replication
    ///   reproduces exactly the samples the file stores at the positions it
    ///   stores them and invents nothing in between; an interpolating filter
    ///   would look smoother but would be this decoder guessing. The subsequent
    ///   JPEG XL encode is 4:4:4, so nothing re-subsamples afterwards.
    /// * **Depth is preserved**: a 10-bit HEIC stays 10-bit here, and the
    ///   caller declares that depth to the encoder.
    /// * **Range and matrix are honoured** from the stream's own signalling
    ///   rather than assumed to be full-range BT.709.
    pub fn to_rgb(&self) -> Result<HeicRgb> {
        let (width, height) = (self.width, self.height);
        ensure!(width > 0 && height > 0, "HEIC image has a zero dimension");
        let bit_depth = self.bit_depth;
        ensure!(
            (8..=16).contains(&bit_depth),
            "unsupported HEIC bit depth: {bit_depth}"
        );

        let max = ((1u32 << bit_depth) - 1) as f32;
        // Limited ("TV") range packs luma into 16..235 and chroma into 16..240,
        // scaled to the bit depth; full range uses the whole interval.
        let scale = f32::from(1u16 << (bit_depth - 8));
        let (y_offset, y_range, c_range) = if self.full_range {
            (0.0f32, max, max)
        } else {
            (16.0 * scale, 219.0 * scale, 224.0 * scale)
        };
        let c_centre = f32::from(1u16 << (bit_depth - 1));

        // Table E.5 matrix coefficients -> luma weights (Kr, Kb).
        let (kr, kb) = match self.matrix_coeffs {
            // Identity: the "YCbCr" planes are really G, B, R.
            0 => (0.0, 0.0),
            1 => (0.2126, 0.0722),   // BT.709
            9 | 10 => (0.2627, 0.0593), // BT.2020 NCL / CL
            // 5/6 are BT.601; 2 is "unspecified", for which BT.601 is the
            // conventional fallback and what the BPG mapping already assumed.
            _ => (0.299, 0.114),
        };
        let kg = 1.0 - kr - kb;

        let pixels = width as usize * height as usize;
        let mut out = vec![0u16; pixels * 3];
        let (cw, _ch) = self.chroma_format.plane_dimensions(width, height);
        let hdiv = self.chroma_format.horizontal_divisor();
        let vdiv = self.chroma_format.vertical_divisor();
        let monochrome = self.chroma_format == HeicChromaFormat::Monochrome;
        let identity = self.matrix_coeffs == 0;

        for y in 0..height {
            for x in 0..width {
                let y_index = (y * self.y_stride + x) as usize;
                let luma = *self
                    .y_plane
                    .get(y_index)
                    .ok_or_else(|| anyhow!("HEIC luma plane is short at ({x}, {y})"))?;

                let (cb, cr) = if monochrome {
                    (c_centre, c_centre)
                } else {
                    // Replicating upsample: the chroma sample covering this
                    // pixel is the one at (x / hdiv, y / vdiv).
                    let cx = (x / hdiv).min(cw.saturating_sub(1));
                    let cy = y / vdiv;
                    let cb_index = (cy * self.cb_stride + cx) as usize;
                    let cr_index = (cy * self.cr_stride + cx) as usize;
                    (
                        f32::from(self.cb_plane.get(cb_index).copied().unwrap_or(0)),
                        f32::from(self.cr_plane.get(cr_index).copied().unwrap_or(0)),
                    )
                };

                let (r, g, b) = if identity {
                    // GBR order, no matrix, no range scaling.
                    (cr, f32::from(luma), cb)
                } else {
                    let yn = (f32::from(luma) - y_offset) / y_range;
                    let cbn = (cb - c_centre) / c_range;
                    let crn = (cr - c_centre) / c_range;
                    let r = yn + 2.0 * (1.0 - kr) * crn;
                    let b = yn + 2.0 * (1.0 - kb) * cbn;
                    let g = yn - (2.0 * (1.0 - kr) * kr / kg) * crn
                        - (2.0 * (1.0 - kb) * kb / kg) * cbn;
                    (r * max, g * max, b * max)
                };

                let base = (y as usize * width as usize + x as usize) * 3;
                for (offset, value) in [r, g, b].into_iter().enumerate() {
                    if let Some(slot) = out.get_mut(base + offset) {
                        *slot = value.round().clamp(0.0, max) as u16;
                    }
                }
            }
        }

        Ok(HeicRgb {
            width,
            height,
            bit_depth,
            data: out,
        })
    }
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
        assert!(
            HeicCodec::is_available(),
            "Pure Rust decoder should always be available"
        );
        assert!(HeicCodec::get_version().is_some());
    }

    #[test]
    fn test_bpg_color_space_mapping() {
        assert_eq!(matrix_coeffs_to_bpg_color_space(1), 3);
        assert_eq!(matrix_coeffs_to_bpg_color_space(5), 0);
        assert_eq!(matrix_coeffs_to_bpg_color_space(9), 4);
        assert_eq!(matrix_coeffs_to_bpg_color_space(2), 3);
    }
}
