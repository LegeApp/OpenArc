//! GPU thumbnail pipeline management (FFI-safe wrappers)
//!
//! Handles:
//! - Lazy initialization of D3D12 pipeline
//! - JPEG decoding to YCbCr 4:2:0 planar
//! - Universal image decoding (HEIC, PNG, etc.) → YCbCr
//! - CPU pre-downscale for large images (avoid staging overflow)
//! - GPU resize → atlas tile
//! - Tile readback → JPEG encoding (not PNG)

use parking_lot::Mutex;
use std::sync::Arc;
use std::path::Path;
use std::io::Cursor;

type GpuPipeline = thumbnail_gpu::ThumbnailPipeline;
type YCbCrData = thumbnail_gpu::YCbCrData;

/// Maximum YCbCr data size for GPU staging upload (bytes).
/// Staging ring slots are 32 MB each. Leave headroom.
const MAX_STAGING_BYTES: usize = 64 * 1024 * 1024; // 64 MB (allow larger images)

/// Max dimension for CPU pre-downscale.
/// Images larger than this get downscaled on CPU before GPU upload.
/// ~4096x4096 YCbCr ≈ 24 MB, well within 64MB staging limit.
/// Most phone cameras are 4000x3000 or less, so this avoids unnecessary CPU work.
const MAX_PRE_DOWNSCALE_DIM: u32 = 4096;

/// Global GPU pipeline singleton (per-process, thread-safe via Mutex).
/// Lazy init: None → Some on first GPU request.
static GPU_PIPELINE: parking_lot::Mutex<Option<Arc<Mutex<GpuPipeline>>>> =
    parking_lot::Mutex::new(None);

/// Initialize the GPU pipeline on demand.
/// Returns error string on failure (empty string = success).
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
    // Read file
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Read failed: {}", e))?;

    // Decode with zune-image (YCbCr path)
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

/// Decode JPEG to YCbCr with automatic reduced-resolution decoding for thumbnails.
///
/// For 256×256 thumbnails, we don't need full resolution. This function:
/// 1. Decodes at full resolution (most efficient path for zune-jpeg)
/// 2. Only downscales if decoded data > 64MB (rare - only >8K images)
///
/// This eliminates unnecessary CPU pre-downscaling for typical 4000×3000 photos
/// (17.2 MB YCbCr fits easily in 64MB staging buffer).
pub fn decode_jpeg_to_ycbcr_thumbnail(path: &Path) -> Result<YCbCrData, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Read failed: {}", e))?;

    // Decode at full resolution
    match zune_image::decode_jpeg_ycbcr(&bytes) {
        Ok(ycbcr) => {
            let mut ycbcr_data = YCbCrData {
                y_plane: ycbcr.y_plane,
                cb_plane: ycbcr.cb_plane,
                cr_plane: ycbcr.cr_plane,
                width: ycbcr.width,
                height: ycbcr.height,
            };
            
            // Only downscale if very large (>64MB = >8K images)
            // Typical 4000×3000 photos (17.2 MB) will NOT be downscaled
            if ycbcr_data.total_bytes() > MAX_STAGING_BYTES {
                ycbcr_data = maybe_downscale_ycbcr(ycbcr_data);
            }
            
            Ok(ycbcr_data)
        }
        Err(e) => Err(format!("YCbCr decode failed: {}", e)),
    }
}

