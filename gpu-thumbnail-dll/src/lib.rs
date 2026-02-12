//! GPU Thumbnail DLL - Separate dynamically-loaded library for GPU acceleration
//!
//! This DLL is loaded at runtime by bpg-viewer. If GPU initialization fails or
//! the DLL is missing, bpg-viewer falls back to CPU thumbnailing.

use parking_lot::Mutex;
use std::sync::Arc;
use std::path::Path;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint};

type GpuPipeline = thumbnail_gpu::ThumbnailPipeline;
type YCbCrData = thumbnail_gpu::YCbCrData;

/// Maximum YCbCr data size for GPU staging upload (bytes).
const MAX_STAGING_BYTES: usize = 64 * 1024 * 1024; // 64 MB
const MAX_PRE_DOWNSCALE_DIM: u32 = 4096;

/// Global GPU pipeline singleton
static GPU_PIPELINE: parking_lot::Mutex<Option<Arc<Mutex<GpuPipeline>>>> =
    parking_lot::Mutex::new(None);

/// Thread-local error message storage
thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<CString>> = std::cell::RefCell::new(None);
}

fn set_last_error(msg: &str) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = CString::new(msg).ok();
    });
}

/// Initialize the GPU pipeline on demand.
pub fn gpu_pipeline_init() -> Result<Arc<Mutex<GpuPipeline>>, String> {
    let mut opt = GPU_PIPELINE.lock();
    if let Some(pipeline) = opt.as_ref() {
        return Ok(Arc::clone(pipeline));
    }

    match GpuPipeline::new() {
        Ok(pipeline) => {
            let pipeline = Arc::new(Mutex::new(pipeline));
            *opt = Some(Arc::clone(&pipeline));
            Ok(pipeline)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Decode JPEG file to YCbCr 4:2:0 planar.
pub fn decode_jpeg_to_ycbcr(path: &Path) -> Result<YCbCrData, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Read failed: {}", e))?;

    match zune_image::decode_jpeg_ycbcr(&bytes) {
        Ok(ycbcr) => Ok(YCbCrData {
            y_plane: ycbcr.y_plane,
            cb_plane: ycbcr.cb_plane,
            cr_plane: ycbcr.cr_plane,
            width: ycbcr.width,
            height: ycbcr.height,
        }),
        Err(e) => Err(format!("YCbCr decode failed: {}", e)),
    }
}

/// Pre-downscale YCbCr data if it exceeds staging buffer limits.
pub fn maybe_downscale_ycbcr(mut ycbcr: YCbCrData) -> YCbCrData {
    let total = ycbcr.total_bytes();
    if total <= MAX_STAGING_BYTES && ycbcr.width <= MAX_PRE_DOWNSCALE_DIM && ycbcr.height <= MAX_PRE_DOWNSCALE_DIM {
        return ycbcr;
    }

    let scale_x = (ycbcr.width as f32) / (MAX_PRE_DOWNSCALE_DIM as f32);
    let scale_y = (ycbcr.height as f32) / (MAX_PRE_DOWNSCALE_DIM as f32);
    let scale = scale_x.max(scale_y).max(1.0);

    if scale <= 1.0 {
        return ycbcr;
    }

    let new_w = ((ycbcr.width as f32 / scale) as u32).max(2) & !1;
    let new_h = ((ycbcr.height as f32 / scale) as u32).max(2) & !1;
    let new_cw = (new_w + 1) / 2;
    let new_ch = (new_h + 1) / 2;

    let new_y = downscale_plane(&ycbcr.y_plane, ycbcr.width, ycbcr.height, new_w, new_h);

    let old_cw = (ycbcr.width + 1) / 2;
    let old_ch = (ycbcr.height + 1) / 2;
    let new_cb = downscale_plane(&ycbcr.cb_plane, old_cw, old_ch, new_cw, new_ch);
    let new_cr = downscale_plane(&ycbcr.cr_plane, old_cw, old_ch, new_cw, new_ch);

    YCbCrData {
        y_plane: new_y,
        cb_plane: new_cb,
        cr_plane: new_cr,
        width: new_w,
        height: new_h,
    }
}

fn downscale_plane(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dst_w * dst_h) as usize];
    let sx = src_w as f32 / dst_w as f32;
    let sy = src_h as f32 / dst_h as f32;

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let src_x = ((dx as f32 + 0.5) * sx) as u32;
            let src_y = ((dy as f32 + 0.5) * sy) as u32;
            let src_x = src_x.min(src_w - 1);
            let src_y = src_y.min(src_h - 1);
            dst[(dy * dst_w + dx) as usize] = src[(src_y * src_w + src_x) as usize];
        }
    }
    dst
}

