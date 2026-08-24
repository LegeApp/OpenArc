//! Loading a source image at the highest precision it actually carries.
//!
//! Every image that reaches the JPEG XL encoder goes through here first, and
//! the rule this module exists to enforce is: **nothing is narrowed on the way
//! in**. The old BPG pipeline flattened almost everything to 8-bit RGBA and
//! then encoded it as YCbCr 4:2:0, so a 16-bit TIFF and a phone JPEG arrived at
//! the encoder looking much the same. Here a 16-bit source stays 16-bit, a
//! 10-bit HEIC stays 10-bit, greyscale stays greyscale, and chroma is never
//! subsampled at any point.
//!
//! # Alpha
//!
//! JPEG XL carries alpha as an extra channel, and the JPXL encoder does not
//! implement extra channels yet — `num_extra` is 0 on every stream it writes.
//! Rather than silently discard transparency, [`load`] distinguishes the two
//! cases:
//!
//! * an alpha channel that is **fully opaque** carries no information, so it is
//!   dropped and the image encodes as RGB — lossless with respect to anything
//!   anyone can see;
//! * an alpha channel that is **actually used** sets
//!   [`PreparedSource::has_transparency`], and the caller stores the original
//!   file byte for byte instead of encoding it.
//!
//! That is the honest trade while the encoder lacks the feature: no image
//! silently loses its transparency.

use anyhow::{anyhow, Context, Result};
use image::DynamicImage;
use std::path::Path;

use crate::orchestrator::OriginalImageFormat;

/// Owned interleaved samples at the source's own width.
#[derive(Debug, Clone)]
pub enum OwnedSamples {
    Rgb8(Vec<u8>),
    Rgb16(Vec<u16>),
    Gray8(Vec<u8>),
    Gray16(Vec<u16>),
}

impl OwnedSamples {
    /// A borrowed view for [`codecs::jxl::encode`].
    pub fn as_jxl(&self) -> codecs::jxl::JxlSamples<'_> {
        match self {
            Self::Rgb8(v) => codecs::jxl::JxlSamples::Rgb8(v),
            Self::Rgb16(v) => codecs::jxl::JxlSamples::Rgb16(v),
            Self::Gray8(v) => codecs::jxl::JxlSamples::Gray8(v),
            Self::Gray16(v) => codecs::jxl::JxlSamples::Gray16(v),
        }
    }
}

/// A source image ready for the encoder.
#[derive(Debug, Clone)]
pub struct PreparedSource {
    pub width: u32,
    pub height: u32,
    /// The source's own depth, `1..=16`.
    pub bits_per_sample: u32,
    pub samples: OwnedSamples,
    /// Set when the source had an alpha channel that is not fully opaque. See
    /// the module documentation: such an image must be stored, not encoded.
    pub has_transparency: bool,
}

impl PreparedSource {
    /// The encoder's view of this image.
    pub fn as_jxl_image(&self) -> codecs::jxl::JxlImage<'_> {
        codecs::jxl::JxlImage {
            width: self.width,
            height: self.height,
            bits_per_sample: self.bits_per_sample,
            samples: self.samples.as_jxl(),
        }
    }
}

/// Loads `path` at full precision, choosing the decoder from `format`.
pub fn load(path: &Path, format: OriginalImageFormat) -> Result<PreparedSource> {
    match format {
        OriginalImageFormat::Raw => load_raw(path),
        OriginalImageFormat::Heic => load_heic(path),
        _ => load_generic(path),
    }
}

/// Camera RAW, developed by `raw-autotune` into its native 16-bit RGB.
///
/// The rendered buffer is handed on as-is: no PNG or TIFF intermediate, no
/// narrowing, and no second copy of a buffer that is already ~140 MB at 24 MPix.
fn load_raw(path: &Path) -> Result<PreparedSource> {
    let rendered = raw_autotune::api::render_file_rgb16(
        path,
        &raw_autotune::api::RenderOptions::automatic(),
    )
    .with_context(|| format!("Failed to develop RAW with raw-autotune: {}", path.display()))?;

    // `row_stride` is in samples; a padded stride has to be compacted before
    // the encoder sees it, but the common case is an exact fit and copies
    // nothing.
    let width = rendered.width;
    let height = rendered.height;
    let expected = width as usize * height as usize * 3;
    let data = if rendered.row_stride as usize == width as usize * 3 {
        rendered.data
    } else {
        let stride = rendered.row_stride as usize;
        let mut packed = Vec::with_capacity(expected);
        for row in 0..height as usize {
            let start = row * stride;
            let end = start + width as usize * 3;
            packed.extend_from_slice(
                rendered
                    .data
                    .get(start..end)
                    .ok_or_else(|| anyhow!("developed RAW is short at row {row}"))?,
            );
        }
        packed
    };

    Ok(PreparedSource {
        width,
        height,
        bits_per_sample: 16,
        samples: OwnedSamples::Rgb16(data),
        has_transparency: false,
    })
}