/// Process YCbCr data through GPU pipeline.
/// Returns (tile_x, tile_y) of the (256×256) tile in the 4096×4096 atlas.
/// 
/// Error recovery: If GPU processing fails, returns error but pipeline remains usable.
pub fn gpu_process_thumbnail(
    source_id: u64,
    ycbcr: &YCbCrData,
) -> Result<(u32, u32), String> {
    let pipeline = gpu_pipeline_init()?;
    let mut p = pipeline.lock();

    // Validate input before GPU processing
    let total_bytes = ycbcr.total_bytes();
    if total_bytes > MAX_STAGING_BYTES {
        return Err(format!(
            "Image too large for GPU processing: {:.1} MB (max: {:.1} MB). Image: {}x{}",
            total_bytes as f64 / (1024.0 * 1024.0),
            MAX_STAGING_BYTES as f64 / (1024.0 * 1024.0),
            ycbcr.width,
            ycbcr.height
        ));
    }

    match p.process_thumbnail(source_id, ycbcr) {
        Ok(tile) => {
            let (tx, ty) = p.atlas.tile_pixel_offset(tile);
            Ok((tx, ty))
        }
        Err(e) => {
            eprintln!("[GPU] Process failed for source_id {}: {}", source_id, e);
            Err(e.to_string())
        }
    }
}

/// Read back a tile from the atlas and encode as JPEG.
/// `tile_x`, `tile_y` are the pixel offsets (from gpu_process_thumbnail).
/// `quality`: JPEG quality 1-100 (85 recommended for thumbnails).
/// 
/// Error recovery: Returns error on failure but doesn't crash the pipeline.
pub fn gpu_readback_and_encode_jpeg(
    tile_x: u32,
    tile_y: u32,
    output_path: &Path,
    quality: u8,
) -> Result<(), String> {
    let pipeline = gpu_pipeline_init()?;
    let mut p = pipeline.lock();

    // Simple brute-force tile lookup (in production, cache tile indices)
    let atlas_size = p.atlas.atlas_size;
    let tile_size = p.atlas.tile_size;
    let tile_idx = (tile_y / tile_size) * (atlas_size / tile_size) + (tile_x / tile_size);

    let tile_data = p
        .readback_tile(thumbnail_gpu::TileIndex(tile_idx as u16))
        .map_err(|e| {
            let err_msg = format!("Readback failed for tile ({}, {}): {}", tile_x, tile_y, e);
            eprintln!("[GPU] {}", err_msg);
            err_msg
        })?;

    // Convert RGBA8 → RGB (strip alpha channel for JPEG)
    let mut rgb_data = Vec::with_capacity((tile_size as usize) * (tile_size as usize) * 3);
    for chunk in tile_data.chunks_exact(4) {
        rgb_data.push(chunk[0]); // R
        rgb_data.push(chunk[1]); // G
        rgb_data.push(chunk[2]); // B
    }

    // Encode as JPEG using image crate
    let output_file = std::fs::File::create(output_path)
        .map_err(|e| {
            let err_msg = format!("Create file failed: {:?}: {}", output_path, e);
            eprintln!("[GPU] {}", err_msg);
            err_msg
        })?;
    
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
        .map_err(|e| {
            let err_msg = format!("JPEG encoding failed: {}", e);
            eprintln!("[GPU] {}", err_msg);
            err_msg
        })?;

    Ok(())
}