/// Decode any image to YCbCr using codecs
pub fn decode_any_to_ycbcr(path: &Path) -> Result<YCbCrData, String> {
    // Check if it's HEIC
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if ext == "heic" || ext == "heif" {
        use codecs::heic::HeicCodec;
        let mut decoder = HeicCodec::new()
            .map_err(|e| format!("HEIC codec init failed: {}", e))?;

        let ycbcr = decoder.decode_file_ycbcr420(path)
            .map_err(|e| format!("HEIC YCbCr decode failed: {}", e))?;

        return Ok(YCbCrData {
            y_plane: ycbcr.y_plane,
            cb_plane: ycbcr.cb_plane,
            cr_plane: ycbcr.cr_plane,
            width: ycbcr.width,
            height: ycbcr.height,
        });
    }

    // For other formats, use image crate thumbnail
    match image::open(path) {
        Ok(img) => {
            let img = img.thumbnail(1024, 1024);
            let rgb_img = img.to_rgb8();
            let width = rgb_img.width();
            let height = rgb_img.height();
            let rgb = rgb_img.as_raw();
            rgb_to_ycbcr420(rgb, width, height)
        }
        Err(e) => Err(format!("Image decode failed: {}", e)),
    }
}

/// Convert RGB to YCbCr 4:2:0 planar
pub fn rgb_to_ycbcr420(rgb: &[u8], width: u32, height: u32) -> Result<YCbCrData, String> {
    let pixel_count = (width * height) as usize;
    if rgb.len() < pixel_count * 3 {
        return Err(format!(
            "RGB buffer too small: {} bytes for {}x{} (need {})",
            rgb.len(), width, height, pixel_count * 3
        ));
    }

    let chroma_w = (width + 1) / 2;
    let chroma_h = (height + 1) / 2;
    let chroma_count = (chroma_w * chroma_h) as usize;

    let mut y_plane = Vec::with_capacity(pixel_count);
    for chunk in rgb.chunks_exact(3) {
        let r = chunk[0] as f32;
        let g = chunk[1] as f32;
        let b = chunk[2] as f32;
        let y = (0.299 * r + 0.587 * g + 0.114 * b).round().clamp(0.0, 255.0);
        y_plane.push(y as u8);
    }

    let mut cb_plane = Vec::with_capacity(chroma_count);
    let mut cr_plane = Vec::with_capacity(chroma_count);

    for cy in 0..chroma_h {
        for cx in 0..chroma_w {
            let y0 = (cy * 2) as usize;
            let x0 = (cx * 2) as usize;
            let w = width as usize;

            let mut cb_sum: f32 = 0.0;
            let mut cr_sum: f32 = 0.0;
            let mut count: f32 = 0.0;

            for dy in 0..2usize {
                let y = y0 + dy;
                if y >= height as usize { break; }
                for dx in 0..2usize {
                    let x = x0 + dx;
                    if x >= w { break; }
                    let idx = (y * w + x) * 3;
                    let r = rgb[idx] as f32;
                    let g = rgb[idx + 1] as f32;
                    let b = rgb[idx + 2] as f32;
                    cb_sum += -0.169 * r - 0.331 * g + 0.500 * b + 128.0;
                    cr_sum += 0.500 * r - 0.419 * g - 0.081 * b + 128.0;
                    count += 1.0;
                }
            }

            cb_plane.push((cb_sum / count).round().clamp(0.0, 255.0) as u8);
            cr_plane.push((cr_sum / count).round().clamp(0.0, 255.0) as u8);
        }
    }

    Ok(YCbCrData {
        y_plane,
        cb_plane,
        cr_plane,
        width,
        height,
    })
}

