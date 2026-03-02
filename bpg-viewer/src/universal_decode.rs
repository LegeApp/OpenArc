// Universal Image Decoding Module
// Supports full-resolution decoding of BPG, standard image formats, HEIC/HEIF, RAW, DNG, and JPEG2000 files
// Returns either BGRA (for UI) or native YCbCr (for JPEG thumbnails) to avoid conversion overhead

use std::path::Path;
use anyhow::{Result, anyhow};
use image::{DynamicImage, ImageBuffer, Rgba};

use crate::decoder::decode_file as decode_bpg_file;
use crate::image_data::ImageData;

/// Decoded image data with flexible format support
pub struct UniversalDecodedImage {
    pub width: u32,
    pub height: u32,
    /// Image data in either BGRA or YCbCr format
    /// - BGRA: For UI display (WPF, WinForms)
    /// - YCbCr: For JPEG/HEIC native format (avoids conversion)
    pub data: ImageData,
}

/// Convenience function: decode any supported image file to interleaved RGB.
/// Used by the GPU thumbnail pipeline to feed non-JPEG images into the YCbCr conversion.
pub fn decode_to_rgb(path: &Path) -> Result<(Vec<u8>, u32, u32), String> {
    let decoded = UniversalDecodedImage::decode_file(path)
        .map_err(|e| format!("Decode failed for {}: {}", path.display(), e))?;

    // Get BGRA data (convert if necessary)
    let bgra = decoded.to_bgra()
        .map_err(|e| format!("BGRA conversion failed: {}", e))?;

    // Convert BGRA → RGB
    let pixel_count = (decoded.width * decoded.height) as usize;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    for chunk in bgra.chunks_exact(4) {
        rgb.push(chunk[2]); // R (from BGRA[2])
        rgb.push(chunk[1]); // G (from BGRA[1])
        rgb.push(chunk[0]); // B (from BGRA[0])
    }

    Ok((rgb, decoded.width, decoded.height))
}

impl UniversalDecodedImage {
    /// Convert image data to BGRA format (for UI compatibility)
    ///
    /// If data is already BGRA, this is a cheap clone.
    /// If data is YCbCr, this performs conversion.
    pub fn to_bgra(&self) -> Result<Vec<u8>> {
        self.data.to_bgra8(self.width, self.height)
    }

    /// Check if image is in native YCbCr format
    pub fn is_ycbcr(&self) -> bool {
        self.data.is_ycbcr()
    }

    /// Decode any supported image file
    ///
    /// Returns image in native format:
    /// - HEIC/HEIF: YCbCr 4:2:0 (optimal for JPEG thumbnails)
    /// - JPEG: BGRA (YCbCr decode not yet implemented)
    /// - PNG, BMP, etc.: BGRA
    /// - BPG: BGRA
    pub fn decode_file(input_path: &Path) -> Result<Self> {
        let file_ext = input_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        match file_ext.as_str() {
            "bpg" => Self::decode_bpg(input_path),
            "heic" | "heif" => Self::decode_heic(input_path),
            "dng" => Self::decode_dng(input_path),
            "jp2" | "j2k" | "j2c" | "jpc" | "jpt" | "jph" | "jhc" => Self::decode_jpeg2000(input_path),
            "cr2" | "nef" | "arw" | "orf" | "rw2" | "raf" | "3fr" | "fff" | "dcr" | "kdc" | "srf" | "sr2" | "erf" | "mef" | "mrw" | "nrw" | "pef" | "iiq" | "x3f" => {
                Self::decode_raw(input_path)
            }
            _ => Self::decode_standard(input_path),
        }
    }

    /// Decode BPG file (returns BGRA)
    fn decode_bpg(input_path: &Path) -> Result<Self> {
        let decoded = decode_bpg_file(input_path.to_str().unwrap())?;
        let bgra = decoded.to_bgra32()?;
        Ok(Self {
            width: decoded.width,
            height: decoded.height,
            data: ImageData::from_bgra(bgra),
        })
    }

