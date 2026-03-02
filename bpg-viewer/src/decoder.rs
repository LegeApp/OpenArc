// BPG Decoder Module
use std::os::raw::c_int;
use std::ptr;
use anyhow::{Result, anyhow};
use lcms2::{Profile, CIExyY, CIExyYTRIPLE, ToneCurve};

use crate::ffi::{self, BPGImageFormat};

/// Decoded BPG image data
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: BPGImageFormat,
    pub color_space: u8,
    pub exif_data: Option<Vec<u8>>,
}

impl DecodedImage {
    /// Get the number of bytes per pixel
    pub fn bytes_per_pixel(&self) -> usize {
        match self.format {
            BPGImageFormat::Gray => 1,
            BPGImageFormat::RGB24 | BPGImageFormat::BGR24 => 3,
            BPGImageFormat::RGBA32 | BPGImageFormat::BGRA32 => 4,
            BPGImageFormat::YCbCr420P => 3, // Approximation
            BPGImageFormat::YCbCr444P => 3,
        }
    }

    /// Get stride (bytes per row)
    pub fn stride(&self) -> usize {
        self.width as usize * self.bytes_per_pixel()
    }

    /// Convert to RGBA32 if not already in that format
    pub fn to_rgba32(&self) -> Result<Vec<u8>> {
        if self.format == BPGImageFormat::RGBA32 {
            return Ok(self.data.clone());
        }

        let pixel_count = (self.width * self.height) as usize;
        let mut rgba_data = vec![0u8; pixel_count * 4];

        match self.format {
            BPGImageFormat::RGB24 => {
                for i in 0..pixel_count {
                    rgba_data[i * 4] = self.data[i * 3];
                    rgba_data[i * 4 + 1] = self.data[i * 3 + 1];
                    rgba_data[i * 4 + 2] = self.data[i * 3 + 2];
                    rgba_data[i * 4 + 3] = 255;
                }
            }
            BPGImageFormat::BGRA32 => {
                for i in 0..pixel_count {
                    rgba_data[i * 4] = self.data[i * 4 + 2];
                    rgba_data[i * 4 + 1] = self.data[i * 4 + 1];
                    rgba_data[i * 4 + 2] = self.data[i * 4];
                    rgba_data[i * 4 + 3] = self.data[i * 4 + 3];
                }
            }
            BPGImageFormat::BGR24 => {
                for i in 0..pixel_count {
                    rgba_data[i * 4] = self.data[i * 3 + 2];
                    rgba_data[i * 4 + 1] = self.data[i * 3 + 1];
                    rgba_data[i * 4 + 2] = self.data[i * 3];
                    rgba_data[i * 4 + 3] = 255;
                }
            }
            BPGImageFormat::Gray => {
                for i in 0..pixel_count {
                    let gray = self.data[i];
                    rgba_data[i * 4] = gray;
                    rgba_data[i * 4 + 1] = gray;
                    rgba_data[i * 4 + 2] = gray;
                    rgba_data[i * 4 + 3] = 255;
                }
            }
            _ => return Err(anyhow!("Unsupported format conversion: {:?}", self.format)),
        }

        Ok(rgba_data)
    }

