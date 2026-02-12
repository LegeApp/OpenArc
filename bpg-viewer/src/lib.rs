// BPG Viewer and Thumbnail Library
// Standalone FFI-capable library for BPG image viewing and thumbnail generation

pub mod ffi;
pub mod decoder;
pub mod encoder;           // Stub only - encoder disabled
pub mod image_data;        // Unified BGRA/YCbCr image data representation
pub mod thumbnail;
pub mod universal_thumbnail;
pub mod universal_decode;
pub mod pipeline;          // Clean Tokio async thumbnail pipeline
pub mod fullimage_loader;  // Async full-image loading

// Note: encoder is a stub (disabled), thumbnail_cpu functionality is now used directly via fast_image_resize

// Re-export main types
pub use decoder::{DecodedImage, decode_file, decode_memory};
pub use thumbnail::{ThumbnailGenerator, ThumbnailConfig};
pub use universal_thumbnail::UniversalThumbnailGenerator;
pub use ffi::{BPGImageFormat, BPGEncoderConfig};

// C FFI interface for embedding in other languages
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint};
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice;

/// Error codes for C FFI
#[repr(C)]
pub enum BPGViewerError {
    Success = 0,
    InvalidParam = -1,
    DecodeFailed = -2,
    EncodeFailed = -3,
    OutOfMemory = -4,
    IoError = -5,
}

/// Opaque handle to decoded image
pub struct BPGImageHandle {
    image: DecodedImage,
}

/// Opaque handle to thumbnail generator
pub struct BPGThumbnailHandle {
    generator: ThumbnailGenerator,
}

/// Opaque handle to universal thumbnail generator
pub struct UniversalThumbnailHandle {
    generator: universal_thumbnail::UniversalThumbnailGenerator,
}

/// Opaque handle to universally decoded image (full resolution BGRA)
pub struct UniversalImageHandle {
    image: universal_decode::UniversalDecodedImage,
    bgra_cache: std::sync::Mutex<Option<Vec<u8>>>, // Cached BGRA data for FFI
}

// C FFI Functions

/// Decode a BPG file and return a handle to the decoded image
/// Returns null on failure
#[no_mangle]
pub extern "C" fn bpg_viewer_decode_file(path: *const c_char) -> *mut BPGImageHandle {
    if path.is_null() {
        return ptr::null_mut();
    }

    let path_str = unsafe {
        match CStr::from_ptr(path).to_str() {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        }
    };

    match decode_file(path_str) {
        Ok(image) => Box::into_raw(Box::new(BPGImageHandle { image })),
        Err(_) => ptr::null_mut(),
    }
}

/// Get image dimensions from handle
#[no_mangle]
pub extern "C" fn bpg_viewer_get_dimensions(
    handle: *const BPGImageHandle,
    width: *mut c_uint,
    height: *mut c_uint,
) -> c_int {
    if handle.is_null() || width.is_null() || height.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };
    unsafe {
        *width = handle_ref.image.width;
        *height = handle_ref.image.height;
    }

    BPGViewerError::Success as c_int
}

/// Get image color space
#[no_mangle]
pub extern "C" fn bpg_viewer_get_color_space(
    handle: *const BPGImageHandle,
    color_space: *mut u8,
) -> c_int {
    if handle.is_null() || color_space.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };
    unsafe {
        *color_space = handle_ref.image.color_space;
    }

    BPGViewerError::Success as c_int
}

/// Decode directly to a provided buffer (e.g. WPF WriteableBitmap)
/// Performs color conversion (source -> sRGB) and format conversion (BGRA)
#[no_mangle]
pub extern "C" fn bpg_viewer_decode_to_buffer(
    handle: *const BPGImageHandle,
    buffer: *mut u8,
    buffer_size: usize,
    stride: usize,
) -> c_int {
    if handle.is_null() || buffer.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };
    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, buffer_size) };

    match handle_ref.image.copy_to_buffer(buffer_slice, stride) {
        Ok(_) => BPGViewerError::Success as c_int,
        Err(_) => BPGViewerError::DecodeFailed as c_int,
    }
}

