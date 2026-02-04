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

// RAII wrappers for automatic cleanup
struct ScopedImageHandle(*mut HeifImageHandle);
impl Drop for ScopedImageHandle {
    fn drop(&mut self) {
        unsafe { 
            if !self.0.is_null() { heif_image_handle_release(self.0); } 
        }
    }
}

struct ScopedImage(*mut HeifImage);
impl Drop for ScopedImage {
    fn drop(&mut self) {
        unsafe { 
            if !self.0.is_null() { heif_image_release(self.0); } 
        }
    }
}

struct ScopedEncoder(*mut HeifEncoder);
impl Drop for ScopedEncoder {
    fn drop(&mut self) {
        unsafe { 
            if !self.0.is_null() { heif_encoder_release(self.0); } 
        }
    }
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
    fn heif_context_read_from_memory_without_copy(
        ctx: *mut HeifContext,
        data: *const c_void,
        size: usize,
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
    pub unsafe fn heif_context_read_from_memory_without_copy(
        _ctx: *mut HeifContext, _data: *const c_void, _size: usize, _options: *const c_void,
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

/// Decoded HEIC image in YCbCr planar format (for BPG encoding)
#[derive(Debug)]
pub struct DecodedHeicYCbCr {
    pub width: u32,
    pub height: u32,
    pub y_plane: Vec<u8>,
    pub cb_plane: Vec<u8>,
    pub cr_plane: Vec<u8>,
    pub y_stride: u32,
    pub cb_stride: u32,
    pub cr_stride: u32,
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
    pub fn decode_file(&mut self, path: &Path) -> Result<DecodedHeicImage> {
        let path_str = path.to_string_lossy();
        let path_cstr = CString::new(path_str.as_ref())?;

        unsafe {
            // Read the file
            let err = heif_context_read_from_file(self.ctx, path_cstr.as_ptr(), ptr::null());
            if err.code != 0 {
                return Err(anyhow!("Failed to read HEIC file: {}", Self::error_message(&err)));
            }

            self.decode_primary_image()
        }
    }

    /// Decode from memory buffer
    pub fn decode_from_memory(&mut self, buffer: &[u8]) -> Result<DecodedHeicImage> {
        unsafe {
            let err = heif_context_read_from_memory_without_copy(
                self.ctx,
                buffer.as_ptr() as *const c_void,
                buffer.len(),
                ptr::null()
            );
            
            if err.code != 0 {
                return Err(anyhow!("Failed to read HEIC from memory: {}", Self::error_message(&err)));
            }
            
            self.decode_primary_image()
        }
    }

    /// Internal: Decode the primary image from the loaded context
    fn decode_primary_image(&self) -> Result<DecodedHeicImage> {
        unsafe {
            // Get primary image handle
            let mut handle_ptr: *mut HeifImageHandle = ptr::null_mut();
            let err = heif_context_get_primary_image_handle(self.ctx, &mut handle_ptr);
            if err.code != 0 {
                return Err(anyhow!("Failed to get handle: {}", Self::error_message(&err)));
            }
            // Wrap immediately for automatic cleanup
            let handle = ScopedImageHandle(handle_ptr);

            // Get image dimensions
            let width = heif_image_handle_get_width(handle.0) as u32;
            let height = heif_image_handle_get_height(handle.0) as u32;
            let has_alpha = heif_image_handle_has_alpha_channel(handle.0) != 0;

            // Decode to RGB/RGBA
            let mut img_ptr: *mut HeifImage = ptr::null_mut();
            let chroma = if has_alpha {
                HeifChroma::InterleavedRGBA
            } else {
                HeifChroma::InterleavedRGB
            };

            let err = heif_decode_image(handle.0, &mut img_ptr, HeifColorspace::RGB, chroma, ptr::null());
            if err.code != 0 {
                return Err(anyhow!("Failed to decode: {}", Self::error_message(&err)));
            }
            // Wrap immediately for automatic cleanup
            let img = ScopedImage(img_ptr);

            // Get pixel data
            let mut stride: c_int = 0;
            let data_ptr = heif_image_get_plane_readonly(img.0, HeifChannel::Interleaved, &mut stride);

            if data_ptr.is_null() {
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

            // RAII wrappers handle cleanup automatically
            Ok(DecodedHeicImage { width, height, data, has_alpha })
        }
    }

    /// Decode a HEIC/HEIF file to YCbCr 4:2:0 planar format (optimal for BPG encoding)
    pub fn decode_file_ycbcr420(&mut self, path: &Path) -> Result<DecodedHeicYCbCr> {
        let path_str = path.to_string_lossy();
        let path_cstr = CString::new(path_str.as_ref())?;

        unsafe {
            // Read the file
            let err = heif_context_read_from_file(self.ctx, path_cstr.as_ptr(), ptr::null());
            if err.code != 0 {
                return Err(anyhow!("Failed to read HEIC file: {}", Self::error_message(&err)));
            }

            // Get primary image handle
            let mut handle_ptr: *mut HeifImageHandle = ptr::null_mut();
            let err = heif_context_get_primary_image_handle(self.ctx, &mut handle_ptr);
            if err.code != 0 {
                return Err(anyhow!("Failed to get handle: {}", Self::error_message(&err)));
            }
            // Wrap immediately for automatic cleanup
            let handle = ScopedImageHandle(handle_ptr);

            // Get image dimensions
            let width = heif_image_handle_get_width(handle.0) as u32;
            let height = heif_image_handle_get_height(handle.0) as u32;

            // Decode to YCbCr 4:2:0 (native HEIC colorspace, no conversion)
            let mut img_ptr: *mut HeifImage = ptr::null_mut();
            let err = heif_decode_image(
                handle.0,
                &mut img_ptr,
                HeifColorspace::YCbCr,
                HeifChroma::Chroma420,
                ptr::null()
            );

            if err.code != 0 {
                return Err(anyhow!("Failed to decode image to YCbCr: {}", Self::error_message(&err)));
            }
            // Wrap immediately for automatic cleanup
            let img = ScopedImage(img_ptr);

            // Get Y plane
            let mut y_stride: c_int = 0;
            let y_ptr = heif_image_get_plane_readonly(img.0, HeifChannel::Y, &mut y_stride);
            if y_ptr.is_null() {
                return Err(anyhow!("Failed to get Y plane"));
            }

            // Get Cb plane (subsampled 2x2)
            let mut cb_stride: c_int = 0;
            let cb_ptr = heif_image_get_plane_readonly(img.0, HeifChannel::Cb, &mut cb_stride);
            if cb_ptr.is_null() {
                return Err(anyhow!("Failed to get Cb plane"));
            }

            // Get Cr plane (subsampled 2x2)
            let mut cr_stride: c_int = 0;
            let cr_ptr = heif_image_get_plane_readonly(img.0, HeifChannel::Cr, &mut cr_stride);
            if cr_ptr.is_null() {
                return Err(anyhow!("Failed to get Cr plane"));
            }

            // Copy Y plane (full resolution)
            let mut y_plane = Vec::with_capacity(height as usize * y_stride as usize);
            for row in 0..height as isize {
                let row_ptr = y_ptr.offset(row * y_stride as isize);
                let row_slice = std::slice::from_raw_parts(row_ptr, width as usize);
                y_plane.extend_from_slice(row_slice);
            }

            // Copy Cb plane (half resolution)
            let chroma_width = (width + 1) / 2;
            let chroma_height = (height + 1) / 2;
            let mut cb_plane = Vec::with_capacity(chroma_height as usize * cb_stride as usize);
            for row in 0..chroma_height as isize {
                let row_ptr = cb_ptr.offset(row * cb_stride as isize);
                let row_slice = std::slice::from_raw_parts(row_ptr, chroma_width as usize);
                cb_plane.extend_from_slice(row_slice);
            }

            // Copy Cr plane (half resolution)
            let mut cr_plane = Vec::with_capacity(chroma_height as usize * cr_stride as usize);
            for row in 0..chroma_height as isize {
                let row_ptr = cr_ptr.offset(row * cr_stride as isize);
                let row_slice = std::slice::from_raw_parts(row_ptr, chroma_width as usize);
                cr_plane.extend_from_slice(row_slice);
            }

            // RAII wrappers handle cleanup automatically
            Ok(DecodedHeicYCbCr {
                width,
                height,
                y_plane,
                cb_plane,
                cr_plane,
                y_stride: width,
                cb_stride: chroma_width,
                cr_stride: chroma_width,
            })
        }
    }

    /// Encode RGB/RGBA data to HEIC file
    pub fn encode_to_file(
        &mut self,
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

            let mut img_ptr: *mut HeifImage = ptr::null_mut();
            let err = heif_image_create(
                width as c_int,
                height as c_int,
                HeifColorspace::RGB,
                chroma,
                &mut img_ptr,
            );

            if err.code != 0 {
                heif_context_free(enc_ctx);
                return Err(anyhow!("Failed to create image: {}", Self::error_message(&err)));
            }
            // Wrap immediately for automatic cleanup
            let img = ScopedImage(img_ptr);

            // Add plane
            let err = heif_image_add_plane(
                img.0,
                HeifChannel::Interleaved,
                width as c_int,
                height as c_int,
                8, // 8 bits per component
            );

            if err.code != 0 {
                heif_context_free(enc_ctx);
                return Err(anyhow!("Failed to add image plane: {}", Self::error_message(&err)));
            }

            // Copy data to image
            let mut stride: c_int = 0;
            let plane_ptr = heif_image_get_plane(img.0, HeifChannel::Interleaved, &mut stride);

            if plane_ptr.is_null() {
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
            let mut encoder_ptr: *mut HeifEncoder = ptr::null_mut();
            let err = heif_context_get_encoder_for_format(enc_ctx, config.format, &mut encoder_ptr);

            if err.code != 0 {
                heif_context_free(enc_ctx);
                return Err(anyhow!("Failed to get encoder: {}", Self::error_message(&err)));
            }
            // Wrap immediately for automatic cleanup
            let encoder = ScopedEncoder(encoder_ptr);

            // Set quality
            if config.lossless {
                heif_encoder_set_lossless(encoder.0, 1);
            } else {
                heif_encoder_set_lossy_quality(encoder.0, config.quality as c_int);
            }

            // Encode
            let mut out_handle_ptr: *mut HeifImageHandle = ptr::null_mut();
            let err = heif_context_encode_image(enc_ctx, img.0, encoder.0, ptr::null(), &mut out_handle_ptr);

            if err.code != 0 {
                heif_context_free(enc_ctx);
                return Err(anyhow!("Failed to encode image: {}", Self::error_message(&err)));
            }

            // Wrap output handle for cleanup
            let _out_handle = ScopedImageHandle(out_handle_ptr);

            // Write to file
            let err = heif_context_write_to_file(enc_ctx, output_cstr.as_ptr());
            heif_context_free(enc_ctx);

            if err.code != 0 {
                return Err(anyhow!("Failed to write HEIC file: {}", Self::error_message(&err)));
            }

            // RAII wrappers handle cleanup automatically
            Ok(())
        }
    }

    /// Decode HEIC and save as PNG (lossless intermediate format)
    pub fn decode_to_png(&mut self, input_path: &Path, output_path: &Path) -> Result<()> {
        let decoded = self.decode_file(input_path)?;

        // Create proper ImageBuffer from decoded data
        // libheif returns RGB/RGBA in correct order, but we need to use image crate properly
        use image::{ImageBuffer, Rgb, Rgba, DynamicImage};
        
        let img = if decoded.has_alpha {
            let rgba_buf = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                decoded.width,
                decoded.height,
                decoded.data
            ).ok_or_else(|| anyhow!("Failed to create RGBA image buffer"))?;
            DynamicImage::ImageRgba8(rgba_buf)
        } else {
            let rgb_buf = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(
                decoded.width,
                decoded.height,
                decoded.data
            ).ok_or_else(|| anyhow!("Failed to create RGB image buffer"))?;
            DynamicImage::ImageRgb8(rgb_buf)
        };
        
        img.save(output_path)?;
        Ok(())
    }

    /// Decode HEIC and save as JPEG
    pub fn decode_to_jpeg(&mut self, input_path: &Path, output_path: &Path, quality: u8) -> Result<()> {
        let decoded = self.decode_file(input_path)?;

        // Zero-copy view of the raw buffer using image crate
        let dynamic_img = if decoded.has_alpha {
            image::DynamicImage::ImageRgba8(
                image::ImageBuffer::from_raw(decoded.width, decoded.height, decoded.data)
                    .ok_or_else(|| anyhow!("Invalid buffer size"))?
            )
        } else {
            image::DynamicImage::ImageRgb8(
                image::ImageBuffer::from_raw(decoded.width, decoded.height, decoded.data)
                    .ok_or_else(|| anyhow!("Invalid buffer size"))?
            )
        };

        // Fast intrinsic conversion to RGB8 (strips alpha efficiently)
        let rgb_img = dynamic_img.into_rgb8();

        let mut output_file = std::fs::File::create(output_path)?;
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output_file, quality);
        rgb_img.write_with_encoder(encoder)?;
        Ok(())
    }

    /// Encode PNG to HEIC
    pub fn png_to_heic(&mut self, input_path: &Path, output_path: &Path, config: &HeicEncoderConfig) -> Result<()> {
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

// Safe to send between threads, but not safe to share (&self methods are now &mut self)
unsafe impl Send for HeicCodec {}

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
    let mut codec = HeicCodec::new()?;
    codec.decode_file(path)
}

/// Decode HEIC to PNG (convenience function)
pub fn heic_to_png(input: &Path, output: &Path) -> Result<()> {
    let mut codec = HeicCodec::new()?;
    codec.decode_to_png(input, output)
}

/// Decode HEIC to JPEG (convenience function)
pub fn heic_to_jpeg(input: &Path, output: &Path, quality: u8) -> Result<()> {
    let mut codec = HeicCodec::new()?;
    codec.decode_to_jpeg(input, output, quality)
}

/// Encode PNG to HEIC (convenience function)
pub fn png_to_heic(input: &Path, output: &Path, quality: u8) -> Result<()> {
    let mut codec = HeicCodec::new()?;
    let config = HeicEncoderConfig {
        quality,
        lossless: false,
        format: HeifCompressionFormat::HEVC,
    };
    codec.png_to_heic(input, output, &config)
}

/// Encode PNG to HEIC losslessly (convenience function)
pub fn png_to_heic_lossless(input: &Path, output: &Path) -> Result<()> {
    let mut codec = HeicCodec::new()?;
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