    /// Decode standard image formats (JPEG, PNG, TIFF, etc.)
    fn decode_standard(input_path: &Path) -> Result<Self> {
        let img = image::open(input_path)
            .map_err(|e| anyhow!("Failed to open image {}: {}", input_path.display(), e))?;

        Self::from_dynamic_image(img)
    }

    /// Decode HEIC/HEIF files (returns YCbCr 4:2:0 for optimal thumbnail encoding)
    fn decode_heic(input_path: &Path) -> Result<Self> {
        // Try YCbCr decode first (optimal path for JPEG thumbnails)
        let mut codec = codecs::heic::HeicCodec::new()
            .map_err(|e| anyhow!("Failed to create HEIC codec: {}", e))?;

        match codec.decode_file_ycbcr420(input_path) {
            Ok(ycbcr) => {
                // Return native YCbCr format (avoids conversion for JPEG thumbnails)
                Ok(Self {
                    width: ycbcr.width,
                    height: ycbcr.height,
                    data: ImageData::from_ycbcr420(
                        ycbcr.y_plane,
                        ycbcr.cb_plane,
                        ycbcr.cr_plane,
                        ycbcr.width,
                        ycbcr.height,
                        ycbcr.y_stride,
                        ycbcr.cb_stride,
                    ),
                })
            }
            Err(_) => {
                // Fallback to RGB decode (for images that don't support YCbCr)
                let decoded = codecs::heic::decode_heic_file(input_path)?;

                let mut rgba = Vec::with_capacity(decoded.width as usize * decoded.height as usize * 4);
                if decoded.has_alpha {
                    rgba.extend_from_slice(&decoded.data);
                } else {
                    for rgb in decoded.data.chunks(3) {
                        if rgb.len() == 3 {
                            rgba.push(rgb[0]);
                            rgba.push(rgb[1]);
                            rgba.push(rgb[2]);
                            rgba.push(255);
                        }
                    }
                }

                // Convert RGBA to BGRA
                let bgra = Self::rgba_to_bgra(&rgba);

                Ok(Self {
                    width: decoded.width,
                    height: decoded.height,
                    data: ImageData::from_bgra(bgra),
                })
            }
        }
    }

    /// Decode RAW files
    fn decode_raw(input_path: &Path) -> Result<Self> {
        use rawloader::RawLoader;

        let raw = RawLoader::new().decode_file(input_path)
            .map_err(|e| anyhow!("Failed to decode RAW file {}: {}", input_path.display(), e))?;

        let width = raw.width;
        let height = raw.height;

        // Convert to BGRA (simple demosaicing - grayscale for now)
        let mut bgra_data = vec![0u8; (width * height * 4) as usize];

        match &raw.data {
            rawloader::RawImageData::Float(data) => {
                for (i, &value) in data.iter().enumerate() {
                    let pixel_value = (value * 255.0) as u8;
                    bgra_data[i * 4] = pixel_value;     // B
                    bgra_data[i * 4 + 1] = pixel_value; // G
                    bgra_data[i * 4 + 2] = pixel_value; // R
                    bgra_data[i * 4 + 3] = 255;         // A
                }
            }
            rawloader::RawImageData::Integer(data) => {
                for (i, &value) in data.iter().enumerate() {
                    let pixel_value = (value >> 8) as u8;
                    bgra_data[i * 4] = pixel_value;     // B
                    bgra_data[i * 4 + 1] = pixel_value; // G
                    bgra_data[i * 4 + 2] = pixel_value; // R
                    bgra_data[i * 4 + 3] = 255;         // A
                }
            }
        }

        Ok(Self {
            width: width as u32,
            height: height as u32,
            data: ImageData::from_bgra(bgra_data),
        })
    }