/// Get EXIF data from image
#[no_mangle]
pub extern "C" fn bpg_viewer_get_exif_data(
    handle: *const BPGImageHandle,
    data: *mut *const u8,
    size: *mut usize,
) -> c_int {
    if handle.is_null() || data.is_null() || size.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };
    
    if let Some(ref exif) = handle_ref.image.exif_data {
        unsafe {
            *data = exif.as_ptr();
            *size = exif.len();
        }
    } else {
        unsafe {
            *data = ptr::null();
            *size = 0;
        }
    }

    BPGViewerError::Success as c_int
}

/// Get image data pointer and size
/// The returned pointer is valid as long as the handle exists
#[no_mangle]
pub extern "C" fn bpg_viewer_get_data(
    handle: *const BPGImageHandle,
    data: *mut *const u8,
    size: *mut usize,
) -> c_int {
    if handle.is_null() || data.is_null() || size.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };
    unsafe {
        *data = handle_ref.image.data.as_ptr();
        *size = handle_ref.image.data.len();
    }

    BPGViewerError::Success as c_int
}

/// Get RGBA32 data from image (performs conversion if needed)
/// Caller must free the returned pointer with bpg_viewer_free_buffer
#[no_mangle]
pub extern "C" fn bpg_viewer_get_rgba32(
    handle: *const BPGImageHandle,
    data: *mut *mut u8,
    size: *mut usize,
) -> c_int {
    if handle.is_null() || data.is_null() || size.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };

    match handle_ref.image.to_rgba32() {
        Ok(rgba_data) => {
            let len = rgba_data.len();
            let boxed = rgba_data.into_boxed_slice();
            let ptr = Box::into_raw(boxed) as *mut u8;

            unsafe {
                *data = ptr;
                *size = len;
            }
            BPGViewerError::Success as c_int
        }
        Err(_) => BPGViewerError::DecodeFailed as c_int,
    }
}

/// Get BGRA32 data from image (for WPF/Windows)
/// Caller must free the returned pointer with bpg_viewer_free_buffer
#[no_mangle]
pub extern "C" fn bpg_viewer_get_bgra32(
    handle: *const BPGImageHandle,
    data: *mut *mut u8,
    size: *mut usize,
) -> c_int {
    if handle.is_null() || data.is_null() || size.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };

    match handle_ref.image.to_bgra32() {
        Ok(bgra_data) => {
            let len = bgra_data.len();
            let boxed = bgra_data.into_boxed_slice();
            let ptr = Box::into_raw(boxed) as *mut u8;

            unsafe {
                *data = ptr;
                *size = len;
            }
            BPGViewerError::Success as c_int
        }
        Err(_) => BPGViewerError::DecodeFailed as c_int,
    }
}

/// Free buffer allocated by bpg_viewer_get_rgba32 or bpg_viewer_get_bgra32
#[no_mangle]
pub extern "C" fn bpg_viewer_free_buffer(ptr: *mut u8, size: usize) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(slice::from_raw_parts_mut(ptr, size));
    }
}

/// Free decoded image handle
#[no_mangle]
pub extern "C" fn bpg_viewer_free_image(handle: *mut BPGImageHandle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle);
        }
    }
}

/// Create a thumbnail generator with default settings
#[no_mangle]
pub extern "C" fn bpg_thumbnail_create() -> *mut BPGThumbnailHandle {
    let generator = ThumbnailGenerator::new();
    Box::into_raw(Box::new(BPGThumbnailHandle { generator }))
}

/// Create a thumbnail generator with specific dimensions
#[no_mangle]
pub extern "C" fn bpg_thumbnail_create_with_size(
    max_width: c_uint,
    max_height: c_uint,
) -> *mut BPGThumbnailHandle {
    let generator = ThumbnailGenerator::with_dimensions(max_width, max_height);
    Box::into_raw(Box::new(BPGThumbnailHandle { generator }))
}

/// Generate thumbnail and save as PNG
#[no_mangle]
pub extern "C" fn bpg_thumbnail_generate_png(
    handle: *const BPGThumbnailHandle,
    input_path: *const c_char,
    output_path: *const c_char,
) -> c_int {
    if handle.is_null() || input_path.is_null() || output_path.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };

    let input_str = unsafe {
        match CStr::from_ptr(input_path).to_str() {
            Ok(s) => s,
            Err(_) => return BPGViewerError::InvalidParam as c_int,
        }
    };

    let output_str = unsafe {
        match CStr::from_ptr(output_path).to_str() {
            Ok(s) => s,
            Err(_) => return BPGViewerError::InvalidParam as c_int,
        }
    };

    match handle_ref.generator.generate_thumbnail_to_png(
        std::path::Path::new(input_str),
        std::path::Path::new(output_str),
    ) {
        Ok(_) => BPGViewerError::Success as c_int,
        Err(_) => BPGViewerError::EncodeFailed as c_int,
    }
}

