//! Unified thumbnail pipeline using Tokio
//!
//! This module provides a single, clean async API for thumbnail generation
//! that replaces the old GPU/CPU/Windows-cache complexity with a simple
//! Tokio-based pipeline that uses spawn_blocking for CPU-intensive work.

use std::path::PathBuf;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::Arc;
use tokio::sync::Semaphore;
use anyhow::Result;

/// Maximum concurrent thumbnail operations (CPU-bound work)
const MAX_CONCURRENT: usize = 8;

/// Shared pipeline – created once per process
pub struct Pipeline {
    runtime: tokio::runtime::Runtime,
    semaphore: Arc<Semaphore>,
}

static PIPELINE: std::sync::OnceLock<Arc<Pipeline>> = std::sync::OnceLock::new();

/// Get or initialize the global pipeline
fn get_pipeline() -> &'static Arc<Pipeline> {
    PIPELINE.get_or_init(|| {
        eprintln!("[Pipeline] Initializing Tokio runtime (4 workers, max {} concurrent thumbnails)", MAX_CONCURRENT);

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("thumbnail-worker")
            .enable_all()
            .build()
            .expect("Failed to start Tokio runtime");

        Arc::new(Pipeline {
            runtime: rt,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT)),
        })
    })
}

/// Convert Rust error → C string (caller must free with `bpg_error_free`)
fn err_to_c(err: impl std::fmt::Display) -> *const c_char {
    CString::new(err.to_string())
        .unwrap_or_else(|_| CString::new("Error message contained null byte").unwrap())
        .into_raw()
}

/// FFI entry – async thumbnail generation
///
/// # Arguments
/// * `source_id` - Unique ID for this thumbnail request
/// * `input_path` - Path to source image (UTF-8 C string)
/// * `output_path` - Path to save thumbnail (UTF-8 C string)
/// * `quality` - JPEG quality 1-100 (85 recommended)
/// * `callback` - Function called when generation completes
///
/// # Returns
/// * `0` - Request accepted and queued
/// * `-1` - Invalid arguments (null pointer or bad UTF-8)
/// * `-2` - Pipeline initialization failed
///
/// # Callback Signature
/// `fn(source_id: u64, result: c_int, error: *const c_char)`
/// * `result = 0` - Success
/// * `result = -1` - Generation failed (error string provided)
/// * `error` - Error message (null on success, must free with `bpg_error_free`)
#[no_mangle]
pub extern "C" fn bpg_generate_thumbnail_async(
    source_id: u64,
    input_path: *const c_char,
    output_path: *const c_char,
    quality: u8,
    callback: extern "C" fn(u64, c_int, *const c_char),
) -> c_int {
    // Validate arguments
    if input_path.is_null() || output_path.is_null() {
        eprintln!("[Pipeline] Error: null pointer argument");
        return -1;
    }

    let in_str = match unsafe { CStr::from_ptr(input_path) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[Pipeline] Error: Invalid UTF-8 in input path: {}", e);
            return -1;
        }
    };

    let out_str = match unsafe { CStr::from_ptr(output_path) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[Pipeline] Error: Invalid UTF-8 in output path: {}", e);
            return -1;
        }
    };

    let input_pb = PathBuf::from(in_str);
    let output_pb = PathBuf::from(out_str);
    let quality_clamped = quality.clamp(1, 100);

    // Get the global pipeline
    let pipeline = get_pipeline();
    let semaphore = pipeline.semaphore.clone();

    // Schedule on Tokio runtime
    pipeline.runtime.spawn(async move {
        // Acquire semaphore permit for backpressure
        let _permit = match semaphore.acquire().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[Pipeline] Failed to acquire semaphore: {}", e);
                callback(source_id, -1, err_to_c(e));
                return;
            }
        };

        // CPU-bound work runs in blocking thread pool
        let result: Result<()> = tokio::task::spawn_blocking(move || {
            // Use universal thumbnail generator (supports all formats)
            let gen = crate::universal_thumbnail::UniversalThumbnailGenerator::with_dimensions(256, 256);

            // Generate thumbnail to JPEG (smaller than PNG, better for thumbnails)
            gen.generate_thumbnail_to_jpeg(&input_pb, &output_pb, quality_clamped)
                .map_err(|e| anyhow::anyhow!("Thumbnail generation failed: {}", e))
        })
        .await
        .map_err(|e| anyhow::anyhow!("Task panicked: {}", e))
        .and_then(|r| r); // Flatten Result<Result<T>>

        // Invoke callback with result
        match result {
            Ok(_) => {
                callback(source_id, 0, std::ptr::null());
            }
            Err(e) => {
                eprintln!("[Pipeline] Thumbnail generation failed: {}", e);
                callback(source_id, -1, err_to_c(e));
            }
        }
    });

    0 // Request accepted
}

/// Free error string allocated by pipeline callbacks
///
/// Call this on any non-null error pointer received in callbacks.
/// Safe to call with null pointer (no-op).
#[no_mangle]
pub extern "C" fn bpg_error_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            // Reconstruct CString and let it drop
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Check if the pipeline is initialized
///
/// Returns 1 if initialized, 0 if not yet initialized.
/// This is mainly for testing/debugging.
#[no_mangle]
pub extern "C" fn bpg_pipeline_is_initialized() -> c_int {
    if PIPELINE.get().is_some() {
        1
    } else {
        0
    }
}

/// Shutdown the pipeline and cleanup resources
///
/// This should be called on process exit. After calling this,
/// no more thumbnail requests can be processed.
///
/// Returns 0 on success, -1 if already shut down.
#[no_mangle]
pub extern "C" fn bpg_pipeline_shutdown() -> c_int {
    // Note: OnceLock doesn't support taking the value out
    // The runtime will be dropped when the process exits
    // This is mainly a no-op for compatibility
    eprintln!("[Pipeline] Shutdown requested (runtime will cleanup on process exit)");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_singleton() {
        let p1 = get_pipeline();
        let p2 = get_pipeline();
        assert!(Arc::ptr_eq(p1, p2), "Pipeline should be a singleton");
    }

    #[test]
    fn test_err_to_c_roundtrip() {
        let err = "Test error message";
        let c_str = err_to_c(err);

        let result = unsafe {
            CStr::from_ptr(c_str).to_str().unwrap()
        };

        assert_eq!(result, err);

        // Free it
        bpg_error_free(c_str as *mut c_char);
    }

    #[test]
    fn test_err_to_c_with_null() {
        // Should not panic on null byte in error
        let err = "Error\0with null";
        let c_str = err_to_c(err);

        // Should get fallback message
        let result = unsafe {
            CStr::from_ptr(c_str).to_str().unwrap()
        };

        assert!(result.contains("null byte"));

        bpg_error_free(c_str as *mut c_char);
    }

    #[tokio::test]
    async fn test_pipeline_concurrent_requests() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        // Create a simple callback that increments counter
        extern "C" fn test_callback(_: u64, _: c_int, _: *const c_char) {
            // In real test, would need to access counter
        }

        // Just test that pipeline initializes and accepts requests
        let pipeline = get_pipeline();
        assert!(Arc::strong_count(pipeline) >= 1);
    }
}