    /// Decode DNG files
    fn decode_dng(input_path: &Path) -> Result<Self> {
        // Prefer embedded JPEG preview when present (much faster and more accurate)
        if let Ok(preview) = Self::try_decode_dng_embedded_jpeg_preview(input_path) {
            return Self::from_dynamic_image(preview);
        }

        // Fallback to RAW decode
        Self::decode_raw(input_path)
    }

    fn try_decode_dng_embedded_jpeg_preview(input_path: &Path) -> Result<DynamicImage> {
        use std::fs::File;
        use dng::DngReader;
        use dng::ifd::IfdPath;
        use dng::tags::ifd;

        let file = File::open(input_path)
            .map_err(|e| anyhow!("Failed to open DNG {}: {}", input_path.display(), e))?;

        let reader = DngReader::read(file)
            .map_err(|e| anyhow!("Failed to parse DNG {}: {}", input_path.display(), e))?;

        let path = IfdPath::default().chain_tag(ifd::JPEGInterchangeFormat);
        let entry = reader
            .get_entry_by_path(&path)
            .ok_or_else(|| anyhow!("No embedded JPEG preview"))?;

        let len = reader
            .needed_buffer_size_for_offsets(entry)
            .map_err(|e| anyhow!("Failed reading DNG preview length: {}", e))?;

        let mut buf = vec![0u8; len];
        reader
            .read_offsets_to_buffer(entry, &mut buf)
            .map_err(|e| anyhow!("Failed reading DNG preview bytes: {}", e))?;

        image::load_from_memory(&buf)
            .map_err(|e| anyhow!("Failed decoding embedded DNG JPEG preview: {}", e))
    }

    /// Decode JPEG2000 files
    fn decode_jpeg2000(input_path: &Path) -> Result<Self> {
        use openjp2::{Codec, CODEC_FORMAT, Stream};
        use openjp2::openjpeg::opj_set_default_decoder_parameters;

        let format = match openjp2::detect_format_from_extension(input_path.extension()) {
            Ok(openjp2::J2KFormat::JP2) => CODEC_FORMAT::OPJ_CODEC_JP2,
            Ok(openjp2::J2KFormat::J2K) => CODEC_FORMAT::OPJ_CODEC_J2K,
            Ok(openjp2::J2KFormat::JPT) => CODEC_FORMAT::OPJ_CODEC_JPT,
            Err(_) => CODEC_FORMAT::OPJ_CODEC_J2K,
        };

        let mut stream = Stream::new_file(input_path, 1 << 20, true)
            .map_err(|e| anyhow!("Failed to open JPEG2000 {}: {}", input_path.display(), e))?;

        let mut codec = Codec::new_decoder(format)
            .ok_or_else(|| anyhow!("Failed to create JPEG2000 decoder"))?;

        let mut params = openjp2::opj_dparameters_t::default();
        unsafe { opj_set_default_decoder_parameters(&mut params) };
        if codec.setup_decoder(&mut params) == 0 {
            return Err(anyhow!("JPEG2000 setup_decoder failed"));
        }

        let mut img = codec
            .read_header(&mut stream)
            .ok_or_else(|| anyhow!("JPEG2000 read_header failed"))?;

        if codec.decode(&mut stream, &mut img) == 0 {
            return Err(anyhow!("JPEG2000 decode failed"));
        }
        let _ = codec.end_decompress(&mut stream);

        let comps = img
            .comps_data_iter()
            .ok_or_else(|| anyhow!("JPEG2000: no components"))?
            .collect::<Vec<_>>();

        if comps.is_empty() {
            return Err(anyhow!("JPEG2000: empty components"));
        }

        let (w, h, prec) = img.comp0_dims_prec();
        if w == 0 || h == 0 {
            return Err(anyhow!("JPEG2000: invalid dimensions"));
        }

        let max_val = if prec > 0 && prec < 31 { (1i64 << prec) - 1 } else { 255 };
        let scale = 255.0 / max_val as f64;
        let pixel_count = w * h;

        let mut bgra = Vec::with_capacity(pixel_count * 4);

        let comp_to_u8 = |comp: &openjp2::image::ImageCompRef<'_>, i: usize| -> u8 {
            let v = comp.data[i] + comp.adjust;
            let v = (v as f64 * scale).round();
            v.clamp(0.0, 255.0) as u8
        };

