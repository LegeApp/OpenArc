// Native BPG Library FFI Bindings
// Direct integration with libbpg_native.a (no subprocess)

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use anyhow::{Result, anyhow};

// Opaque encoder context
#[repr(C)]
pub struct BPGEncoderContext {
    _private: [u8; 0],
}

// Encoder configuration
#[repr(C)]
#[derive(Debug, Clone)]
pub struct BPGEncoderConfig {
    pub quality: c_int,
    pub bit_depth: c_int,
    pub lossless: c_int,
    pub chroma_format: c_int,
    pub encoder_type: c_int,
    pub compress_level: c_int,
}

// Error codes
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BPGError {
    Ok = 0,
    InvalidParam = -1,
    OutOfMemory = -2,
    UnsupportedFormat = -3,
    EncodeFailed = -4,
    DecodeFailed = -5,
    FileIO = -6,
    InvalidImage = -7,
}

// Image format
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BPGImageFormat {
    Gray = 0,
    RGB24,
    RGBA32,
    BGR24,
    BGRA32,
    YCbCr420P,   // Planar YCbCr 4:2:0 (JPEG native, no color conversion)
    YCbCr444P,   // Planar YCbCr 4:4:4 (no color conversion)
}

// FFI declarations
extern "C" {
    fn bpg_encoder_create() -> *mut BPGEncoderContext;
    fn bpg_encoder_create_ex(config: *const BPGEncoderConfig) -> *mut BPGEncoderContext;
    fn bpg_encoder_set_config(ctx: *mut BPGEncoderContext, config: *const BPGEncoderConfig) -> c_int;
    fn bpg_encoder_get_default_config(config: *mut BPGEncoderConfig);
    
    fn bpg_encode_from_file(
        ctx: *mut BPGEncoderContext,
        input_path: *const c_char,
        output_data: *mut *mut u8,
        output_size: *mut usize,
    ) -> c_int;
    
    fn bpg_encode_from_memory(
        ctx: *mut BPGEncoderContext,
        input_data: *const u8,
        width: c_int,
        height: c_int,
        stride: c_int,
        format: BPGImageFormat,
        output_data: *mut *mut u8,
        output_size: *mut usize,
    ) -> c_int;
    
    fn bpg_encode_to_file(
        ctx: *mut BPGEncoderContext,
        input_path: *const c_char,
        output_path: *const c_char,
    ) -> c_int;
    
    fn bpg_encoder_get_error(ctx: *mut BPGEncoderContext) -> *const c_char;
    fn bpg_encoder_destroy(ctx: *mut BPGEncoderContext);
    
    fn bpg_decode_file(
        input_path: *const c_char,
        output_data: *mut *mut u8,
        width: *mut c_int,
        height: *mut c_int,
        format: *mut BPGImageFormat,
    ) -> c_int;
    
    fn bpg_free(ptr: *mut c_void);
    fn bpg_get_version() -> *const c_char;
    fn bpg_get_supported_encoders() -> c_int;
}

// Safe Rust wrapper
pub struct NativeBPGEncoder {
    ctx: *mut BPGEncoderContext,
}

impl NativeBPGEncoder {
    /// Create encoder with default configuration
    pub fn new() -> Result<Self> {
        let ctx = unsafe { bpg_encoder_create() };
        if ctx.is_null() {
            return Err(anyhow!("Failed to create BPG encoder"));
        }
        Ok(Self { ctx })
    }
    
    /// Create encoder with custom quality
    pub fn with_quality(quality: u8) -> Result<Self> {
        let mut config = Self::default_config();
        config.quality = quality as c_int;
        
        let ctx = unsafe { bpg_encoder_create_ex(&config) };
        if ctx.is_null() {
            return Err(anyhow!("Failed to create BPG encoder"));
        }
        Ok(Self { ctx })
    }
    
    /// Get default configuration
    pub fn default_config() -> BPGEncoderConfig {
        let mut config = BPGEncoderConfig {
            quality: 28,
            bit_depth: 8,
            lossless: 0,
            chroma_format: 1,
            encoder_type: 0,
            compress_level: 8,
        };
        unsafe {
            bpg_encoder_get_default_config(&mut config);
        }
        config
    }
    
    /// Set encoder configuration
    pub fn set_config(&mut self, config: &BPGEncoderConfig) -> Result<()> {
        let result = unsafe { bpg_encoder_set_config(self.ctx, config) };
        if result != 0 {
            return Err(anyhow!("Failed to set config: {}", self.get_error()));
        }
        Ok(())
    }
    