/// Free thumbnail generator handle
#[no_mangle]
pub extern "C" fn bpg_thumbnail_free(handle: *mut BPGThumbnailHandle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle);
        }
    }
}

/// Create universal thumbnail generator with default settings
#[no_mangle]
pub extern "C" fn universal_thumbnail_create() -> *mut UniversalThumbnailHandle {
    let generator = universal_thumbnail::UniversalThumbnailGenerator::new();
    Box::into_raw(Box::new(UniversalThumbnailHandle { generator }))
}

/// Create universal thumbnail generator with specific dimensions
#[no_mangle]
pub extern "C" fn universal_thumbnail_create_with_size(
    max_width: c_uint,
    max_height: c_uint,
) -> *mut UniversalThumbnailHandle {
    let generator = universal_thumbnail::UniversalThumbnailGenerator::with_dimensions(
        max_width,
        max_height,
    );
    Box::into_raw(Box::new(UniversalThumbnailHandle { generator }))
}

/// Generate thumbnail for any supported image format and save as PNG
#[no_mangle]
pub extern "C" fn universal_thumbnail_generate_png(
    handle: *const UniversalThumbnailHandle,
    input_path: *const c_char,
    output_path: *const c_char,
) -> c_int {
    if handle.is_null() || input_path.is_null() || output_path.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };

    let input_str = unsafe {
        match CStr::from_ptr(input_path).to_str() {
            Ok(s) => s,
            Err(_) => return BPGViewerError::InvalidParam as c_int,
        }
    };

    let output_str = unsafe {
        match CStr::from_ptr(output_path).to_str() {
            Ok(s) => s,
            Err(_) => return BPGViewerError::InvalidParam as c_int,
        }
    };

    match handle_ref.generator.generate_thumbnail_to_png(
        std::path::Path::new(input_str),
        std::path::Path::new(output_str),
    ) {
        Ok(_) => BPGViewerError::Success as c_int,
        Err(_) => BPGViewerError::EncodeFailed as c_int,
    }
}

/// Generate thumbnail for any supported image format and save as JPEG
/// quality: 1-100 (85 is a good default, gives ~3-5x smaller files than PNG for photos)
#[no_mangle]
pub extern "C" fn universal_thumbnail_generate_jpeg(
    handle: *const UniversalThumbnailHandle,
    input_path: *const c_char,
    output_path: *const c_char,
    quality: c_uint,
) -> c_int {
    if handle.is_null() || input_path.is_null() || output_path.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };

    let input_str = unsafe {
        match CStr::from_ptr(input_path).to_str() {
            Ok(s) => s,
            Err(_) => return BPGViewerError::InvalidParam as c_int,
        }
    };

    let output_str = unsafe {
        match CStr::from_ptr(output_path).to_str() {
            Ok(s) => s,
            Err(_) => return BPGViewerError::InvalidParam as c_int,
        }
    };

    let q = quality.min(100) as u8;

    match handle_ref.generator.generate_thumbnail_to_jpeg(
        std::path::Path::new(input_str),
        std::path::Path::new(output_str),
        q,
    ) {
        Ok(_) => BPGViewerError::Success as c_int,
        Err(_) => BPGViewerError::EncodeFailed as c_int,
    }
}

/// Check if a file format is supported by the universal thumbnail generator
#[no_mangle]
pub extern "C" fn universal_thumbnail_is_supported(file_path: *const c_char) -> c_int {
    if file_path.is_null() {
        return 0;
    }

    let path_str = unsafe {
        match CStr::from_ptr(file_path).to_str() {
            Ok(s) => s,
            Err(_) => return 0,
        }
    };

    if universal_thumbnail::UniversalThumbnailGenerator::is_supported_format(
        std::path::Path::new(path_str)
    ) {
        1
    } else {
        0
    }
}

/// Free universal thumbnail generator handle
#[no_mangle]
pub extern "C" fn universal_thumbnail_free(handle: *mut UniversalThumbnailHandle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle);
        }
    }
}