/// Pre-downscale YCbCr data if it exceeds staging buffer limits.
///
/// For thumbnail generation (256x256 output), we don't need full-resolution
/// input. Downscaling to ~2048x2048 on CPU before GPU upload avoids staging
/// buffer overflows with high-resolution photos (24-48 MP).
pub fn maybe_downscale_ycbcr(mut ycbcr: YCbCrData) -> YCbCrData {
    let total = ycbcr.total_bytes();
    if total <= MAX_STAGING_BYTES && ycbcr.width <= MAX_PRE_DOWNSCALE_DIM && ycbcr.height <= MAX_PRE_DOWNSCALE_DIM {
        return ycbcr;  // No downscaling needed
    }

    // Only log when actually downscaling (not every image)
    // Calculate downscale factor
    let scale_x = (ycbcr.width as f32) / (MAX_PRE_DOWNSCALE_DIM as f32);
    let scale_y = (ycbcr.height as f32) / (MAX_PRE_DOWNSCALE_DIM as f32);
    let scale = scale_x.max(scale_y).max(1.0);
    
    if scale <= 1.0 {
        return ycbcr;  // No downscaling needed
    }

    let new_w = ((ycbcr.width as f32 / scale) as u32).max(2) & !1; // ensure even
    let new_h = ((ycbcr.height as f32 / scale) as u32).max(2) & !1;
    let new_cw = (new_w + 1) / 2;
    let new_ch = (new_h + 1) / 2;

    // Only log for very large images (>6000px) to reduce noise
    if ycbcr.width > 6000 || ycbcr.height > 6000 {
        eprintln!(
            "[GPU] Pre-downscale (large image): {}x{} ({:.1} MB) -> {}x{} ({:.1} MB)",
            ycbcr.width, ycbcr.height,
            total as f64 / (1024.0 * 1024.0),
            new_w, new_h,
            (new_w * new_h + new_cw * new_ch * 2) as f64 / (1024.0 * 1024.0),
        );
    }

    // Downscale Y plane with bilinear sampling
    let new_y = downscale_plane(&ycbcr.y_plane, ycbcr.width, ycbcr.height, new_w, new_h);
    
    // Downscale chroma planes
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

/// Simple box-filter downscale of a single plane.
fn downscale_plane(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dst_w * dst_h) as usize];
    let sx = src_w as f32 / dst_w as f32;
    let sy = src_h as f32 / dst_h as f32;

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            // Nearest-neighbor for speed (this is a pre-downscale, GPU does quality resize)
            let src_x = ((dx as f32 + 0.5) * sx) as u32;
            let src_y = ((dy as f32 + 0.5) * sy) as u32;
            let src_x = src_x.min(src_w - 1);
            let src_y = src_y.min(src_h - 1);
            dst[(dy * dst_w + dx) as usize] = src[(src_y * src_w + src_x) as usize];
        }
    }
    dst
}

/// Decode any supported image file to YCbCr 4:2:0 for GPU processing.
///
/// Uses the universal thumbnail decoder to get RGB, then converts to YCbCr.
/// Supports HEIC, PNG, TIFF, BPG, RAW, DNG, JPEG2000, etc.
pub fn decode_any_to_ycbcr(path: &Path) -> Result<YCbCrData, String> {
    // Use the universal decoder to get RGB pixels
    let (rgb, width, height) = crate::universal_decode::decode_to_rgb(path)?;
    
    rgb_to_ycbcr420(&rgb, width, height)
}

/// Optimized decode for thumbnails - uses image::thumbnail() to avoid full decode.
///
/// For PNG/TIFF/WebP supported by `image` crate:
///   Fast path: reduced-resolution decode → RGB → YCbCr (saves 95% memory)
///
/// For HEIC files:
///   Fast path: direct YCbCr decode (no RGB conversion, preserves color accuracy)
///
/// For RAW/DNG/JPEG2000/etc. not supported by `image` crate:
///   Fallback: universal decoder → full RGB → YCbCr → maybe_downscale
pub fn decode_any_to_ycbcr_thumbnail(path: &Path) -> Result<YCbCrData, String> {
    // Check if it's a HEIC file first
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    
    if ext == "heic" || ext == "heif" {
        // Use direct HEIC YCbCr decode path (no RGB conversion needed)
        use codecs::heic::HeicCodec;
        let mut decoder = HeicCodec::new()
            .map_err(|e| format!("HEIC codec init failed: {}", e))?;
            
        let ycbcr = decoder.decode_file_ycbcr420(path)
            .map_err(|e| format!("HEIC YCbCr decode failed: {}", e))?;
            
        Ok(YCbCrData {
            y_plane: ycbcr.y_plane,
            cb_plane: ycbcr.cb_plane,
            cr_plane: ycbcr.cr_plane,
            width: ycbcr.width,
            height: ycbcr.height,
        })
    } else {
        // Try fast path with image crate's built-in thumbnail() for other formats
        match image::open(path) {
            Ok(img) => {
                let img = img.thumbnail(1024, 1024);
                let rgb_img = img.to_rgb8();
                let width = rgb_img.width();
                let height = rgb_img.height();
                let rgb = rgb_img.as_raw();
                rgb_to_ycbcr420(rgb, width, height)
            }
            Err(_) => {
                // Fallback to universal decoder for formats the image crate doesn't support
                decode_any_to_ycbcr(path)
            }
        }
    }
}

