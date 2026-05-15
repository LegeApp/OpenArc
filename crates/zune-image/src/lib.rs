//! zune-image — JPEG decoder with native YCbCr output
//!
//! Wraps upstream zune-jpeg 0.5.12 to provide two decode paths:
//!
//! * **`decode_rgb()`** — standard RGB output for display / thumbnails
//! * **`decode_ycbcr()`** — native YCbCr 4:2:0 planar output, zero
//!   color-space conversion, optimal for feeding directly into
//!   `NativeBPGEncoder::encode_from_ycbcr420_planar()`.
//!
//! The YCbCr path works because upstream zune-jpeg supports
//! `DecoderOptions::jpeg_set_out_colorspace(ColorSpace::YCbCr)` which
//! skips the internal YCbCr→RGB conversion entirely.

pub use zune_core::colorspace;

pub mod decoder;

pub use decoder::{JpegDecoder, YCbCrImage};

/// Convenience: decode JPEG bytes to interleaved RGB.
pub fn decode_jpeg_rgb(data: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let mut dec = JpegDecoder::from_bytes(data.to_vec());
    dec.decode_rgb()
}

/// Convenience: decode JPEG bytes to YCbCr 4:2:0 planar.
pub fn decode_jpeg_ycbcr(data: &[u8]) -> Result<YCbCrImage, String> {
    let mut dec = JpegDecoder::from_bytes(data.to_vec());
    dec.decode_ycbcr()
}