    /// Convert to BGRA32 format (for WPF/Windows)
    pub fn to_bgra32(&self) -> Result<Vec<u8>> {
        let pixel_count = (self.width * self.height) as usize;
        let mut bgra_data = vec![0u8; pixel_count * 4];

        match self.format {
            BPGImageFormat::RGB24 => {
                for i in 0..pixel_count {
                    bgra_data[i * 4] = self.data[i * 3 + 2];     // B
                    bgra_data[i * 4 + 1] = self.data[i * 3 + 1]; // G
                    bgra_data[i * 4 + 2] = self.data[i * 3];     // R
                    bgra_data[i * 4 + 3] = 255;                   // A
                }
            }
            BPGImageFormat::RGBA32 => {
                for i in 0..pixel_count {
                    bgra_data[i * 4] = self.data[i * 4 + 2];     // B
                    bgra_data[i * 4 + 1] = self.data[i * 4 + 1]; // G
                    bgra_data[i * 4 + 2] = self.data[i * 4];     // R
                    bgra_data[i * 4 + 3] = self.data[i * 4 + 3]; // A
                }
            }
            BPGImageFormat::BGRA32 => {
                return Ok(self.data.clone());
            }
            BPGImageFormat::BGR24 => {
                for i in 0..pixel_count {
                    bgra_data[i * 4] = self.data[i * 3];         // B
                    bgra_data[i * 4 + 1] = self.data[i * 3 + 1]; // G
                    bgra_data[i * 4 + 2] = self.data[i * 3 + 2]; // R
                    bgra_data[i * 4 + 3] = 255;                   // A
                }
            }
            BPGImageFormat::Gray => {
                for i in 0..pixel_count {
                    let gray = self.data[i];
                    bgra_data[i * 4] = gray;     // B
                    bgra_data[i * 4 + 1] = gray; // G
                    bgra_data[i * 4 + 2] = gray; // R
                    bgra_data[i * 4 + 3] = 255;  // A
                }
            }
            _ => return Err(anyhow!("Unsupported format conversion: {:?}", self.format)),
        }

        Ok(bgra_data)
    }

    /// Copy decoded data to an output buffer with color conversion to sRGB + BGRA32 format
    pub fn copy_to_buffer(&self, output: &mut [u8], stride: usize) -> Result<()> {
        use lcms2::{Intent, PixelFormat, Profile, Transform};

        let height = self.height as usize;
        let width = self.width as usize;
        let src_row_bytes = width * 3; // RGB24
        let dst_row_bytes = width * 4;  // BGRA32

        eprintln!("copy_to_buffer: width={}, height={}, stride={}", width, height, stride);
        eprintln!("  self.data.len()={}, expected={}", self.data.len(), width * height * 3);
        eprintln!("  output.len()={}, required={}", output.len(), height * stride);

        if output.len() < height * stride {
            return Err(anyhow!("Output buffer too small"));
        }
        
        if self.data.len() < width * height * 3 {
            return Err(anyhow!("Source data incomplete: have {} bytes, need {} bytes", 
                self.data.len(), width * height * 3));
        }

        // Fast path: already sRGB (color_space == 1 is explicit RGB in BPG spec)
        if self.color_space == 1 {
            for y in 0..height {
                let src_offset = y * src_row_bytes;
                let dst_offset = y * stride;
                
                for x in 0..width {
                    let src_idx = src_offset + x * 3;
                    let dst_idx = dst_offset + x * 4;
                    
                    output[dst_idx + 0] = self.data[src_idx + 2]; // B
                    output[dst_idx + 1] = self.data[src_idx + 1]; // G
                    output[dst_idx + 2] = self.data[src_idx + 0]; // R
                    output[dst_idx + 3] = 255;                     // A
                }
            }
            return Ok(());
        }

        // Need color management
        let source_profile = match self.color_space {
            0 => create_bt601_profile()?,
            2 => create_bt709_profile()?,
            3 | 4 => create_bt2020_profile()?,
            _ => Profile::new_srgb(), // unknown → treat as sRGB
        };

        let srgb_profile = Profile::new_srgb();

        let transform = Transform::new(
            &source_profile,
            PixelFormat::RGB_8,
            &srgb_profile,
            PixelFormat::BGR_8,
            Intent::Perceptual,
        )?;

        let mut temp_bgr_row = vec![0u8; width * 3];

        for y in 0..height {
            let src_offset = y * src_row_bytes;
            let src_row = &self.data[src_offset..src_offset + src_row_bytes];

            transform.transform_pixels(src_row, &mut temp_bgr_row);

            let dst_offset = y * stride;

            for x in 0..width {
                let src_idx = x * 3;
                let dst_idx = dst_offset + x * 4;
                
                output[dst_idx + 0] = temp_bgr_row[src_idx + 0]; // B
                output[dst_idx + 1] = temp_bgr_row[src_idx + 1]; // G
                output[dst_idx + 2] = temp_bgr_row[src_idx + 2]; // R
                output[dst_idx + 3] = 255;                        // A
            }
        }

        Ok(())
    }
}

