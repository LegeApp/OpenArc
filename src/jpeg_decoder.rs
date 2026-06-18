//! JPEG decoder helpers backed by crates.io `zune-jpeg`.

use std::io::Cursor;

use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

/// Decoded YCbCr image (planar format, ready for BPG encoding).
#[derive(Debug, Clone)]
pub struct YCbCrImage {
    pub width: u32,
    pub height: u32,
    pub y_plane: Vec<u8>,
    pub cb_plane: Vec<u8>,
    pub cr_plane: Vec<u8>,
    pub y_stride: u32,
    pub cb_stride: u32,
    pub cr_stride: u32,
}

fn decode_with_options(
    data: &[u8],
    options: DecoderOptions,
) -> Result<(Vec<u8>, usize, usize, ColorSpace), String> {
    let mut dec = zune_jpeg::JpegDecoder::new_with_options(Cursor::new(data), options);
    let pixels = dec
        .decode()
        .map_err(|e| format!("JPEG decode error: {e:?}"))?;
    let (w, h) = dec
        .dimensions()
        .ok_or_else(|| "Failed to get JPEG dimensions".to_string())?;
    let cs = dec
        .output_colorspace()
        .ok_or_else(|| "Failed to get JPEG output colorspace".to_string())?;
    Ok((pixels, w, h, cs))
}

/// Decode JPEG bytes to interleaved RGB.
pub fn decode_jpeg_rgb(data: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let opts = DecoderOptions::default();
    let (pixels, w, h, cs) = decode_with_options(data, opts)?;
    match cs {
        ColorSpace::RGB => Ok((pixels, w as u32, h as u32)),
        ColorSpace::Luma => {
            let rgb: Vec<u8> = pixels.iter().flat_map(|&g| [g, g, g]).collect();
            Ok((rgb, w as u32, h as u32))
        }
        other => Err(format!("Unexpected output colorspace: {other:?}")),
    }
}

/// Decode JPEG bytes to YCbCr 4:2:0 planar.
pub fn decode_jpeg_ycbcr(data: &[u8]) -> Result<YCbCrImage, String> {
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::YCbCr);
    let (pixels, w, h, cs) = decode_with_options(data, opts)?;

    let width = w as u32;
    let height = h as u32;
    let chroma_w = width.div_ceil(2);
    let chroma_h = height.div_ceil(2);
    let pixel_count = (width * height) as usize;
    let chroma_count = (chroma_w * chroma_h) as usize;

    match cs {
        ColorSpace::YCbCr => {
            let mut y_plane = Vec::with_capacity(pixel_count);
            let mut cb_full = Vec::with_capacity(pixel_count);
            let mut cr_full = Vec::with_capacity(pixel_count);

            for chunk in pixels.chunks_exact(3) {
                y_plane.push(chunk[0]);
                cb_full.push(chunk[1]);
                cr_full.push(chunk[2]);
            }

            let mut cb_plane = Vec::with_capacity(chroma_count);
            let mut cr_plane = Vec::with_capacity(chroma_count);

            for cy in 0..chroma_h {
                for cx in 0..chroma_w {
                    let y0 = (cy * 2) as usize;
                    let x0 = (cx * 2) as usize;
                    let stride = width as usize;

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
                            if x >= stride {
                                break;
                            }
                            let idx = y * stride + x;
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
        other => Err(format!("Unexpected colorspace from YCbCr decode: {other:?}")),
    }
}
