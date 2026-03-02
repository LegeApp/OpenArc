// Native BPG Library FFI Bindings
// Direct integration with libbpg (no subprocess)

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr;

// Opaque decoder context
#[repr(C)]
pub struct BPGDecoderContext {
    _private: [u8; 0],
}

// Opaque encoder context
#[repr(C)]
pub struct BPGEncoderContext {
    _private: [u8; 0],
}

// Image info structure
#[repr(C)]
pub struct BPGImageInfo {
    pub width: u32,
    pub height: u32,
    pub format: u8,              // BPGImageFormatEnum
    pub has_alpha: u8,
    pub color_space: u8,         // BPGColorSpaceEnum
    pub bit_depth: u8,
    pub premultiplied_alpha: u8,
    pub has_w_plane: u8,
    pub limited_range: u8,
    pub has_animation: u8,
    pub loop_count: u16,
}

// Extension data structure
#[repr(C)]
pub struct BPGExtensionData {
    pub tag: c_int,      // 1=EXIF, 2=ICC, 3=XMP, 4=THUMBNAIL, 5=ANIM
    pub buf: *mut u8,
    pub len: c_int,
    pub next: *mut BPGExtensionData,
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

// Decoder output format
#[repr(C)]
pub enum BPGDecoderOutputFormat {
    RGB24 = 0,
    RGBA32,
    RGB48,
    RGBA64,
    CMYK32,
    CMYK64,
}

// FFI declarations - using the newer API that's available in the library
extern "C" {
    // Decoder functions (newer API)
    pub fn bpg_decoder_open() -> *mut BPGDecoderContext;
    pub fn bpg_decoder_keep_extension_data(s: *mut BPGDecoderContext, enable: c_int);
    pub fn bpg_decoder_decode(s: *mut BPGDecoderContext, buf: *const u8, buf_len: c_int) -> c_int;
    pub fn bpg_decoder_get_info(s: *mut BPGDecoderContext, p: *mut BPGImageInfo) -> c_int;
    pub fn bpg_decoder_start(s: *mut BPGDecoderContext, out_fmt: BPGDecoderOutputFormat) -> c_int;
    pub fn bpg_decoder_get_frame_duration(s: *mut BPGDecoderContext, pnum: *mut c_int, pden: *mut c_int);
    pub fn bpg_decoder_get_line(s: *mut BPGDecoderContext, buf: *mut c_void) -> c_int;
    pub fn bpg_decoder_close(s: *mut BPGDecoderContext);
    pub fn bpg_decoder_get_data(s: *mut BPGDecoderContext, pline_size: *mut c_int, plane: c_int) -> *mut u8;
    pub fn bpg_decoder_get_info_from_buf(
        p: *mut BPGImageInfo,
        pfirst_md: *mut *mut c_void,  // Changed to void pointer to avoid defining ExtensionData here
        buf: *const u8,
        buf_len: c_int,
    ) -> c_int;
    pub fn bpg_decoder_free_extension_data(first_md: *mut c_void);  // Changed to void pointer
    pub fn bpg_decoder_get_extension_data(s: *mut BPGDecoderContext, pfirst_md: *mut *mut BPGExtensionData) -> c_int;

    // Encoder functions are NOT available in the in-memory-only modified BPG library
    // Commenting them out to avoid linker errors
    // pub fn bpg_encoder_create() -> *mut BPGEncoderContext;
    // pub fn bpg_encoder_create_ex(config: *const BPGEncoderConfig) -> *mut BPGEncoderContext;
    // pub fn bpg_encoder_set_config(ctx: *mut BPGEncoderContext, config: *const BPGEncoderConfig) -> c_int;
    // pub fn bpg_encoder_get_default_config(config: *mut BPGEncoderConfig);
    // pub fn bpg_encoder_get_error(ctx: *mut BPGEncoderContext) -> *const c_char;
    // pub fn bpg_encoder_destroy(ctx: *mut BPGEncoderContext);

    // bpg_free is also not available in this modified library
    // pub fn bpg_free(ptr: *mut c_void);
}

// Safe helper functions
// Note: bpg_free is not available in the modified library
// pub unsafe fn free_bpg_memory(ptr: *mut u8) {
//     if !ptr.is_null() {
//         bpg_free(ptr as *mut c_void);
//     }
// }