/// Create BT.601 (Rec. 601) color profile
fn create_bt601_profile() -> Result<Profile> {
    // BT.601 primaries
    let primaries = CIExyYTRIPLE {
        Red: CIExyY { x: 0.630, y: 0.340, Y: 1.0 },
        Green: CIExyY { x: 0.310, y: 0.595, Y: 1.0 },
        Blue: CIExyY { x: 0.155, y: 0.070, Y: 1.0 },
    };
    
    // D65 white point
    let white_point = CIExyY { x: 0.3127, y: 0.3290, Y: 1.0 };
    
    // BT.601 uses gamma 2.2
    let gamma = 2.2;
    let transfer_curve = ToneCurve::new(gamma);
    let transfer_curves = [&transfer_curve, &transfer_curve, &transfer_curve];
    
    Profile::new_rgb(&white_point, &primaries, &transfer_curves)
        .map_err(|e| anyhow!("Failed to create BT.601 profile: {:?}", e))
}

/// Create BT.709 (Rec. 709) color profile
fn create_bt709_profile() -> Result<Profile> {
    
    // BT.709 primaries
    let primaries = CIExyYTRIPLE {
        Red: CIExyY { x: 0.64, y: 0.33, Y: 1.0 },
        Green: CIExyY { x: 0.30, y: 0.60, Y: 1.0 },
        Blue: CIExyY { x: 0.15, y: 0.06, Y: 1.0 },
    };
    
    // D65 white point
    let white_point = CIExyY { x: 0.3127, y: 0.3290, Y: 1.0 };
    
    // BT.709 uses gamma 2.4 (similar to sRGB but without the linear segment)
    let gamma = 2.4;
    let transfer_curve = ToneCurve::new(gamma);
    let transfer_curves = [&transfer_curve, &transfer_curve, &transfer_curve];
    
    Profile::new_rgb(&white_point, &primaries, &transfer_curves)
        .map_err(|e| anyhow!("Failed to create BT.709 profile: {:?}", e))
}

/// Create BT.2020 (Rec. 2020) color profile
fn create_bt2020_profile() -> Result<Profile> {
    
    // BT.2020 primaries (wider gamut)
    let primaries = CIExyYTRIPLE {
        Red: CIExyY { x: 0.708, y: 0.292, Y: 1.0 },
        Green: CIExyY { x: 0.170, y: 0.797, Y: 1.0 },
        Blue: CIExyY { x: 0.131, y: 0.046, Y: 1.0 },
    };
    
    // D65 white point
    let white_point = CIExyY { x: 0.3127, y: 0.3290, Y: 1.0 };
    
    // BT.2020 uses gamma 2.4
    let gamma = 2.4;
    let transfer_curve = ToneCurve::new(gamma);
    let transfer_curves = [&transfer_curve, &transfer_curve, &transfer_curve];
    
    Profile::new_rgb(&white_point, &primaries, &transfer_curves)
        .map_err(|e| anyhow!("Failed to create BT.2020 profile: {:?}", e))
}

/// Decode a BPG file
pub fn decode_file(input_path: &str) -> Result<DecodedImage> {
    // Read the file into memory, then use the memory-based decoder
    // This works with the in-memory-only BPG library
    let input_data = std::fs::read(input_path)?;
    decode_memory(&input_data)
}