        if comps.len() >= 3 {
            for i in 0..pixel_count {
                let r = comp_to_u8(&comps[0], i);
                let g = comp_to_u8(&comps[1], i);
                let b = comp_to_u8(&comps[2], i);
                let a = if comps.len() >= 4 { comp_to_u8(&comps[3], i) } else { 255 };
                bgra.push(b);
                bgra.push(g);
                bgra.push(r);
                bgra.push(a);
            }
        } else {
            for i in 0..pixel_count {
                let g = comp_to_u8(&comps[0], i);
                bgra.push(g);
                bgra.push(g);
                bgra.push(g);
                bgra.push(255);
            }
        }

        Ok(Self {
            width: w as u32,
            height: h as u32,
            data: ImageData::from_bgra(bgra),
        })
    }

    /// Convert DynamicImage to BGRA format
    fn from_dynamic_image(img: DynamicImage) -> Result<Self> {
        let rgba = img.to_rgba8();
        let (width, height) = (rgba.width(), rgba.height());
        let bgra = Self::rgba_to_bgra(rgba.as_raw());

        Ok(Self {
            width,
            height,
            data: ImageData::from_bgra(bgra),
        })
    }

    /// Convert RGBA to BGRA
    fn rgba_to_bgra(rgba: &[u8]) -> Vec<u8> {
        let mut bgra = Vec::with_capacity(rgba.len());
        for chunk in rgba.chunks(4) {
            if chunk.len() == 4 {
                bgra.push(chunk[2]); // B
                bgra.push(chunk[1]); // G
                bgra.push(chunk[0]); // R
                bgra.push(chunk[3]); // A
            }
        }
        bgra
    }

    /// Check if a file extension is supported
    pub fn is_supported_format(file_path: &Path) -> bool {
        let file_ext = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        matches!(file_ext.as_str(),
            // BPG
            "bpg" |
            // Standard formats
            "jpg" | "jpeg" | "png" | "tiff" | "tif" | "bmp" | "webp" | "gif" | "ico" |
            "pnm" | "pbm" | "pgm" | "ppm" | "pam" | "dds" | "tga" |
            "hdr" | "exr" |
            // HEIC/HEIF
            "heic" | "heif" |
            // RAW formats
            "cr2" | "nef" | "arw" | "orf" | "rw2" | "raf" | "3fr" | "fff" | "dcr" |
            "kdc" | "srf" | "sr2" | "erf" | "mef" | "mrw" | "nrw" | "pef" | "iiq" | "x3f" |
            // DNG
            "dng" |
            // JPEG2000
            "jp2" | "j2k" | "j2c" | "jpc" | "jpt" | "jph" | "jhc"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_formats() {
        assert!(UniversalDecodedImage::is_supported_format(Path::new("test.jpg")));
        assert!(UniversalDecodedImage::is_supported_format(Path::new("test.bpg")));
        assert!(UniversalDecodedImage::is_supported_format(Path::new("test.cr2")));
        assert!(UniversalDecodedImage::is_supported_format(Path::new("test.dng")));
        assert!(UniversalDecodedImage::is_supported_format(Path::new("test.heic")));
        assert!(!UniversalDecodedImage::is_supported_format(Path::new("test.txt")));
    }

    #[test]
    fn test_rgba_to_bgra() {
        let rgba = vec![255, 128, 64, 255]; // R=255, G=128, B=64, A=255
        let bgra = UniversalDecodedImage::rgba_to_bgra(&rgba);
        assert_eq!(bgra, vec![64, 128, 255, 255]); // B=64, G=128, R=255, A=255
    }
}
