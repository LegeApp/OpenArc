//! A Rust port of the TooJpeg JPEG encoder with performance optimizations.
//!
//! This library provides a simple interface for encoding RGB(A) images to JPEG format
//! with various quality and optimization settings.

#![allow(missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod toojpeg;

pub use toojpeg::{write_jpeg, write_jpeg_ycbcr420, BitCode, BitWriter, I16, I32, U16, U8};

/// Image format options for the JPEG encoder
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// RGB format (3 bytes per pixel)
    RGB,
    /// RGBA format (4 bytes per pixel, alpha is ignored)
    RGBA,
    /// Grayscale format (1 byte per pixel)
    Gray,
    /// YCbCr 4:2:0 format (planar: Y plane + Cb plane + Cr plane)
    YCbCr420,
}

/// JPEG encoding options
#[derive(Debug, Clone, Copy)]
pub struct EncodeOptions {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Image format (RGB, RGBA, or Grayscale)
    pub format: ImageFormat,
    /// Quality from 1 (worst) to 100 (best)
    pub quality: u8,
    /// Whether to use baseline DCT encoding (true) or progressive (false)
    pub baseline: bool,
    /// Whether to use optimized Huffman tables
    pub optimized: bool,
    /// Whether to downsample chroma channels (4:2:0 subsampling)
    pub downsample: bool,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            format: ImageFormat::RGB,
            quality: 90,
            baseline: true,
            optimized: true,
            downsample: true,
        }
    }
}

/// Encode an image to JPEG format
///
/// # Arguments
/// * `pixels` - The image pixel data in the format specified by `options.format`
/// * `options` - Encoding options including dimensions, format, and quality
/// * `output` - A writer that implements `std::io::Write` to receive the JPEG data
///
/// # Returns
/// `Result<(), &'static str>` indicating success or an error message
pub fn encode_jpeg<W: std::io::Write>(
    pixels: &[u8],
    options: EncodeOptions,
    output: &mut W,
) -> Result<(), &'static str> {
    // Special path for YCbCr 4:2:0 input
    if matches!(options.format, ImageFormat::YCbCr420) {
        return encode_ycbcr420(pixels, options, output);
    }

    // Input validation for RGB/RGBA/Gray
    let bytes_per_pixel = match options.format {
        ImageFormat::RGB => 3,
        ImageFormat::RGBA => 4,
        ImageFormat::Gray => 1,
        ImageFormat::YCbCr420 => unreachable!(), // handled above
    };

    let expected_len = (options.width as usize)
        .checked_mul(options.height as usize)
        .and_then(|x| x.checked_mul(bytes_per_pixel));

    if expected_len.map_or(true, |len| pixels.len() < len) {
        return Err("Input buffer too small for specified dimensions and format");
    }

    // Convert to the format expected by write_jpeg
    let is_rgb = matches!(options.format, ImageFormat::RGB | ImageFormat::RGBA);
    let quality = options.quality.clamp(1, 100) as u8;

    // Create a BitWriter for the output
    let mut writer = BitWriter::new(|byte| {
        output
            .write_all(&[byte])
            .map_err(|_| "Failed to write output")
    });

    // Call the low-level write_jpeg function
    write_jpeg(
        &mut writer,
        pixels,
        options.width as u16,
        options.height as u16,
        is_rgb,
        quality,
        options.downsample,
        None, // comment
    )
}