    /// Encode image file to BPG (returns encoded data)
    pub fn encode_from_file(&self, input_path: &str) -> Result<Vec<u8>> {
        let input_cstr = CString::new(input_path)?;
        let mut output_data: *mut u8 = ptr::null_mut();
        let mut output_size: usize = 0;
        
        let result = unsafe {
            bpg_encode_from_file(
                self.ctx,
                input_cstr.as_ptr(),
                &mut output_data,
                &mut output_size,
            )
        };
        
        if result != 0 {
            return Err(anyhow!("Encoding failed: {}", self.get_error()));
        }
        
        if output_data.is_null() || output_size == 0 {
            return Err(anyhow!("Encoding produced no output"));
        }
        
        // Copy data to Vec and free C-allocated memory
        let data = unsafe {
            let slice = std::slice::from_raw_parts(output_data, output_size);
            let vec = slice.to_vec();
            bpg_free(output_data as *mut c_void);
            vec
        };
        
        Ok(data)
    }
    
    /// Encode image file to BPG file
    pub fn encode_to_file(&self, input_path: &str, output_path: &str) -> Result<()> {
        let input_cstr = CString::new(input_path)?;
        let output_cstr = CString::new(output_path)?;
        
        let result = unsafe {
            bpg_encode_to_file(
                self.ctx,
                input_cstr.as_ptr(),
                output_cstr.as_ptr(),
            )
        };
        
        if result != 0 {
            return Err(anyhow!("Encoding failed: {}", self.get_error()));
        }
        
        Ok(())
    }
    
    /// Encode raw image data to BPG
    pub fn encode_from_memory(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        format: BPGImageFormat,
    ) -> Result<Vec<u8>> {
        let mut output_data: *mut u8 = ptr::null_mut();
        let mut output_size: usize = 0;
        
        let result = unsafe {
            bpg_encode_from_memory(
                self.ctx,
                data.as_ptr(),
                width as c_int,
                height as c_int,
                stride as c_int,
                format,
                &mut output_data,
                &mut output_size,
            )
        };
        
        if result != 0 {
            return Err(anyhow!("Encoding failed: {}", self.get_error()));
        }
        
        if output_data.is_null() || output_size == 0 {
            return Err(anyhow!("Encoding produced no output"));
        }
        
        // Copy data to Vec and free C-allocated memory
        let data = unsafe {
            let slice = std::slice::from_raw_parts(output_data, output_size);
            let vec = slice.to_vec();
            bpg_free(output_data as *mut c_void);
            vec
        };
        
        Ok(data)
    }
    
    /// Get last error message
    fn get_error(&self) -> String {
        unsafe {
            let err_ptr = bpg_encoder_get_error(self.ctx);
            if err_ptr.is_null() {
                return "Unknown error".to_string();
            }
            CStr::from_ptr(err_ptr)
                .to_string_lossy()
                .into_owned()
        }
    }
}

impl Drop for NativeBPGEncoder {
    fn drop(&mut self) {
        unsafe {
            bpg_encoder_destroy(self.ctx);
        }
    }
}

unsafe impl Send for NativeBPGEncoder {}
unsafe impl Sync for NativeBPGEncoder {}

// Decoder functions
pub fn decode_file(input_path: &str) -> Result<(Vec<u8>, u32, u32, BPGImageFormat)> {
    let input_cstr = CString::new(input_path)?;
    let mut output_data: *mut u8 = ptr::null_mut();
    let mut width: c_int = 0;
    let mut height: c_int = 0;
    let mut format = BPGImageFormat::RGBA32;
    
    let result = unsafe {
        bpg_decode_file(
            input_cstr.as_ptr(),
            &mut output_data,
            &mut width,
            &mut height,
            &mut format,
        )
    };
    
    if result != 0 {
        return Err(anyhow!("Decoding failed with error code: {}", result));
    }
    
    if output_data.is_null() || width == 0 || height == 0 {
        return Err(anyhow!("Decoding produced no output"));
    }
    
    // Calculate size and copy data
    let size = (width * height * 4) as usize;  // RGBA32
    let data = unsafe {
        let slice = std::slice::from_raw_parts(output_data, size);
        let vec = slice.to_vec();
        bpg_free(output_data as *mut c_void);
        vec
    };
    
    Ok((data, width as u32, height as u32, format))
}

// Utility functions
pub fn get_version() -> String {
    unsafe {
        let ver_ptr = bpg_get_version();
        if ver_ptr.is_null() {
            return "unknown".to_string();
        }
        CStr::from_ptr(ver_ptr)
            .to_string_lossy()
            .into_owned()
    }
}

pub fn get_supported_encoders() -> i32 {
    unsafe { bpg_get_supported_encoders() }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encoder_creation() {
        let encoder = NativeBPGEncoder::new();
        assert!(encoder.is_ok());
    }
    
    #[test]
    fn test_version() {
        let version = get_version();
        assert!(!version.is_empty());
        println!("BPG version: {}", version);
    }
    
    #[test]
    fn test_supported_encoders() {
        let encoders = get_supported_encoders();
        assert!(encoders & 0x01 != 0);  // x265 should be supported
    }
}
