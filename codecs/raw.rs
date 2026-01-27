use std::ffi::CString;
use std::path::Path;
use anyhow::{anyhow, Result};
use image::{ImageBuffer, Rgb};
use tempfile::NamedTempFile;

use super::libraw_sys::*;
use std::os::raw::c_int;

pub struct RawConverter;

impl RawConverter {
    pub fn new() -> Self {
        Self
    }

    pub fn convert_to_png(&self, raw_path: &Path) -> Result<Vec<u8>> {
        let raw_path_c = CString::new(raw_path.to_string_lossy().as_bytes())?;
        
        // Initialize libraw
        let lr = unsafe { libraw_init(0) };
        if lr.is_null() {
            return Err(anyhow!("Failed to initialize libraw"));
        }

        // Open file
        let result = unsafe { libraw_open_file(lr, raw_path_c.as_ptr()) };
        if result != libraw_errors_t::LIBRAW_SUCCESS as c_int {
            let error_msg = libraw_error_string(result);
            unsafe { libraw_close(lr) };
            return Err(anyhow!("Failed to open RAW file: {}", error_msg));
        }

        // Unpack data
        let result = unsafe { libraw_unpack(lr) };
        if result != libraw_errors_t::LIBRAW_SUCCESS as c_int {
            let error_msg = libraw_error_string(result);
            unsafe { libraw_close(lr) };
            return Err(anyhow!("Failed to unpack RAW data: {}", error_msg));
        }

        // Process image
        let result = unsafe { libraw_dcraw_process(lr) };
        if result != libraw_errors_t::LIBRAW_SUCCESS as c_int {
            let error_msg = libraw_error_string(result);
            unsafe { libraw_close(lr) };
            return Err(anyhow!("Failed to process RAW image: {}", error_msg));
        }

        // Write to temporary PPM file
        let temp_ppm = NamedTempFile::new()?;
        let temp_ppm_path = CString::new(temp_ppm.path().to_string_lossy().as_bytes())?;
        
        let result = unsafe { libraw_dcraw_ppm_tiff_writer(lr, temp_ppm_path.as_ptr()) };
        if result != libraw_errors_t::LIBRAW_SUCCESS as c_int {
            let error_msg = libraw_error_string(result);
            unsafe { libraw_close(lr) };
            return Err(anyhow!("Failed to write PPM: {}", error_msg));
        }

        unsafe { libraw_close(lr) };

        // Read PPM and convert to PNG
        let ppm_data = std::fs::read(temp_ppm.path())?;
        self.ppm_to_png(&ppm_data)
    }

    pub(crate) fn ppm_to_png(&self, ppm_data: &[u8]) -> Result<Vec<u8>> {
        let ppm_str = String::from_utf8_lossy(ppm_data);
        let mut lines = ppm_str.lines();
        
        // Parse PPM header
        let magic = lines.next().ok_or_else(|| anyhow!("Invalid PPM: no magic number"))?;
        if magic != "P6" {
            return Err(anyhow!("Unsupported PPM format: {}", magic));
        }

        let dimensions = lines.next().ok_or_else(|| anyhow!("Invalid PPM: no dimensions"))?;
        let mut parts = dimensions.split_whitespace();
        let width: u32 = parts.next()
            .ok_or_else(|| anyhow!("Invalid PPM: no width"))?
            .parse()
            .map_err(|_| anyhow!("Invalid PPM: invalid width"))?;
        let height: u32 = parts.next()
            .ok_or_else(|| anyhow!("Invalid PPM: no height"))?
            .parse()
            .map_err(|_| anyhow!("Invalid PPM: invalid height"))?;

        let max_val = lines.next().ok_or_else(|| anyhow!("Invalid PPM: no max value"))?;
        let max_val: u16 = max_val.parse().map_err(|_| anyhow!("Invalid PPM: invalid max value"))?;

        // Find start of binary data
        let header_end = ppm_str.find("P6\n")
            .and_then(|i| ppm_str[i..].find('\n'))
            .and_then(|i| ppm_str[i..].find('\n'))
            .and_then(|i| ppm_str[i..].find('\n'))
            .map(|i| {
                let pos = ppm_str[i..].find('\n').unwrap_or(0);
                i + pos + 1
            })
            .ok_or_else(|| anyhow!("Invalid PPM: cannot find data start"))?;

        let binary_data = &ppm_data[header_end..];
        
        // Convert to 16-bit RGB image
        let mut img_data = Vec::with_capacity((width * height) as usize * 3);
        
        if max_val == 65535 {
            // Already 16-bit
            for chunk in binary_data.chunks_exact(6) {
                if chunk.len() < 6 { break; }
                let r = u16::from_be_bytes([chunk[0], chunk[1]]);
                let g = u16::from_be_bytes([chunk[2], chunk[3]]);
                let b = u16::from_be_bytes([chunk[4], chunk[5]]);
                img_data.push([r, g, b]);
            }
        } else if max_val == 255 {
            // Convert 8-bit to 16-bit
            for chunk in binary_data.chunks_exact(3) {
                if chunk.len() < 3 { break; }
                let r = (chunk[0] as u16) << 8;
                let g = (chunk[1] as u16) << 8;
                let b = (chunk[2] as u16) << 8;
                img_data.push([r, g, b]);
            }
        } else {
            return Err(anyhow!("Unsupported PPM max value: {}", max_val));
        }

        // Create image buffer
        let img: ImageBuffer<Rgb<u16>, Vec<u16>> = ImageBuffer::from_raw(width, height, 
            img_data.into_iter().flatten().collect())
            .ok_or_else(|| anyhow!("Failed to create image buffer"))?;

        // Encode as PNG
        let png_data = image::DynamicImage::ImageRgb16(img)
            .into_rgb8();
        
        let mut png_bytes = Vec::new();
        {
            let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
            png_data.write_with_encoder(encoder)?;
        }
        
        Ok(png_bytes)
    }
}

impl Default for RawConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_raw_converter_new() {
        let converter = RawConverter::new();
        // Just test that it doesn't panic
        assert!(true);
    }

    #[test]
    fn test_raw_converter_default() {
        let converter = RawConverter::default();
        // Just test that it doesn't panic
        assert!(true);
    }

    #[test]
    fn test_ppm_to_png_invalid_format() {
        let converter = RawConverter::new();
        
        // Test with invalid PPM format
        let ppm_data = b"P5\n2 2\n255\n1234"; // P5 is grayscale, not RGB
        
        let result = converter.ppm_to_png(ppm_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_nonexistent_file() {
        let converter = RawConverter::new();
        let nonexistent_path = PathBuf::from("definitely_does_not_exist.cr2");
        
        let result = converter.convert_to_png(&nonexistent_path);
        assert!(result.is_err());
    }
}