/// Encode YCbCr 4:2:0 planar data directly to JPEG (no RGB conversion!)
///
/// # Buffer Layout
/// The input buffer should contain three planes concatenated:
/// - Y plane: width × height bytes
/// - Cb plane: (width/2) × (height/2) bytes
/// - Cr plane: (width/2) × (height/2) bytes
///
/// # Arguments
/// * `ycbcr` - YCbCr 4:2:0 planar data (Y + Cb + Cr planes)
/// * `options` - Encoding options (format field is ignored)
/// * `output` - Output writer
pub fn encode_ycbcr420<W: std::io::Write>(
    ycbcr: &[u8],
    options: EncodeOptions,
    output: &mut W,
) -> Result<(), &'static str> {
    let width = options.width as usize;
    let height = options.height as usize;

    // Calculate expected buffer size for 4:2:0
    let y_size = width * height;
    let c_width = (width + 1) / 2;
    let c_height = (height + 1) / 2;
    let c_size = c_width * c_height;
    let expected_len = y_size + 2 * c_size;

    if ycbcr.len() < expected_len {
        return Err("YCbCr buffer too small for 4:2:0 format");
    }

    // Extract the three planes
    let y_plane = &ycbcr[0..y_size];
    let cb_plane = &ycbcr[y_size..(y_size + c_size)];
    let cr_plane = &ycbcr[(y_size + c_size)..(y_size + 2 * c_size)];

    // Use the direct YCbCr path that bypasses RGB conversion entirely
    let mut writer = BitWriter::new(|byte| {
        output
            .write_all(&[byte])
            .map_err(|_| "Failed to write output")
    });

    write_jpeg_ycbcr420(
        &mut writer,
        y_plane,
        cb_plane,
        cr_plane,
        options.width as u16,
        options.height as u16,
        options.quality.clamp(1, 100),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_encode_rgb() {
        // Create a simple 2x2 RGB image (red, green, blue, white)
        let pixels = vec![
            255, 0, 0, // Red
            0, 255, 0, // Green
            0, 0, 255, // Blue
            255, 255, 255, // White
        ];

        let options = EncodeOptions {
            width: 2,
            height: 2,
            format: ImageFormat::RGB,
            quality: 90,
            ..Default::default()
        };

        let mut output = Vec::new();
        encode_jpeg(&pixels, options, &mut output).unwrap();

        // Basic validation of JPEG output
        assert!(output.len() > 100); // Should be at least 100 bytes
        assert_eq!(&output[0..2], [0xFF, 0xD8]); // JPEG SOI marker
    }

    #[test]
    fn test_encode_ycbcr420_roundtrip() {
        // Create a simple 16x16 test pattern in YCbCr 4:2:0
        let width = 16;
        let height = 16;
        let c_width = (width + 1) / 2;
        let c_height = (height + 1) / 2;

        // Create gradient patterns for Y, Cb, Cr
        let mut y_plane = vec![0u8; width * height];
        let mut cb_plane = vec![128u8; c_width * c_height];
        let mut cr_plane = vec![128u8; c_width * c_height];

        // Y plane: horizontal gradient from black to white
        for y in 0..height {
            for x in 0..width {
                y_plane[y * width + x] = ((x as f32 / width as f32) * 255.0) as u8;
            }
        }

        // Cb plane: vertical gradient (blue to neutral)
        for y in 0..c_height {
            for x in 0..c_width {
                cb_plane[y * c_width + x] = 128 + ((y as f32 / c_height as f32) * 64.0) as u8;
            }
        }

        // Cr plane: vertical gradient (neutral to red)
        for y in 0..c_height {
            for x in 0..c_width {
                cr_plane[y * c_width + x] = 128 + ((y as f32 / c_height as f32) * 64.0) as u8;
            }
        }

        // Concatenate planes for encode_ycbcr420
        let mut ycbcr_buffer = Vec::new();
        ycbcr_buffer.extend_from_slice(&y_plane);
        ycbcr_buffer.extend_from_slice(&cb_plane);
        ycbcr_buffer.extend_from_slice(&cr_plane);

        // Encode to JPEG
        let options = EncodeOptions {
            width: width as u32,
            height: height as u32,
            format: ImageFormat::YCbCr420,
            quality: 90,
            baseline: true,
            optimized: true,
            downsample: true,
        };

        let mut jpeg_output = Vec::new();
        encode_ycbcr420(&ycbcr_buffer, options, &mut jpeg_output).unwrap();

        // Validate JPEG structure
        assert!(jpeg_output.len() > 200, "JPEG output too small: {} bytes", jpeg_output.len());
        assert_eq!(&jpeg_output[0..2], [0xFF, 0xD8], "Missing JPEG SOI marker");
        assert_eq!(&jpeg_output[jpeg_output.len()-2..], [0xFF, 0xD9], "Missing JPEG EOI marker");

        // Decode back with zune-jpeg for round-trip validation
        use zune_jpeg::JpegDecoder;
        use zune_core::options::DecoderOptions;

        let decode_options = DecoderOptions::default();
        let mut decoder = JpegDecoder::new_with_options(&jpeg_output, decode_options);

        let decoded_pixels = decoder.decode().expect("Failed to decode JPEG");
        let (decoded_width, decoded_height) = decoder.dimensions().unwrap();

        assert_eq!(decoded_width, width, "Width mismatch after round-trip");
        assert_eq!(decoded_height, height, "Height mismatch after round-trip");
        assert_eq!(decoded_pixels.len(), width * height * 3, "Decoded RGB size mismatch");

        // Verify the decoded image has reasonable values (not all black/white)
        let mut sum: u64 = 0;
        for &pixel in decoded_pixels.iter() {
            sum += pixel as u64;
        }
        let avg = sum / decoded_pixels.len() as u64;

        // Average should be somewhere in middle range (not 0 or 255)
        assert!(avg > 50 && avg < 200, "Decoded image average {} is suspicious", avg);

        println!("✓ YCbCr 4:2:0 round-trip test passed!");
        println!("  Original: {}x{} YCbCr 4:2:0", width, height);
        println!("  JPEG size: {} bytes", jpeg_output.len());
        println!("  Decoded: {}x{} RGB", decoded_width, decoded_height);
        println!("  Average pixel value: {}", avg);
    }
}
