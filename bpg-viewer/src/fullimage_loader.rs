//! Async full-image loading to prevent UI blocking
//!
//! Loads full-resolution images on background threads.

use std::path::{Path, PathBuf};
use std::thread;

/// Full-resolution image response
#[derive(Debug)]
pub struct FullImageResponse {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// BGRA8 pixel data (for WPF compatibility)
    pub data: Vec<u8>,
    /// Row stride in bytes
    pub stride: usize,
}

/// Load full-resolution image asynchronously
///
/// This spawns the load operation on a background thread,
/// then invokes the callback with the result.
pub fn load_fullimage_async<F>(path: PathBuf, callback: F)
where
    F: FnOnce(Result<FullImageResponse, String>) + Send + 'static,
{
    thread::spawn(move || {
        let result = load_fullimage_sync(&path);
        callback(result);
    });
}

/// Load full-resolution image synchronously
///
/// This is called on a background thread by load_fullimage_async.
pub fn load_fullimage_sync(path: &Path) -> Result<FullImageResponse, String> {
    // Use universal decoder to support all formats
    let decoded = crate::universal_decode::UniversalDecodedImage::decode_file(path)
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    // Convert to BGRA for UI (convert if YCbCr)
    let bgra_data = decoded.to_bgra()
        .map_err(|e| format!("BGRA conversion failed: {}", e))?;

    Ok(FullImageResponse {
        width: decoded.width,
        height: decoded.height,
        stride: decoded.width as usize * 4, // BGRA = 4 bytes per pixel
        data: bgra_data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fullimage_response_creation() {
        let response = FullImageResponse {
            width: 100,
            height: 100,
            data: vec![0u8; 100 * 100 * 4],
            stride: 100 * 4,
        };

        assert_eq!(response.width, 100);
        assert_eq!(response.height, 100);
        assert_eq!(response.stride, 400);
        assert_eq!(response.data.len(), 40000);
    }
}