/// All-in-one GPU thumbnail generation
pub fn generate_gpu_thumbnail(
    source_id: u64,
    input_path: &Path,
    output_path: &Path,
    quality: u8,
    is_jpeg: bool,
) -> Result<(), String> {
    // Decode
    let ycbcr = if is_jpeg {
        decode_jpeg_to_ycbcr(input_path)?
    } else {
        decode_any_to_ycbcr(input_path)?
    };

    let pipeline = gpu_pipeline_init()?;

    // Process and readback in one lock
    let tile_data = {
        let mut p = pipeline.lock();

        // Flush if atlas is getting full
        if p.atlas.occupied_count() > p.atlas.tile_count / 2 {
            if let Err(e) = p.flush_gpu() {
                eprintln!("[GPU] Warning: GPU flush failed: {}", e);
            }
        }

        let (tile_idx, data) = p.process_and_readback_tile(source_id, &ycbcr)
            .map_err(|e| format!("GPU process+readback failed: {}", e))?;
        p.atlas.release_tile(tile_idx);
        data
    };

    // Encode to JPEG (no lock)
    let tile_size = 256u32;
    let mut rgb_data = Vec::with_capacity((tile_size * tile_size * 3) as usize);
    for chunk in tile_data.chunks_exact(4) {
        rgb_data.push(chunk[0]);
        rgb_data.push(chunk[1]);
        rgb_data.push(chunk[2]);
    }

    let output_file = std::fs::File::create(output_path)
        .map_err(|e| format!("Create file failed: {:?}: {}", output_path, e))?;

    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
        output_file,
        quality,
    );

    encoder
        .encode(
            &rgb_data,
            tile_size,
            tile_size,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("JPEG encoding failed: {}", e))?;

    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// C FFI EXPORTS
// ══════════════════════════════════════════════════════════════════

/// Initialize GPU pipeline
/// Returns 0 on success, -1 on failure
#[no_mangle]
pub extern "C" fn gpu_thumbnail_pipeline_init() -> c_int {
    match gpu_pipeline_init() {
        Ok(_) => 0,
        Err(e) => {
            set_last_error(&e);
            -1
        }
    }
}

/// Generate thumbnail (all-in-one operation)
/// Returns 0 on success, -1 on GPU error, -2 on decode error, -3 on encode error
#[no_mangle]
pub extern "C" fn gpu_thumbnail_generate(
    source_id: libc::uint64_t,
    input_path: *const c_char,
    output_path: *const c_char,
    quality: c_uint,
) -> c_int {
    if input_path.is_null() || output_path.is_null() {
        set_last_error("Null pointer parameter");
        return -1;
    }

    let input_str = match unsafe { CStr::from_ptr(input_path) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid UTF-8 in input path");
            return -2;
        }
    };

    let output_str = match unsafe { CStr::from_ptr(output_path) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid UTF-8 in output path");
            return -2;
        }
    };

    let input = Path::new(input_str);
    let output = Path::new(output_str);

    // Check if JPEG
    let ext = input.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let is_jpeg = ext == "jpg" || ext == "jpeg";

    let quality = quality.clamp(1, 100) as u8;

    match generate_gpu_thumbnail(source_id as u64, input, output, quality, is_jpeg) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(&e);
            if e.contains("decode") || e.contains("Read failed") {
                -2
            } else if e.contains("encoding") {
                -3
            } else {
                -1
            }
        }
    }
}

/// Get last error message
/// Returns pointer to error string (valid until next error or thread exit)
#[no_mangle]
pub extern "C" fn gpu_thumbnail_get_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(std::ptr::null())
    })
}

/// Check if GPU is available
#[no_mangle]
pub extern "C" fn gpu_thumbnail_is_available() -> c_int {
    match gpu_pipeline_init() {
        Ok(_) => 1,
        Err(_) => 0,
    }
}