/// HEIC/HEIF, decoded to full-resolution RGB at the source bit depth.
fn load_heic(path: &Path) -> Result<PreparedSource> {
    let mut codec = codecs::heic::HeicCodec::new()?;
    let decoded = codec
        .decode_file_yuv(path)
        .with_context(|| format!("Failed to decode HEIC file: {}", path.display()))?;
    let rgb = decoded
        .to_rgb()
        .with_context(|| format!("Failed to convert HEIC to RGB: {}", path.display()))?;

    let bits = u32::from(rgb.bit_depth);
    let samples = if bits > 8 {
        OwnedSamples::Rgb16(rgb.data)
    } else {
        OwnedSamples::Rgb8(rgb.data.iter().map(|&s| s as u8).collect())
    };

    Ok(PreparedSource {
        width: rgb.width,
        height: rgb.height,
        bits_per_sample: bits,
        samples,
        has_transparency: false,
    })
}

/// Everything the `image` crate (or the JPEG/JP2 readers) can open.
fn load_generic(path: &Path) -> Result<PreparedSource> {
    let decoded = crate::image_loader::load_image(path)
        .with_context(|| format!("Failed to load image: {}", path.display()))?;
    from_dynamic_image(decoded.img)
}

/// Converts a decoded `DynamicImage` to the widest representation that does not
/// invent precision it does not have.
///
/// The match is on the buffer's actual variant rather than on a target format,
/// so an 8-bit PNG is not promoted to 16-bit (which would cost bits and add
/// nothing) and a 16-bit TIFF is not demoted to 8-bit (which would lose the
/// thing worth keeping).
pub fn from_dynamic_image(img: DynamicImage) -> Result<PreparedSource> {
    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Err(anyhow!("image has a zero dimension: {width}x{height}"));
    }

    Ok(match img {
        DynamicImage::ImageLuma8(buf) => PreparedSource {
            width,
            height,
            bits_per_sample: 8,
            samples: OwnedSamples::Gray8(buf.into_raw()),
            has_transparency: false,
        },
        DynamicImage::ImageLuma16(buf) => PreparedSource {
            width,
            height,
            bits_per_sample: 16,
            samples: OwnedSamples::Gray16(buf.into_raw()),
            has_transparency: false,
        },
        DynamicImage::ImageRgb8(buf) => PreparedSource {
            width,
            height,
            bits_per_sample: 8,
            samples: OwnedSamples::Rgb8(buf.into_raw()),
            has_transparency: false,
        },
        DynamicImage::ImageRgb16(buf) => PreparedSource {
            width,
            height,
            bits_per_sample: 16,
            samples: OwnedSamples::Rgb16(buf.into_raw()),
            has_transparency: false,
        },
        DynamicImage::ImageLumaA8(buf) => {
            let raw = buf.into_raw();
            let transparent = raw.chunks_exact(2).any(|px| px.get(1) != Some(&u8::MAX));
            PreparedSource {
                width,
                height,
                bits_per_sample: 8,
                samples: OwnedSamples::Gray8(raw.chunks_exact(2).map(|px| px[0]).collect()),
                has_transparency: transparent,
            }
        }
        DynamicImage::ImageLumaA16(buf) => {
            let raw = buf.into_raw();
            let transparent = raw.chunks_exact(2).any(|px| px.get(1) != Some(&u16::MAX));
            PreparedSource {
                width,
                height,
                bits_per_sample: 16,
                samples: OwnedSamples::Gray16(raw.chunks_exact(2).map(|px| px[0]).collect()),
                has_transparency: transparent,
            }
        }
        DynamicImage::ImageRgba8(buf) => {
            let raw = buf.into_raw();
            let transparent = raw.chunks_exact(4).any(|px| px.get(3) != Some(&u8::MAX));
            PreparedSource {
                width,
                height,
                bits_per_sample: 8,
                samples: OwnedSamples::Rgb8(
                    raw.chunks_exact(4).flat_map(|px| [px[0], px[1], px[2]]).collect(),
                ),
                has_transparency: transparent,
            }
        }
        DynamicImage::ImageRgba16(buf) => {
            let raw = buf.into_raw();
            let transparent = raw.chunks_exact(4).any(|px| px.get(3) != Some(&u16::MAX));
            PreparedSource {
                width,
                height,
                bits_per_sample: 16,
                samples: OwnedSamples::Rgb16(
                    raw.chunks_exact(4).flat_map(|px| [px[0], px[1], px[2]]).collect(),
                ),
                has_transparency: transparent,
            }
        }
        // 32-bit float buffers (OpenEXR, HDR). JPEG XL's integer path tops out
        // at 16 bits, so these are taken at that ceiling rather than at 8.
        other => {
            let buf = other.to_rgb16();
            PreparedSource {
                width,
                height,
                bits_per_sample: 16,
                samples: OwnedSamples::Rgb16(buf.into_raw()),
                has_transparency: false,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Luma, Rgb, Rgba};

    #[test]
    fn an_8_bit_rgb_source_is_not_promoted() {
        let buf = ImageBuffer::<Rgb<u8>, _>::from_raw(2, 2, vec![7u8; 12]).expect("buffer");
        let prepared = from_dynamic_image(DynamicImage::ImageRgb8(buf)).expect("prepared");
        assert_eq!(prepared.bits_per_sample, 8);
        assert!(matches!(prepared.samples, OwnedSamples::Rgb8(_)));
        assert!(!prepared.has_transparency);
    }

    #[test]
    fn a_16_bit_source_keeps_its_precision() {
        let buf = ImageBuffer::<Rgb<u16>, _>::from_raw(2, 2, vec![4_000u16; 12]).expect("buffer");
        let prepared = from_dynamic_image(DynamicImage::ImageRgb16(buf)).expect("prepared");
        assert_eq!(prepared.bits_per_sample, 16);
        match prepared.samples {
            OwnedSamples::Rgb16(v) => assert!(v.iter().all(|&s| s == 4_000)),
            other => panic!("expected 16-bit RGB, got {other:?}"),
        }
    }

    #[test]
    fn greyscale_stays_greyscale() {
        let buf = ImageBuffer::<Luma<u8>, _>::from_raw(2, 2, vec![9u8; 4]).expect("buffer");
        let prepared = from_dynamic_image(DynamicImage::ImageLuma8(buf)).expect("prepared");
        assert!(matches!(prepared.samples, OwnedSamples::Gray8(_)));
    }

    #[test]
    fn a_fully_opaque_alpha_channel_is_dropped_without_flagging_transparency() {
        // Every alpha is 255, so the channel carries nothing.
        let raw: Vec<u8> = (0..4).flat_map(|i| [i as u8, 2, 3, 255]).collect();
        let buf = ImageBuffer::<Rgba<u8>, _>::from_raw(2, 2, raw).expect("buffer");
        let prepared = from_dynamic_image(DynamicImage::ImageRgba8(buf)).expect("prepared");
        assert!(!prepared.has_transparency);
        match prepared.samples {
            OwnedSamples::Rgb8(v) => assert_eq!(v.len(), 12, "alpha dropped, RGB kept"),
            other => panic!("expected 8-bit RGB, got {other:?}"),
        }
    }

    #[test]
    fn a_used_alpha_channel_is_flagged_so_the_caller_can_store_the_original() {
        let raw: Vec<u8> = vec![1, 2, 3, 255, 4, 5, 6, 128, 7, 8, 9, 255, 1, 1, 1, 255];
        let buf = ImageBuffer::<Rgba<u8>, _>::from_raw(2, 2, raw).expect("buffer");
        let prepared = from_dynamic_image(DynamicImage::ImageRgba8(buf)).expect("prepared");
        assert!(
            prepared.has_transparency,
            "a non-opaque alpha must be reported, never silently dropped"
        );
    }
}