/// Decode BPG data from memory
pub fn decode_memory(input_data: &[u8]) -> Result<DecodedImage> {
    unsafe {
        // Open decoder
        let decoder_ctx = ffi::bpg_decoder_open();
        if decoder_ctx.is_null() {
            return Err(anyhow!("Failed to create decoder context"));
        }

        // Enable keeping extension data (EXIF, ICC, etc.)
        ffi::bpg_decoder_keep_extension_data(decoder_ctx, 1);

        // Decode the buffer
        let result = ffi::bpg_decoder_decode(decoder_ctx, input_data.as_ptr(), input_data.len() as c_int);
        if result < 0 {
            ffi::bpg_decoder_close(decoder_ctx);
            return Err(anyhow!("Decoding failed with error code: {}", result));
        }

        // Get image info
        let mut img_info = std::mem::MaybeUninit::<ffi::BPGImageInfo>::uninit();
        let result = ffi::bpg_decoder_get_info(decoder_ctx, img_info.as_mut_ptr());
        if result < 0 {
            ffi::bpg_decoder_close(decoder_ctx);
            return Err(anyhow!("Failed to get image info with error code: {}", result));
        }
        let img_info = img_info.assume_init();

        eprintln!("=== DECODE_MEMORY START ===");
        eprintln!("Image size: {}x{}", img_info.width, img_info.height);
        eprintln!("Format: {}, Color space: {}", img_info.format, img_info.color_space);

        // Get extension data
        let mut exif_data = None;
        let mut first_md: *mut ffi::BPGExtensionData = ptr::null_mut();
        if ffi::bpg_decoder_get_extension_data(decoder_ctx, &mut first_md) == 0 {
            let mut curr = first_md;
            while !curr.is_null() {
                // Tag 1 = EXIF
                if (*curr).tag == 1 && (*curr).len > 0 {
                    let slice = std::slice::from_raw_parts((*curr).buf, (*curr).len as usize);
                    exif_data = Some(slice.to_vec());
                }
                curr = (*curr).next;
            }
        }

        // Start decoder with RGB24 output format
        let result = ffi::bpg_decoder_start(decoder_ctx, ffi::BPGDecoderOutputFormat::RGB24);
        if result < 0 {
            ffi::bpg_decoder_close(decoder_ctx);
            return Err(anyhow!("Failed to start decoder with error code: {}", result));
        }

        // Calculate output size (RGB24 = 3 bytes per pixel)
        let output_row_bytes = (img_info.width * 3) as usize;
        let output_size = output_row_bytes * img_info.height as usize;
        let mut output_data: Vec<u8> = vec![0u8; output_size];

        eprintln!("Allocating output buffer: {} bytes ({} x {} x 3)",
            output_size, img_info.width, img_info.height);
        eprintln!("Row bytes: {}", output_row_bytes);

        // Get each scanline using bpg_decoder_get_line (converts to RGB24)
        for y in 0..img_info.height as usize {
            let row_ptr = output_data.as_mut_ptr().add(y * output_row_bytes);
            let result = ffi::bpg_decoder_get_line(decoder_ctx, row_ptr as *mut std::ffi::c_void);
            if result < 0 {
                ffi::bpg_decoder_close(decoder_ctx);
                return Err(anyhow!("Failed to get scanline {} with error code: {}", y, result));
            }
        }

        eprintln!("Decoded {} scanlines, total data: {} bytes", img_info.height, output_data.len());
        eprintln!("=== DECODE_MEMORY COMPLETE ===");

        ffi::bpg_decoder_close(decoder_ctx);

        Ok(DecodedImage {
            data: output_data,
            width: img_info.width,
            height: img_info.height,
            format: BPGImageFormat::RGB24, // The output format is RGB24 as specified
            color_space: img_info.color_space,
            exif_data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_per_pixel() {
        let img = DecodedImage {
            data: vec![],
            width: 10,
            height: 10,
            format: BPGImageFormat::RGBA32,
            color_space: 0,
            exif_data: None,
        };
        assert_eq!(img.bytes_per_pixel(), 4);

        let img_rgb = DecodedImage {
            data: vec![],
            width: 10,
            height: 10,
            format: BPGImageFormat::RGB24,
            color_space: 0,
            exif_data: None,
        };
        assert_eq!(img_rgb.bytes_per_pixel(), 3);
    }
}