// ============================================================================
// Universal Image Decode API (full resolution BGRA for viewer)
// ============================================================================

/// Decode any supported image file to full resolution BGRA
/// Returns null on failure
#[no_mangle]
pub extern "C" fn universal_image_decode_file(path: *const c_char) -> *mut UniversalImageHandle {
    if path.is_null() {
        return ptr::null_mut();
    }

    let path_str = unsafe {
        match CStr::from_ptr(path).to_str() {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        }
    };

    match universal_decode::UniversalDecodedImage::decode_file(std::path::Path::new(path_str)) {
        Ok(image) => Box::into_raw(Box::new(UniversalImageHandle {
            image,
            bgra_cache: std::sync::Mutex::new(None),
        })),
        Err(_) => ptr::null_mut(),
    }
}

/// Get image dimensions from universal image handle
#[no_mangle]
pub extern "C" fn universal_image_get_dimensions(
    handle: *const UniversalImageHandle,
    width: *mut c_uint,
    height: *mut c_uint,
) -> c_int {
    if handle.is_null() || width.is_null() || height.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };
    unsafe {
        *width = handle_ref.image.width;
        *height = handle_ref.image.height;
    }

    BPGViewerError::Success as c_int
}

/// Copy BGRA data to a provided buffer (e.g. WPF WriteableBitmap)
/// Buffer must be at least width * height * 4 bytes
#[no_mangle]
pub extern "C" fn universal_image_copy_to_buffer(
    handle: *const UniversalImageHandle,
    buffer: *mut u8,
    buffer_size: usize,
    stride: usize,
) -> c_int {
    if handle.is_null() || buffer.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };
    let img = &handle_ref.image;

    let row_bytes = (img.width as usize) * 4;
    let required_size = stride * (img.height as usize);

    if buffer_size < required_size {
        return BPGViewerError::InvalidParam as c_int;
    }

    // Get BGRA data (convert if necessary)
    let bgra_data = match img.to_bgra() {
        Ok(data) => data,
        Err(_) => return BPGViewerError::DecodeFailed as c_int,
    };

    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, buffer_size) };

    // Copy row by row to handle stride
    for y in 0..img.height as usize {
        let src_start = y * row_bytes;
        let src_end = src_start + row_bytes;
        let dst_start = y * stride;
        let dst_end = dst_start + row_bytes;

        if src_end <= bgra_data.len() && dst_end <= buffer_size {
            buffer_slice[dst_start..dst_end].copy_from_slice(&bgra_data[src_start..src_end]);
        }
    }

    BPGViewerError::Success as c_int
}

/// Get BGRA data pointer and size from universal image handle
/// The returned pointer is valid as long as the handle exists
#[no_mangle]
pub extern "C" fn universal_image_get_data(
    handle: *const UniversalImageHandle,
    data: *mut *const u8,
    size: *mut usize,
) -> c_int {
    if handle.is_null() || data.is_null() || size.is_null() {
        return BPGViewerError::InvalidParam as c_int;
    }

    let handle_ref = unsafe { &*handle };

    // Get or create cached BGRA data
    let mut cache = match handle_ref.bgra_cache.lock() {
        Ok(c) => c,
        Err(_) => return BPGViewerError::InvalidParam as c_int,
    };

    if cache.is_none() {
        // Convert to BGRA and cache
        match handle_ref.image.to_bgra() {
            Ok(bgra) => *cache = Some(bgra),
            Err(_) => return BPGViewerError::DecodeFailed as c_int,
        }
    }

    if let Some(ref bgra_data) = *cache {
        unsafe {
            *data = bgra_data.as_ptr();
            *size = bgra_data.len();
        }
        BPGViewerError::Success as c_int
    } else {
        BPGViewerError::DecodeFailed as c_int
    }
}

/// Check if a file format is supported by the universal image decoder
#[no_mangle]
pub extern "C" fn universal_image_is_supported(file_path: *const c_char) -> c_int {
    if file_path.is_null() {
        return 0;
    }

    let path_str = unsafe {
        match CStr::from_ptr(file_path).to_str() {
            Ok(s) => s,
            Err(_) => return 0,
        }
    };

    if universal_decode::UniversalDecodedImage::is_supported_format(std::path::Path::new(path_str)) {
        1
    } else {
        0
    }
}

