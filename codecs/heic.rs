// HEIC/HEIF Encoding and Decoding via libheif
// Provides full encode/decode support for Apple HEIC and HEIF image formats
// Used by Samsung, Android, and Apple devices

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;
use std::ptr;
use anyhow::{Result, anyhow};

// Opaque libheif types
#[repr(C)]
pub struct HeifContext {
    _private: [u8; 0],
}

#[repr(C)]
pub struct HeifImageHandle {
    _private: [u8; 0],
}

#[repr(C)]
pub struct HeifImage {
    _private: [u8; 0],
}

#[repr(C)]
pub struct HeifEncoder {
    _private: [u8; 0],
}

#[repr(C)]
pub struct HeifEncodingOptions {
    _private: [u8; 0],
}

#[repr(C)]
pub struct HeifError {
    pub code: c_int,
    pub subcode: c_int,
    pub message: *const c_char,
}

// Compression format for encoding
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeifCompressionFormat {
    Undefined = 0,
    HEVC = 1,      // H.265 - standard HEIC
    AVC = 2,       // H.264
    JPEG = 3,
    AV1 = 4,       // AVIF
}

// Colorspace and chroma
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeifColorspace {
    Undefined = 99,
    YCbCr = 0,
    RGB = 1,
    Monochrome = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeifChroma {
    Undefined = 99,
    Monochrome = 0,
    Chroma420 = 1,
    Chroma422 = 2,
    Chroma444 = 3,
    InterleavedRGB = 10,
    InterleavedRGBA = 11,
    InterleavedRRGGBBAA_BE = 12,
    InterleavedRRGGBBAA_LE = 13,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeifChannel {
    Y = 0,
    Cb = 1,
    Cr = 2,
    R = 3,
    G = 4,
    B = 5,
    Alpha = 6,
    Interleaved = 10,
}

// FFI declarations for libheif (decoding)
#[cfg(feature = "heif")]
extern "C" {
    // Context management
    fn heif_context_alloc() -> *mut HeifContext;
    fn heif_context_free(ctx: *mut HeifContext);
    fn heif_context_read_from_file(
        ctx: *mut HeifContext,
        filename: *const c_char,
        options: *const c_void,
    ) -> HeifError;
    fn heif_context_write_to_file(
        ctx: *mut HeifContext,
        filename: *const c_char,
    ) -> HeifError;

    // Decoding
    fn heif_context_get_primary_image_handle(
        ctx: *mut HeifContext,
        handle: *mut *mut HeifImageHandle,
    ) -> HeifError;
    fn heif_image_handle_release(handle: *mut HeifImageHandle);
    fn heif_image_handle_get_width(handle: *const HeifImageHandle) -> c_int;
    fn heif_image_handle_get_height(handle: *const HeifImageHandle) -> c_int;
    fn heif_image_handle_has_alpha_channel(handle: *const HeifImageHandle) -> c_int;
    fn heif_decode_image(
        handle: *const HeifImageHandle,
        out_img: *mut *mut HeifImage,
        colorspace: HeifColorspace,
        chroma: HeifChroma,
        options: *const c_void,
    ) -> HeifError;

    // Image data access
    fn heif_image_release(img: *mut HeifImage);
    fn heif_image_get_plane_readonly(
        img: *const HeifImage,
        channel: HeifChannel,
        out_stride: *mut c_int,
    ) -> *const u8;
    fn heif_image_get_plane(
        img: *mut HeifImage,
        channel: HeifChannel,
        out_stride: *mut c_int,
    ) -> *mut u8;

    // Image creation for encoding
    fn heif_image_create(
        width: c_int,
        height: c_int,
        colorspace: HeifColorspace,
        chroma: HeifChroma,
        out_image: *mut *mut HeifImage,
    ) -> HeifError;
    fn heif_image_add_plane(
        img: *mut HeifImage,
        channel: HeifChannel,
        width: c_int,
        height: c_int,
        bit_depth: c_int,
    ) -> HeifError;

    // Encoding
    fn heif_context_get_encoder_for_format(
        ctx: *mut HeifContext,
        format: HeifCompressionFormat,
        encoder: *mut *mut HeifEncoder,
    ) -> HeifError;
    fn heif_encoder_set_lossy_quality(encoder: *mut HeifEncoder, quality: c_int) -> HeifError;
    fn heif_encoder_set_lossless(encoder: *mut HeifEncoder, lossless: c_int) -> HeifError;
    fn heif_encoder_release(encoder: *mut HeifEncoder);
    fn heif_context_encode_image(
        ctx: *mut HeifContext,
        img: *const HeifImage,
        encoder: *mut HeifEncoder,
        options: *const HeifEncodingOptions,
        out_handle: *mut *mut HeifImageHandle,
    ) -> HeifError;

    // Version info
    fn heif_get_version() -> *const c_char;
}

// Stub implementations when libheif is not available
#[cfg(not(feature = "heif"))]
mod stubs {
    use super::*;

    pub unsafe fn heif_context_alloc() -> *mut HeifContext { ptr::null_mut() }
    pub unsafe fn heif_context_free(_ctx: *mut HeifContext) {}
    pub unsafe fn heif_context_read_from_file(
        _ctx: *mut HeifContext, _filename: *const c_char, _options: *const c_void,
    ) -> HeifError { HeifError { code: -1, subcode: 0, message: ptr::null() } }
    pub unsafe fn heif_context_write_to_file(
        _ctx: *mut HeifContext, _filename: *const c_char,
    ) -> HeifError { HeifError { code: -1, subcode: 0, message: ptr::null() } }
    pub unsafe fn heif_context_get_primary_image_handle(
        _ctx: *mut HeifContext, _handle: *mut *mut HeifImageHandle,
    ) -> HeifError { HeifError { code: -1, subcode: 0, message: ptr::null() } }
    pub unsafe fn heif_image_handle_release(_handle: *mut HeifImageHandle) {}
    pub unsafe fn heif_image_handle_get_width(_handle: *const HeifImageHandle) -> c_int { 0 }
    pub unsafe fn heif_image_handle_get_height(_handle: *const HeifImageHandle) -> c_int { 0 }
    pub unsafe fn heif_image_handle_has_alpha_channel(_handle: *const HeifImageHandle) -> c_int { 0 }
    pub unsafe fn heif_decode_image(
        _handle: *const HeifImageHandle, _out_img: *mut *mut HeifImage,
        _colorspace: HeifColorspace, _chroma: HeifChroma, _options: *const c_void,
    ) -> HeifError { HeifError { code: -1, subcode: 0, message: ptr::null() } }
    pub unsafe fn heif_image_release(_img: *mut HeifImage) {}
    pub unsafe fn heif_image_get_plane_readonly(
        _img: *const HeifImage, _channel: HeifChannel, _out_stride: *mut c_int,
    ) -> *const u8 { ptr::null() }
    pub unsafe fn heif_image_get_plane(
        _img: *mut HeifImage, _channel: HeifChannel, _out_stride: *mut c_int,
    ) -> *mut u8 { ptr::null_mut() }
    pub unsafe fn heif_image_create(
        _width: c_int, _height: c_int, _colorspace: HeifColorspace, _chroma: HeifChroma,
        _out_image: *mut *mut HeifImage,
    ) -> HeifError { HeifError { code: -1, subcode: 0, message: ptr::null() } }
    pub unsafe fn heif_image_add_plane(
        _img: *mut HeifImage, _channel: HeifChannel, _width: c_int, _height: c_int, _bit_depth: c_int,
    ) -> HeifError { HeifError { code: -1, subcode: 0, message: ptr::null() } }
    pub unsafe fn heif_context_get_encoder_for_format(
        _ctx: *mut HeifContext, _format: HeifCompressionFormat, _encoder: *mut *mut HeifEncoder,
    ) -> HeifError { HeifError { code: -1, subcode: 0, message: ptr::null() } }
    pub unsafe fn heif_encoder_set_lossy_quality(_encoder: *mut HeifEncoder, _quality: c_int) -> HeifError {
        HeifError { code: -1, subcode: 0, message: ptr::null() }
    }
    pub unsafe fn heif_encoder_set_lossless(_encoder: *mut HeifEncoder, _lossless: c_int) -> HeifError {
        HeifError { code: -1, subcode: 0, message: ptr::null() }
    }
    pub unsafe fn heif_encoder_release(_encoder: *mut HeifEncoder) {}
    pub unsafe fn heif_context_encode_image(
        _ctx: *mut HeifContext, _img: *const HeifImage, _encoder: *mut HeifEncoder,
        _options: *const HeifEncodingOptions, _out_handle: *mut *mut HeifImageHandle,
    ) -> HeifError { HeifError { code: -1, subcode: 0, message: ptr::null() } }
    pub unsafe fn heif_get_version() -> *const c_char { ptr::null() }
}

#[cfg(not(feature = "heif"))]
use stubs::*;

/// Decoded HEIC image data
#[derive(Debug)]
pub struct DecodedHeicImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub has_alpha: bool,
}

/// HEIC encoder configuration
#[derive(Debug, Clone)]
pub struct HeicEncoderConfig {
    /// Quality for lossy encoding (1-100, higher is better)
    pub quality: u8,
    /// Use lossless compression
    pub lossless: bool,
    /// Compression format (HEVC for HEIC, AV1 for AVIF)
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

/// HEIC Codec using libheif (encode and decode)
pub struct HeicCodec {
    ctx: *mut HeifContext,
}

impl HeicCodec {
    /// Create a new HEIC codec
    pub fn new() -> Result<Self> {
        let ctx = unsafe { heif_context_alloc() };
        if ctx.is_null() {
            return Err(anyhow!("Failed to create HEIF context - libheif may not be available"));
        }
        Ok(Self { ctx })
    }

    /// Check if libheif is available
    pub fn is_available() -> bool {
        #[cfg(feature = "heif")]
        {
            unsafe {
                let ctx = heif_context_alloc();
                if ctx.is_null() {
                    return false;
                }
                heif_context_free(ctx);
                true
            }
        }
        #[cfg(not(feature = "heif"))]
        {
            false
        }
    }

    /// Get libheif version
    pub fn get_version() -> Option<String> {
        unsafe {
            let ver = heif_get_version();
            if ver.is_null() {
                return None;
            }
            Some(CStr::from_ptr(ver).to_string_lossy().into_owned())
        }
    }

    /// Decode a HEIC/HEIF file to RGBA data
    pub fn decode_file(&self, path: &Path) -> Result<DecodedHeicImage> {
        let path_str = path.to_string_lossy();
        let path_cstr = CString::new(path_str.as_ref())?;

        unsafe {
            // Read the file
            let err = heif_context_read_from_file(self.ctx, path_cstr.as_ptr(), ptr::null());
            if err.code != 0 {
                let msg = Self::error_message(&err);
                return Err(anyhow!("Failed to read HEIC file: {}", msg));
            }

            // Get primary image handle
            let mut handle: *mut HeifImageHandle = ptr::null_mut();
            let err = heif_context_get_primary_image_handle(self.ctx, &mut handle);
            if err.code != 0 || handle.is_null() {
                let msg = Self::error_message(&err);
                return Err(anyhow!("Failed to get image handle: {}", msg));
            }

            // Get image dimensions
            let width = heif_image_handle_get_width(handle) as u32;
            let height = heif_image_handle_get_height(handle) as u32;
            let has_alpha = heif_image_handle_has_alpha_channel(handle) != 0;

            // Decode to RGB/RGBA
            let mut img: *mut HeifImage = ptr::null_mut();
            let chroma = if has_alpha {
                HeifChroma::InterleavedRGBA
            } else {
                HeifChroma::InterleavedRGB
            };

            let err = heif_decode_image(handle, &mut img, HeifColorspace::RGB, chroma, ptr::null());

            if err.code != 0 || img.is_null() {
                heif_image_handle_release(handle);
                let msg = Self::error_message(&err);
                return Err(anyhow!("Failed to decode image: {}", msg));
            }

            // Get pixel data
            let mut stride: c_int = 0;
            let data_ptr = heif_image_get_plane_readonly(img, HeifChannel::Interleaved, &mut stride);

            if data_ptr.is_null() {
                heif_image_release(img);
                heif_image_handle_release(handle);
                return Err(anyhow!("Failed to get image data"));
            }

            // Copy data to Vec
            let bytes_per_pixel = if has_alpha { 4 } else { 3 };
            let row_bytes = width as usize * bytes_per_pixel;
            let mut data = Vec::with_capacity(height as usize * row_bytes);

            for y in 0..height as isize {
                let row_ptr = data_ptr.offset(y * stride as isize);
                let row = std::slice::from_raw_parts(row_ptr, row_bytes);
                data.extend_from_slice(row);
            }

            // Clean up
            heif_image_release(img);
            heif_image_handle_release(handle);

            Ok(DecodedHeicImage { width, height, data, has_alpha })
        }
    }

    /// Encode RGB/RGBA data to HEIC file
    pub fn encode_to_file(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        has_alpha: bool,
        output_path: &Path,
        config: &HeicEncoderConfig,
    ) -> Result<()> {
        let output_cstr = CString::new(output_path.to_string_lossy().as_ref())?;

        unsafe {
            // Create a new context for encoding
            let enc_ctx = heif_context_alloc();
            if enc_ctx.is_null() {
                return Err(anyhow!("Failed to create encoding context"));
            }

            // Create image
            let chroma = if has_alpha {
                HeifChroma::InterleavedRGBA
            } else {
                HeifChroma::InterleavedRGB
            };

            let mut img: *mut HeifImage = ptr::null_mut();
            let err = heif_image_create(
                width as c_int,
                height as c_int,
                HeifColorspace::RGB,
                chroma,
                &mut img,
            );

            if err.code != 0 || img.is_null() {
                heif_context_free(enc_ctx);
                let msg = Self::error_message(&err);
                return Err(anyhow!("Failed to create image: {}", msg));
            }

            // Add plane
            let err = heif_image_add_plane(
                img,
                HeifChannel::Interleaved,
                width as c_int,
                height as c_int,
                8, // 8 bits per component
            );

            if err.code != 0 {
                heif_image_release(img);
                heif_context_free(enc_ctx);
                let msg = Self::error_message(&err);
                return Err(anyhow!("Failed to add image plane: {}", msg));
            }

            // Copy data to image
            let mut stride: c_int = 0;
            let plane_ptr = heif_image_get_plane(img, HeifChannel::Interleaved, &mut stride);

            if plane_ptr.is_null() {
                heif_image_release(img);
                heif_context_free(enc_ctx);
                return Err(anyhow!("Failed to get image plane"));
            }

            let bytes_per_pixel = if has_alpha { 4 } else { 3 };
            let row_bytes = width as usize * bytes_per_pixel;

            for y in 0..height as usize {
                let src_offset = y * row_bytes;
                let dst_ptr = plane_ptr.offset((y as isize) * (stride as isize));
                ptr::copy_nonoverlapping(data[src_offset..].as_ptr(), dst_ptr, row_bytes);
            }

            // Get encoder
            let mut encoder: *mut HeifEncoder = ptr::null_mut();
            let err = heif_context_get_encoder_for_format(enc_ctx, config.format, &mut encoder);

            if err.code != 0 || encoder.is_null() {
                heif_image_release(img);
                heif_context_free(enc_ctx);
                let msg = Self::error_message(&err);
                return Err(anyhow!("Failed to get encoder: {}", msg));
            }

            // Set quality
            if config.lossless {
                heif_encoder_set_lossless(encoder, 1);
            } else {
                heif_encoder_set_lossy_quality(encoder, config.quality as c_int);
            }

            // Encode
            let mut out_handle: *mut HeifImageHandle = ptr::null_mut();
            let err = heif_context_encode_image(enc_ctx, img, encoder, ptr::null(), &mut out_handle);

            heif_encoder_release(encoder);
            heif_image_release(img);

            if err.code != 0 {
                heif_context_free(enc_ctx);
                let msg = Self::error_message(&err);
                return Err(anyhow!("Failed to encode image: {}", msg));
            }

            if !out_handle.is_null() {
                heif_image_handle_release(out_handle);
            }

            // Write to file
            let err = heif_context_write_to_file(enc_ctx, output_cstr.as_ptr());
            heif_context_free(enc_ctx);

            if err.code != 0 {
                let msg = Self::error_message(&err);
                return Err(anyhow!("Failed to write HEIC file: {}", msg));
            }

            Ok(())
        }
    }

    /// Decode HEIC and save as PNG (lossless intermediate format)
    pub fn decode_to_png(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        let decoded = self.decode_file(input_path)?;

        let color_type = if decoded.has_alpha {
            image::ColorType::Rgba8
        } else {
            image::ColorType::Rgb8
        };

        image::save_buffer(output_path, &decoded.data, decoded.width, decoded.height, color_type)?;
        Ok(())
    }

    /// Decode HEIC and save as JPEG
    pub fn decode_to_jpeg(&self, input_path: &Path, output_path: &Path, quality: u8) -> Result<()> {
        let decoded = self.decode_file(input_path)?;

        // Convert RGBA to RGB if needed (JPEG doesn't support alpha)
        let rgb_data = if decoded.has_alpha {
            let mut rgb = Vec::with_capacity(decoded.width as usize * decoded.height as usize * 3);
            for chunk in decoded.data.chunks(4) {
                rgb.push(chunk[0]);
                rgb.push(chunk[1]);
                rgb.push(chunk[2]);
            }
            rgb
        } else {
            decoded.data
        };

        let img = image::RgbImage::from_raw(decoded.width, decoded.height, rgb_data)
            .ok_or_else(|| anyhow!("Failed to create image buffer"))?;

        let mut output_file = std::fs::File::create(output_path)?;
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output_file, quality);
        img.write_with_encoder(encoder)?;
        Ok(())
    }

    /// Encode PNG to HEIC
    pub fn png_to_heic(&self, input_path: &Path, output_path: &Path, config: &HeicEncoderConfig) -> Result<()> {
        let img = image::open(input_path)?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        self.encode_to_file(rgba.as_raw(), width, height, true, output_path, config)
    }

    /// Helper to get error message
    fn error_message(err: &HeifError) -> String {
        if err.message.is_null() {
            format!("Error code: {}", err.code)
        } else {
            unsafe { CStr::from_ptr(err.message).to_string_lossy().into_owned() }
        }
    }
}

impl Drop for HeicCodec {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { heif_context_free(self.ctx); }
        }
    }
}

