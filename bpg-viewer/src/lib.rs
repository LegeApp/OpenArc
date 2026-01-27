// BPG Viewer and Thumbnail Library
// Standalone FFI-capable library for BPG image viewing and thumbnail generation

pub mod ffi;
pub mod decoder;
pub mod encoder;
pub mod thumbnail;
pub mod universal_thumbnail;
pub mod universal_decode;

// Re-export main types
pub use decoder::{DecodedImage, decode_file, decode_memory};
pub use encoder::BPGEncoder;
pub use thumbnail::{ThumbnailGenerator, ThumbnailConfig};
pub use universal_thumbnail::UniversalThumbnailGenerator;
pub use ffi::{BPGImageFormat, BPGEncoderConfig};

// C FFI interface for embedding in other languages
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint};
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
        Ok(image) => Box::into_raw(Box::new(UniversalImageHandle { image })),
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

    let buffer_slice = unsafe { slice::from_raw_parts_mut(buffer, buffer_size) };

    // Copy row by row to handle stride
    for y in 0..img.height as usize {
        let src_start = y * row_bytes;
        let src_end = src_start + row_bytes;
        let dst_start = y * stride;
        let dst_end = dst_start + row_bytes;

        if src_end <= img.data.len() && dst_end <= buffer_size {
            buffer_slice[dst_start..dst_end].copy_from_slice(&img.data[src_start..src_end]);
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
    unsafe {
        *data = handle_ref.image.data.as_ptr();
        *size = handle_ref.image.data.len();
    }

    BPGViewerError::Success as c_int
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = ffi::version_string();
        assert!(!version.is_empty());
    }

    #[test]
    fn test_thumbnail_generator() {
        let gen = ThumbnailGenerator::new();
        let config = gen.config;
        assert_eq!(config.max_width, 256);
        assert_eq!(config.max_height, 256);
    }
}