/// Free universal image handle
#[no_mangle]
pub extern "C" fn universal_image_free(handle: *mut UniversalImageHandle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle);
        }
    }
}

/// Get library version string
#[no_mangle]
pub extern "C" fn bpg_viewer_version() -> *const c_char {
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}

// ── GPU Thumbnail FFI Exports (Stubs - GPU Disabled) ─────────────────────

/// Initialize GPU thumbnail pipeline.
/// Returns -1 (GPU permanently disabled, CPU-only mode)
#[no_mangle]
pub extern "C" fn gpu_thumbnail_pipeline_init() -> c_int {
    -1 // GPU disabled
}

/// Legacy API - deprecated, kept for compatibility
/// Use gpu_generate_thumbnail_optimized instead
#[no_mangle]
pub extern "C" fn gpu_thumbnail_process_jpeg(
    _source_id: libc::uint64_t,
    _jpeg_path: *const c_char,
    _out_tile_x: *mut c_uint,
    _out_tile_y: *mut c_uint,
) -> c_int {
    eprintln!("[GPU FFI] Warning: gpu_thumbnail_process_jpeg is deprecated");
    -1
}

/// Legacy API - deprecated, kept for compatibility
/// Use gpu_generate_thumbnail_optimized instead
#[no_mangle]
pub extern "C" fn gpu_thumbnail_process_file(
    _source_id: libc::uint64_t,
    _file_path: *const c_char,
    _out_tile_x: *mut c_uint,
    _out_tile_y: *mut c_uint,
) -> c_int {
    eprintln!("[GPU FFI] Warning: gpu_thumbnail_process_file is deprecated");
    -3
}

/// Legacy API - deprecated, kept for compatibility
/// Use gpu_generate_thumbnail_optimized instead
#[no_mangle]
pub extern "C" fn gpu_thumbnail_readback_jpeg(
    _tile_x: c_uint,
    _tile_y: c_uint,
    _output_path: *const c_char,
    _quality: c_uint,
) -> c_int {
    eprintln!("[GPU FFI] Warning: gpu_thumbnail_readback_jpeg is deprecated");
    -1
}

// ─── GPU Detection Functions (Stubs - GPU Disabled) ─────────────────────────────────

#[no_mangle]
pub extern "C" fn NativeHasGPU() -> bool {
    false // GPU permanently disabled
}

#[no_mangle]
pub extern "C" fn NativeHasCUDA() -> bool {
    false // GPU permanently disabled
}

/// Check if OpenCL is available
#[no_mangle]
pub extern "C" fn NativeHasOpenCL() -> bool {
    false
}

/// Check if DirectML is available
#[no_mangle]
pub extern "C" fn NativeHasDirectML() -> bool {
    false
}

/// Get the active GPU backend type
#[no_mangle]
pub extern "C" fn NativeGetActiveBackend() -> c_int {
    0 // None - CPU only
}

/// Get the active GPU backend name
#[no_mangle]
pub extern "C" fn NativeGetActiveBackendName() -> *const c_char {
    std::ffi::CString::new("CPU").unwrap().into_raw()
}

/// Get the GPU device name
#[no_mangle]
pub extern "C" fn NativeGetDeviceName() -> *const c_char {
    std::ffi::CString::new("CPU").unwrap().into_raw()
}

// ── Async Loading FFI Exports (Phase 3: Multi-Threading) ───────────────────

/// Load full-resolution image asynchronously
///
/// The callback is invoked on a background thread when loading completes.
/// Callback signature: fn(user_data: u64, data_ptr: *const u8, width: u32, height: u32, stride: usize, error: *const c_char)
///
/// If successful, error will be null and data_ptr will point to BGRA8 pixel data.
/// If failed, error will point to an error message string.
#[no_mangle]
pub extern "C" fn fullimage_load_async(
    path: *const c_char,
    callback: extern "C" fn(u64, *const u8, u32, u32, usize, *const c_char),
    user_data: u64,
) {
    if path.is_null() {
        eprintln!("[FullImage] Error: null path");
        return;
    }

    let path_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("[FullImage] Error: Invalid UTF-8 in path");
            return;
        }
    };

    let path_buf = PathBuf::from(path_str);

    fullimage_loader::load_fullimage_async(path_buf, move |result| {
        match result {
            Ok(response) => {
                // Success: pass image data
                callback(
                    user_data,
                    response.data.as_ptr(),
                    response.width,
                    response.height,
                    response.stride,
                    std::ptr::null(), // no error
                );
                // Keep data alive until callback returns
                std::mem::forget(response.data);
            }
            Err(e) => {
                // Failure: pass error message
                let error_cstr = std::ffi::CString::new(e).unwrap_or_default();
                callback(
                    user_data,
                    std::ptr::null(),
                    0,
                    0,
                    0,
                    error_cstr.as_ptr(),
                );
            }
        }
    });
}