/// All-in-one GPU thumbnail generation with automatic tile recycling.
///
/// This is the recommended high-level API that:
/// 1. Decodes image to YCbCr (with automatic reduced-resolution for thumbnails)
/// 2. Uploads to GPU and resizes to 256×256 tile (brief lock)
/// 3. Reads back tile data (brief lock)
/// 4. Encodes to JPEG (NO lock - allows other threads to proceed)
/// 5. **Releases tile back to atlas** (enables unlimited throughput)
///
/// Lock splitting: Most time is spent in JPEG encoding (step 4), during which
/// other threads can upload their thumbnails. This maximizes GPU utilization.
///
/// Returns Ok(()) on success, Err(msg) on any failure.
pub fn generate_gpu_thumbnail(
    source_id: u64,
    input_path: &Path,
    output_path: &Path,
    quality: u8,
    is_jpeg: bool,
) -> Result<(), String> {
    // 1. Decode with optimized path
    let ycbcr = if is_jpeg {
        decode_jpeg_to_ycbcr_thumbnail(input_path)?
    } else {
        decode_any_to_ycbcr_thumbnail(input_path)?
    };

    let pipeline = gpu_pipeline_init()?;

    // 2+3. Upload, process, AND readback in a SINGLE lock acquisition.
    //
    // Previously this was 3 separate lock/unlock cycles:
    //   lock → process → unlock → lock → readback → unlock → lock → release → unlock
    // With 12 C# threads contending, this caused a lock convoy where threads
    // piled up waiting, creating the ~91-image stall.
    //
    // Now: one lock covers process+readback+release. The GPU work is serial
    // anyway (single compute queue), so splitting the lock bought nothing
    // but added contention overhead.
    let tile_data = {
        let mut p = pipeline.lock();
        
        // Check if we need to flush GPU operations periodically to prevent backlog
        // This helps prevent the accumulation of GPU work that causes the stall
        if p.atlas.occupied_count() > p.atlas.tile_count / 2 {
            if let Err(e) = p.flush_gpu() {
                eprintln!("[GPU] Warning: GPU flush failed: {}", e);
                // Continue anyway, as this is just a performance optimization
            }
        }
        
        let (tile_idx, data) = p.process_and_readback_tile(source_id, &ycbcr)
            .map_err(|e| format!("GPU process+readback failed: {}", e))?;
        // Release tile immediately (still under lock — negligible cost)
        p.atlas.release_tile(tile_idx);
        data
    }; // Lock released here — other threads can now use the GPU

    // 4. Encode to JPEG (NO lock - CPU-only work, other threads can use GPU)
    let tile_size = 256u32; // Standard atlas tile size
    let mut rgb_data = Vec::with_capacity((tile_size * tile_size * 3) as usize);
    for chunk in tile_data.chunks_exact(4) {
        rgb_data.push(chunk[0]); // R
        rgb_data.push(chunk[1]); // G
        rgb_data.push(chunk[2]); // B
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

/// Convert interleaved RGB to YCbCr 4:2:0 planar.
///
/// BT.601 coefficients (standard for JPEG/MPEG):
///   Y  =  0.299*R + 0.587*G + 0.114*B
///   Cb = -0.169*R - 0.331*G + 0.500*B + 128
///   Cr =  0.500*R - 0.419*G - 0.081*B + 128
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

    // Full-resolution Y plane
    let mut y_plane = Vec::with_capacity(pixel_count);
    for chunk in rgb.chunks_exact(3) {
        let r = chunk[0] as f32;
        let g = chunk[1] as f32;
        let b = chunk[2] as f32;
        let y = (0.299 * r + 0.587 * g + 0.114 * b).round().clamp(0.0, 255.0);
        y_plane.push(y as u8);
    }

    // Subsample chroma to 4:2:0 via 2x2 box filter
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