unsafe impl Send for HeicCodec {}
unsafe impl Sync for HeicCodec {}

// Legacy type alias for backward compatibility
pub type HeicDecoder = HeicCodec;

/// Check if a file is a HEIC/HEIF file by extension
pub fn is_heic_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let lower = e.to_lowercase();
            lower == "heic" || lower == "heif" || lower == "hif"
        })
        .unwrap_or(false)
}

/// Decode a HEIC file to raw RGBA data (convenience function)
pub fn decode_heic_file(path: &Path) -> Result<DecodedHeicImage> {
    let codec = HeicCodec::new()?;
    codec.decode_file(path)
}

/// Decode HEIC to PNG (convenience function)
pub fn heic_to_png(input: &Path, output: &Path) -> Result<()> {
    let codec = HeicCodec::new()?;
    codec.decode_to_png(input, output)
}

/// Decode HEIC to JPEG (convenience function)
pub fn heic_to_jpeg(input: &Path, output: &Path, quality: u8) -> Result<()> {
    let codec = HeicCodec::new()?;
    codec.decode_to_jpeg(input, output, quality)
}

/// Encode PNG to HEIC (convenience function)
pub fn png_to_heic(input: &Path, output: &Path, quality: u8) -> Result<()> {
    let codec = HeicCodec::new()?;
    let config = HeicEncoderConfig {
        quality,
        lossless: false,
        format: HeifCompressionFormat::HEVC,
    };
    codec.png_to_heic(input, output, &config)
}

/// Encode PNG to HEIC losslessly (convenience function)
pub fn png_to_heic_lossless(input: &Path, output: &Path) -> Result<()> {
    let codec = HeicCodec::new()?;
    let config = HeicEncoderConfig {
        quality: 100,
        lossless: true,
        format: HeifCompressionFormat::HEVC,
    };
    codec.png_to_heic(input, output, &config)
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
        let available = HeicCodec::is_available();
        println!("libheif available: {}", available);

        if let Some(ver) = HeicCodec::get_version() {
            println!("libheif version: {}", ver);
        }
    }
}