/// Generate thumbnail asynchronously (legacy - redirects to new pipeline)
///
/// DEPRECATED: Use bpg_generate_thumbnail_async instead (from pipeline module)
/// This function is kept for backwards compatibility.
///
/// Callback signature: fn(source_id: u64, result: c_int, error: *const c_char)
/// result: 0 = success, non-zero = failure
#[no_mangle]
pub extern "C" fn thumbnail_generate_async(
    source_id: u64,
    input_path: *const c_char,
    output_path: *const c_char,
    quality: u8,
    callback: extern "C" fn(u64, c_int, *const c_char),
) {
    // Redirect to new pipeline
    let _ = bpg_generate_thumbnail_async(source_id, input_path, output_path, quality, callback);
}

// Re-export the clean Tokio pipeline API
// This is the RECOMMENDED async thumbnail API
pub use pipeline::bpg_generate_thumbnail_async;
pub use pipeline::bpg_error_free;
pub use pipeline::bpg_pipeline_is_initialized;
pub use pipeline::bpg_pipeline_shutdown;


// ── Optimized Thumbnail API (Unified GPU/CPU Backend) ──────────────────────

/// All-in-one thumbnail generation (CPU-only, synchronous)
///
/// DEPRECATED: For async thumbnails, use bpg_generate_thumbnail_async instead
///
/// Parameters:
/// - `source_id`: Unique ID for this thumbnail
/// - `input_path`: Path to source image (JPEG, PNG, HEIC, etc.)
/// - `output_path`: Path to save 256×256 JPEG thumbnail
/// - `quality`: JPEG quality 1-100 (85 recommended)
/// - `_is_jpeg`: Unused, kept for compatibility
///
/// Returns:
/// - 0: Success
/// - -1: Backend error
/// - -2: Decode/file error
/// - -3: Encode error
#[no_mangle]
pub extern "C" fn gpu_generate_thumbnail_optimized(
    _source_id: libc::uint64_t,
    input_path: *const c_char,
    output_path: *const c_char,
    quality: c_uint,
    _is_jpeg: bool,
) -> c_int {
    if input_path.is_null() || output_path.is_null() {
        eprintln!("[Thumbnail] Error: null pointer");
        return -1;
    }

    let input_str = match unsafe { CStr::from_ptr(input_path) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[Thumbnail] Error: Invalid UTF-8 in input path: {}", e);
            return -2;
        }
    };

    let output_str = match unsafe { CStr::from_ptr(output_path) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[Thumbnail] Error: Invalid UTF-8 in output path: {}", e);
            return -2;
        }
    };

    let input = std::path::Path::new(input_str);
    let output = std::path::Path::new(output_str);
    let quality = quality.clamp(1, 100) as u8;

    // Use universal thumbnail generator (CPU-only)
    let gen = universal_thumbnail::UniversalThumbnailGenerator::with_dimensions(256, 256);
    match gen.generate_thumbnail_to_jpeg(input, output, quality) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[Thumbnail] Warning: Generation failed for '{}': {}", input_str, e);
            let err_str = e.to_string();
            if err_str.contains("decode") || err_str.contains("Read failed") {
                -2
            } else if err_str.contains("encod") {
                -3
            } else {
                -1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // FIXME: version_string function doesn't exist
    fn test_version() {
        // let version = ffi::version_string();
        // assert!(!version.is_empty());
    }

    #[test]
    #[ignore] // FIXME: config field is private
    fn test_thumbnail_generator() {
        let gen = ThumbnailGenerator::new();
        // let config = gen.config;
        // assert_eq!(config.max_width, 256);
        // assert_eq!(config.max_height, 256);
        drop(gen); // Silence unused warning
    }
}
