use anyhow::{Context, Result, anyhow};
use arcmax::codec::traits::Codec;
use arcmax::codec::{LzmaCodec, LzmaOptions};
use bytemuck::cast_vec;
use codecs::bpg::{
    BPGEncoderConfig, NativeBPGEncoder, estimate_encode_peak, safe_encode_concurrency,
};
use codecs::video_analyzer::analyze_video_compression;
use image;
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex as StdMutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

const CODEC_THREAD_STACK_SIZE: usize = 16 * 1024 * 1024;
const JPEG2000_FALLBACK_QUALITY: u8 = 85;
// JP2 fallback is only kept when it beats BPG, so lower thresholds mostly cost
// CPU and temporary disk while catching more medium-sized BPG outliers. The old
// 5 MB / 0.18 Bpp thresholds only tested the largest files and missed many
// 2-5 MB phone images where JP2 can win.
const BPG_JPEG2000_FALLBACK_MIN_BYTES: u64 = 2_000_000;
const BPG_JPEG2000_FALLBACK_MIN_BYTES_PER_PIXEL: f64 = 0.08;

/// Byte-budget limiter for memory-heavy tasks.
///
/// Workers reserve an estimated peak budget before entering decode/encode
/// sections and release it automatically when done.
struct MemoryBudgetLimiter {
    available: StdMutex<u64>,
    cvar: Condvar,
    capacity: u64,
}

/// Analyze video compression with a timeout to avoid hangs
fn safe_analyze_video(path: &Path) -> Option<codecs::video_analyzer::VideoAnalysis> {
    let path = path.to_path_buf();
    let thread_path = path.clone();
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        let _ = tx.send(std::panic::catch_unwind(|| {
            analyze_video_compression(&thread_path)
        }));
    });

    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(result) => {
            let _ = handle.join();
            match result {
                Ok(Ok(v)) => Some(v),
                Ok(Err(e)) => {
                    warn!("Video analysis failed for {}: {}", path.display(), e);
                    None
                }
                Err(_) => {
                    warn!("Video analysis panicked for {}", path.display());
                    None
                }
            }
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            warn!("Video analysis timed out for {}", path.display());
            None
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => None,
    }
}

impl MemoryBudgetLimiter {
    fn new(capacity: u64) -> Self {
        Self {
            available: StdMutex::new(capacity.max(1)),
            cvar: Condvar::new(),
            capacity: capacity.max(1),
        }
    }

    fn acquire(&self, requested_bytes: u64) -> MemoryBudgetGuard<'_> {
        let request = requested_bytes.clamp(1, self.capacity);
        let mut guard = self.available.lock().unwrap();
        while *guard < request {
            guard = self.cvar.wait(guard).unwrap();
        }
        *guard -= request;
        MemoryBudgetGuard {
            limiter: self,
            reserved: request,
        }
    }
}

/// RAII guard that releases reserved memory budget when dropped.
struct MemoryBudgetGuard<'a> {
    limiter: &'a MemoryBudgetLimiter,
    reserved: u64,
}

impl<'a> Drop for MemoryBudgetGuard<'a> {
    fn drop(&mut self) {
        let mut guard = self.limiter.available.lock().unwrap();
        *guard = guard
            .saturating_add(self.reserved)
            .min(self.limiter.capacity);
        self.limiter.cvar.notify_one();
    }
}

/// Map the x265 preset (`compress_level`, 0..=9) onto the closest peak-memory
/// factor bucket understood by [`estimate_encode_peak`], which selects a
/// 10×/14×/18× planar-bytes multiplier from an `encoder_type` of 0/1/2.
///
/// The encoder *preset* — not `encoder_type` — is what actually drives x265's
/// memory use. The "best" effort runs the placebo preset (`compress_level` 9)
/// yet reports `encoder_type` 1, so keying the estimate off `encoder_type`
/// under-counts the true peak and lets too many encodes run at once. That
/// under-count is what drove the OOM kill on large HEIC batches. Bias the
/// bucket up to match the preset.
fn preset_peak_encoder_type(compress_level: i32, encoder_type: i32) -> i32 {
    let by_preset = match compress_level {
        9 => 2,     // placebo
        7 | 8 => 1, // slower / veryslow
        _ => 0,
    };
    by_preset.max(encoder_type)
}

fn estimate_image_reservation_bytes(
    input: &Path,
    original_format: OriginalImageFormat,
    original_size: u64,
    settings: &OrchestratorSettings,
) -> u64 {
    // The `image` crate cannot read HEIC, so probing it there always fails and
    // forces the worst-case fallback below. The same bpg-decode HEIF parser we
    // use to decode reads real dimensions/bit depth from the container headers
    // (no full decode), so consult it up front for an accurate reservation.
    let heic_info = if original_format == OriginalImageFormat::Heic {
        codecs::heic::HeicCodec::read_info(input).ok()
    } else {
        None
    };

    let dims = heic_info
        .map(|i| (i.width, i.height))
        .filter(|&(w, h)| w > 0 && h > 0)
        .or_else(|| image::image_dimensions(input).ok());

    let estimated_bit_depth: u8 = match (heic_info, original_format) {
        (Some(info), _) => info.bit_depth.max(8),
        (None, OriginalImageFormat::Jpeg) => 8,
        (None, OriginalImageFormat::Heic) => 12,
        (None, OriginalImageFormat::Png | OriginalImageFormat::Tiff) => 12,
        (None, _) => 10,
    };

    let peak_encoder_type =
        preset_peak_encoder_type(settings.bpg_compression_level, settings.bpg_encoder_type);

    let estimate_from_dims = |w: u32, h: u32| {
        let encode_peak = estimate_encode_peak(
            w,
            h,
            estimated_bit_depth,
            settings.bpg_chroma_format,
            peak_encoder_type,
        );

        // Conservative scratch/decode side:
        // - packed pixels for conversion
        // - additional image crate temporary buffers
        let px = u64::from(w) * u64::from(h);
        let decode_bytes = match original_format {
            OriginalImageFormat::Jpeg => px.saturating_mul(3),
            OriginalImageFormat::Heic => px.saturating_mul(3).saturating_mul(2),
            _ => {
                if estimated_bit_depth > 8 {
                    px.saturating_mul(4).saturating_mul(2)
                } else {
                    px.saturating_mul(4)
                }
            }
        };

        encode_peak
            .saturating_add(decode_bytes.saturating_mul(2))
            .saturating_add(original_size.min(256 * 1024 * 1024))
            .max(128 * 1024 * 1024)
    };

    if let Some((w, h)) = dims {
        return estimate_from_dims(w, h);
    }

    // Fallback when dimensions cannot be probed from headers.
    let baseline = estimate_encode_peak(
        6000,
        4000,
        estimated_bit_depth,
        settings.bpg_chroma_format,
        peak_encoder_type,
    );
    let fallback = baseline
        .saturating_add(original_size.saturating_mul(4))
        .max(256 * 1024 * 1024);

    if original_format == OriginalImageFormat::Heic {
        fallback.max(1024 * 1024 * 1024)
    } else {
        fallback
    }
}

use crate::archive_tracker::{ArchiveFileMapping, ArchiveRecord, ArchiveTracker};
use crate::backup_catalog::{BackupCatalog, BackupEntry, normalize_path};
use crate::bpg_wrapper::{BpgAq, BpgEffort};
use crate::file_tracker::{FileTracker, ProcessedFileRecord};
use crate::hash;

/// Cached memory reading: refresh at most once per second to avoid per-item sysinfo overhead.
/// sysinfo on Windows does Win32 API calls that can take 10–50 ms each.
struct MemoryCache {
    system: sysinfo::System,
    last_refresh: Option<Instant>,
    cached_usage: f64,
}

impl MemoryCache {
    fn new() -> Self {
        let mut system = sysinfo::System::new();
        system.refresh_memory();
        let usage = Self::compute(&system);
        Self {
            system,
            last_refresh: Some(Instant::now()),
            cached_usage: usage,
        }
    }

    fn compute(system: &sysinfo::System) -> f64 {
        let total = system.total_memory();
        if total > 0 {
            system.used_memory() as f64 / total as f64
        } else {
            0.0
        }
    }

    /// Returns cached memory usage, refreshing at most once per second.
    fn usage(&mut self) -> f64 {
        let stale = self
            .last_refresh
            .map(|t| t.elapsed() >= Duration::from_secs(1))
            .unwrap_or(true);
        if stale {
            self.system.refresh_memory();
            self.cached_usage = Self::compute(&self.system);
            self.last_refresh = Some(Instant::now());
        }
        self.cached_usage
    }
}

/// Check current memory usage and return the percentage of memory used.
/// Creates a one-shot sysinfo::System — use `MemoryCache` in hot loops.
fn check_memory_usage() -> f64 {
    use sysinfo::System;
    let mut system = System::new();
    system.refresh_memory();
    let total_memory = system.total_memory();
    let used_memory = system.used_memory();

    if total_memory > 0 {
        (used_memory as f64) / (total_memory as f64)
    } else {
        0.0
    }
}

/// Detect optimal bit depth for image encoding based on source image and format
fn detect_image_bit_depth(
    img: &image::DynamicImage,
    original_format: OriginalImageFormat,
    user_setting: i32,
    _bpg_encoder_type: i32,
) -> i32 {
    // JPEG only supports 8-bit
    if original_format == OriginalImageFormat::Jpeg {
        return 8;
    }

    // Check if the image has 16-bit channels
    let has_16bit = matches!(
        img,
        image::DynamicImage::ImageLuma16(_)
            | image::DynamicImage::ImageLumaA16(_)
            | image::DynamicImage::ImageRgb16(_)
            | image::DynamicImage::ImageRgba16(_)
    );

    if has_16bit {
        let max_depth = 12;

        // The `image` crate maps a source's declared bit depth to either an
        // 8- or 16-bit type, so reaching here means the input is a 16-bit
        // *container* (true source depth is somewhere in 9..=16, not knowable
        // more precisely). The Rust BPG encoder currently supports up to 12-bit,
        // so encoding at that cap preserves the most fidelity possible while never
        // exceeding the Rust BPG backend's current supported output depth.
        //
        // An explicit `--bpg-bit-depth` in 9..=14 is honored (capped to the
        // encoder max); the default (8) — or anything below 9 — means "auto",
        // in which case we use the encoder ceiling rather than a fixed 12.
        let preferred = match user_setting {
            9..=12 => user_setting,
            _ => max_depth,
        };

        preferred.min(max_depth).max(10)
    } else {
        // For 8-bit source images, always use 8-bit
        // (no point in encoding 8-bit data at higher bit depth)
        8
    }
}

/// Determine optimal number of encoding threads based on memory usage
fn get_optimal_thread_count(base_count: usize) -> usize {
    let memory_usage = check_memory_usage();

    if memory_usage > 0.90 {
        // Severe memory pressure - reduce to minimum threads
        (base_count / 4).max(1)
    } else if memory_usage > 0.80 {
        // Moderate memory pressure - reduce threads
        (base_count / 2).max(1)
    } else if memory_usage > 0.70 {
        // Light memory pressure - slightly reduce threads
        ((base_count as f64 * 0.75) as usize).max(1)
    } else {
        // Normal memory usage - use base count
        base_count
    }
}

/// Original image format before BPG compression
/// Used to restore files to their original format during extraction
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OriginalImageFormat {
    /// JPEG - decode BPG directly to JPEG
    Jpeg,
    /// PNG - decode BPG to PNG
    Png,
    /// HEIC/HEIF (Samsung, Android, Apple) - decode BPG → PNG
    Heic,
    /// Camera RAW formats - decode BPG to PNG (RAW cannot be recreated)
    Raw,
    /// TIFF - decode BPG to PNG (or TIFF if supported)
    Tiff,
    /// BMP - decode BPG to PNG
    Bmp,
    /// WebP - decode BPG to PNG (or WebP if supported)
    WebP,
}

impl OriginalImageFormat {
    /// Get the file extension for extraction
    pub fn extraction_extension(&self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            // HEIC encoder is not currently available in this pipeline.
            Self::Heic => "png",
            Self::Raw => "png",  // RAW cannot be recreated
            Self::Tiff => "png", // Convert to PNG for compatibility
            Self::Bmp => "png",  // Convert to PNG for compatibility
            Self::WebP => "png", // Convert to PNG for compatibility
        }
    }

    /// Should this format be encoded via PNG intermediate for quality preservation?
    pub fn needs_png_intermediate(&self) -> bool {
        match self {
            Self::Jpeg => false, // JPEG goes directly to BPG
            _ => true,           // All others go through PNG to preserve quality
        }
    }
}

/// Metadata for a compressed image file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub original_filename: String,
    pub original_format: OriginalImageFormat,
    pub original_extension: String,
    pub bpg_filename: String,
}

/// Archive metadata containing format information for all files
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArchiveMetadata {
    pub version: u32,
    pub images: Vec<ImageMetadata>,
    pub created_at: u64,
}

#[derive(Clone, Debug)]
pub struct ListedArchiveFile {
    pub filename: String,
    pub original_size: u64,
    pub compressed_size: u64,
    pub file_type: i32,
}

fn normalize_archive_rel_path(p: &str) -> String {
    let p = p.trim_start_matches("./");
    p.trim_start_matches('/').replace('\\', "/")
}

fn detect_file_type_from_name(name: &str) -> i32 {
    let lower = name.to_ascii_lowercase();
    let ext = std::path::Path::new(&lower)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match ext {
        "bpg" | "jpg" | "jpeg" | "png" | "bmp" | "tif" | "tiff" | "webp" | "heic" | "heif"
        | "ico" | "jp2" | "j2k" | "j2c" | "jpc" | "jpt" | "jph" | "jhc" | "dng" | "cr2" | "nef"
        | "arw" | "orf" | "rw2" | "raf" => 1,
        "mp4" | "mov" | "m4v" | "avi" | "mkv" | "wmv" | "webm" => 2,
        _ => 3,
    }
}

fn parse_manifest_sizes(manifest_text: &str) -> HashMap<String, (u64, u64)> {
    let mut map = HashMap::new();
    for line in manifest_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let arrow_idx = match line.find(" -> ") {
            Some(i) => i,
            None => continue,
        };
        let after_arrow = &line[(arrow_idx + 4)..];
        let open_paren = match after_arrow.find(" (") {
            Some(i) => i,
            None => continue,
        };
        let rel = after_arrow[..open_paren].trim();
        let rel = normalize_archive_rel_path(rel);

        let sizes_part = &after_arrow[(open_paren + 2)..];
        let close_paren = match sizes_part.find(')') {
            Some(i) => i,
            None => continue,
        };
        let inner = &sizes_part[..close_paren];
        let mut pieces = inner.split("->").map(|s| s.trim());
        let orig = pieces.next().and_then(|s| s.parse::<u64>().ok());
        let out = pieces.next().and_then(|s| s.parse::<u64>().ok());
        if let (Some(o), Some(c)) = (orig, out) {
            map.insert(rel, (o, c));
        }
    }
    map
}

pub fn list_archive_contents(archive_path: &Path) -> Result<Vec<ListedArchiveFile>> {
    let decoder = arcmax::tar_zst::open_zst_reader(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;
    let mut archive = tar::Archive::new(decoder);

    let mut files: Vec<(String, u64)> = Vec::new();
    let mut manifest_text: Option<String> = None;

    for entry in archive.entries().context("Failed to read tar entries")? {
        let mut entry = entry.context("Failed to read tar entry")?;
        if !entry.header().entry_type().is_file() {
            continue;
        }

        let path = entry
            .path()
            .context("Failed to read tar entry path")?
            .to_string_lossy()
            .to_string();
        let rel = normalize_archive_rel_path(&path);
        let size = entry.size();

        if rel.eq_ignore_ascii_case("MANIFEST.txt") {
            let mut buf = String::new();
            entry
                .read_to_string(&mut buf)
                .context("Failed to read MANIFEST.txt")?;
            manifest_text = Some(buf);
            continue;
        }

        files.push((rel, size));
    }

    let size_map = manifest_text
        .as_deref()
        .map(parse_manifest_sizes)
        .unwrap_or_default();

    let mut out: Vec<ListedArchiveFile>;

    if !size_map.is_empty() {
        // MANIFEST.txt is treated as the authoritative list of user-facing archive entries.
        // This avoids listing internal files like HASHES/metadata.
        out = Vec::with_capacity(size_map.len());
        for (name, (orig, comp)) in size_map {
            out.push(ListedArchiveFile {
                filename: name.clone(),
                original_size: orig,
                compressed_size: comp,
                file_type: detect_file_type_from_name(&name),
            });
        }
    } else {
        // Fallback: list tar entries but hide internal metadata.
        out = Vec::with_capacity(files.len());
        for (name, stored_size) in files {
            if name.eq_ignore_ascii_case("OPENARC_METADATA.json")
                || name.eq_ignore_ascii_case("HASHES.sha256")
                || name.eq_ignore_ascii_case("MANIFEST.txt")
                || name.eq_ignore_ascii_case("misc.arc")
                || name.eq_ignore_ascii_case("raw.arc")
            {
                continue;
            }

            out.push(ListedArchiveFile {
                filename: name.clone(),
                original_size: stored_size,
                compressed_size: stored_size,
                file_type: detect_file_type_from_name(&name),
            });
        }
    }

    out.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(out)
}

pub fn extract_archive_entry(
    archive_path: &Path,
    entry_name: &str,
    output_path: &Path,
) -> Result<()> {
    let entry_name = normalize_archive_rel_path(entry_name);

    let decoder = arcmax::tar_zst::open_zst_reader(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries().context("Failed to read tar entries")? {
        let mut entry = entry.context("Failed to read tar entry")?;
        if !entry.header().entry_type().is_file() {
            continue;
        }

        let path = entry
            .path()
            .context("Failed to read tar entry path")?
            .to_string_lossy()
            .to_string();
        let rel = normalize_archive_rel_path(&path);
        if rel != entry_name {
            continue;
        }

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create output directory: {}", parent.display())
            })?;
        }

        let mut out = std::fs::File::create(output_path)
            .with_context(|| format!("Failed to create output file: {}", output_path.display()))?;
        std::io::copy(&mut entry, &mut out)
            .with_context(|| format!("Failed to extract {}", entry_name))?;
        out.flush().ok();
        return Ok(());
    }

    Err(anyhow!("Entry not found in archive: {}", entry_name))
}

impl Default for ArchiveMetadata {
    fn default() -> Self {
        Self {
            version: 1,
            images: Vec::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct OrchestratorSettings {
    /// Internal effective BPG QP derived from `bpg_effort` unless an advanced caller overrides it.
    pub bpg_quality: i32,
    pub bpg_lossless: bool,
    pub bpg_effort: BpgEffort,
    pub bpg_aq: BpgAq,
    pub bpg_bit_depth: i32,
    pub bpg_chroma_format: i32,
    pub bpg_encoder_type: i32,
    pub bpg_compression_level: i32,
    /// ZSTD level (1-22) for the final archive container. The container wraps
    /// already-compressed BPG images, x265/AV1/x266 video, and LZMA2 bundles, so a
    /// low level (1-6) is recommended; high levels burn CPU for negligible gain.
    pub compression_level: i32,
    /// LZMA2 level (1-9) for `misc.arc`, the bundle of small/likely-uncompressible
    /// misc files (documents, configs, etc.).
    pub misc_compression_level: i32,
    pub enable_catalog: bool,
    /// Optional custom catalog database path. If unset, defaults to <archive>.catalog.sqlite
    pub catalog_db_path: Option<PathBuf>,
    pub enable_dedup: bool,
    /// Optional staging directory for temp work (defaults to system temp)
    pub staging_dir: Option<PathBuf>,
    /// Quality for HEIC re-encoding during extraction (1-100)
    pub heic_quality: u8,
    /// Quality for JPEG output during extraction (1-100)
    pub jpeg_quality: u8,
    /// Enable file-level tracking in central DB
    pub enable_tracking: bool,
    /// If false, archive files as-is without image/video transcoding
    pub reencode_media: bool,
    /// If true, write the prepared OpenArc folder layout instead of tar.zst/oarc.
    pub output_folder_without_archive: bool,
}

impl Default for OrchestratorSettings {
    fn default() -> Self {
        Self {
            bpg_quality: BpgEffort::Good.default_quality() as i32,
            bpg_lossless: false,
            bpg_effort: BpgEffort::Good,
            bpg_aq: BpgAq::default(),
            bpg_bit_depth: 8,
            bpg_chroma_format: 1,
            bpg_encoder_type: 0,
            bpg_compression_level: 8,
            compression_level: 3,
            misc_compression_level: 6,
            enable_catalog: true,
            catalog_db_path: None,
            enable_dedup: true,
            staging_dir: None,
            heic_quality: 90,
            jpeg_quality: 92,
            enable_tracking: true,
            reencode_media: true,
            output_folder_without_archive: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileClass {
    Image,
    Video,
    Raw,
    Misc,
}

pub type ProgressFn = dyn Fn(usize, usize, &str) + Send + Sync;

pub fn emit_progress(
    progress: &Option<Arc<ProgressFn>>,
    current: usize,
    total: usize,
    msg: impl AsRef<str>,
) {
    if let Some(cb) = progress {
        cb(current, total.max(1), msg.as_ref());
    }
}

fn jpeg2000_fallback_reason(width: u32, height: u32, bpg_size: u64) -> Option<&'static str> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if bpg_size >= BPG_JPEG2000_FALLBACK_MIN_BYTES {
        return Some("BPG output size threshold");
    }
    if pixels > 0 && (bpg_size as f64 / pixels as f64) >= BPG_JPEG2000_FALLBACK_MIN_BYTES_PER_PIXEL
    {
        return Some("BPG bytes-per-pixel threshold");
    }
    None
}

fn encode_jpeg2000_fallback(
    input_path: &Path,
    original_format: OriginalImageFormat,
    output_path: &Path,
) -> Result<u64> {
    let image = if original_format == OriginalImageFormat::Heic {
        let mut codec = codecs::heic::HeicCodec::new()?;
        let decoded = codec.decode_file(input_path)?;
        if decoded.has_alpha {
            let buffer = image::RgbaImage::from_raw(decoded.width, decoded.height, decoded.data)
                .ok_or_else(|| {
                    anyhow!("failed to build HEIC RGBA buffer for JPEG 2000 fallback")
                })?;
            image::DynamicImage::ImageRgba8(buffer)
        } else {
            let buffer = image::RgbImage::from_raw(decoded.width, decoded.height, decoded.data)
                .ok_or_else(|| anyhow!("failed to build HEIC RGB buffer for JPEG 2000 fallback"))?;
            image::DynamicImage::ImageRgb8(buffer)
        }
    } else {
        image::open(input_path).with_context(|| {
            format!(
                "failed to load image for JPEG 2000 fallback: {}",
                input_path.display()
            )
        })?
    };
    let encoded =
        codecs::jpeg2000::encode_dynamic_image_to_jpeg2000(&image, JPEG2000_FALLBACK_QUALITY)?;
    let size = encoded.len() as u64;
    fs::write(output_path, &encoded)
        .with_context(|| format!("Failed to write JPEG 2000 file: {}", output_path.display()))?;
    Ok(size)
}

fn keep_smaller_jpeg2000_if_flagged(
    input_path: &Path,
    original_format: OriginalImageFormat,
    bpg_path: PathBuf,
    bpg_rel: String,
    jp2_path: PathBuf,
    jp2_rel: String,
    width: u32,
    height: u32,
    stats: &Arc<parking_lot::Mutex<Jpeg2000FallbackStats>>,
) -> Result<(PathBuf, String)> {
    let bpg_size = fs::metadata(&bpg_path)?.len();
    let Some(reason) = jpeg2000_fallback_reason(width, height, bpg_size) else {
        return Ok((bpg_path, bpg_rel));
    };

    {
        let mut guard = stats.lock();
        guard.flagged_files += 1;
    }

    let jp2_size =
        encode_jpeg2000_fallback(input_path, original_format, &jp2_path).with_context(|| {
            format!(
                "Failed to encode JPEG 2000 fallback for {} after {reason}",
                input_path.display()
            )
        })?;

    if jp2_size < bpg_size {
        let _ = fs::remove_file(&bpg_path);
        let mut guard = stats.lock();
        guard.replaced_files += 1;
        guard.total_bpg_bytes_for_replaced =
            guard.total_bpg_bytes_for_replaced.saturating_add(bpg_size);
        guard.total_jpeg2000_bytes = guard.total_jpeg2000_bytes.saturating_add(jp2_size);
        guard.total_saved_bytes = guard
            .total_saved_bytes
            .saturating_add(bpg_size.saturating_sub(jp2_size));
        Ok((jp2_path, jp2_rel))
    } else {
        let _ = fs::remove_file(&jp2_path);
        Ok((bpg_path, bpg_rel))
    }
}

fn apply_bpg_settings(
    cfg: &mut BPGEncoderConfig,
    settings: &OrchestratorSettings,
    bit_depth: i32,
    chroma_format: i32,
) {
    cfg.quality = if settings.bpg_lossless {
        0
    } else {
        settings.bpg_quality
    };
    // The current bpg-rs backend uses quality 0 as the near-lossless fallback.
    cfg.lossless = 0;
    cfg.bit_depth = bit_depth;
    cfg.chroma_format = chroma_format;
    cfg.encoder_type = settings.bpg_effort.encoder_type() as i32;
    cfg.compress_level = settings.bpg_effort.compression_level() as i32;
    let (aq_mode, aq_strength, aq_clamp) =
        codecs::bpg::resolve_aq_preset(settings.bpg_aq.as_str()).unwrap_or((0, 0.0, 2));
    cfg.aq_mode = aq_mode;
    cfg.aq_strength = aq_strength;
    cfg.aq_clamp = aq_clamp;
    cfg.two_pass_gate = true;
}

#[derive(Debug, Clone)]
pub struct ProcessedFile {
    pub original_path: PathBuf,
    pub class: FileClass,
    pub archived_rel_path: String,
    pub output_path: PathBuf,
    pub original_size: u64,
    pub output_size: u64,
    pub sha256: Option<String>,
    pub skipped_processing: bool,
    pub original_format: Option<OriginalImageFormat>,
}

#[derive(Debug, Clone)]
pub struct FailedFile {
    pub original_path: PathBuf,
    pub class: FileClass,
    pub error: String,
}

#[derive(Debug, Clone, Default)]
pub struct Jpeg2000FallbackStats {
    pub flagged_files: usize,
    pub replaced_files: usize,
    pub total_bpg_bytes_for_replaced: u64,
    pub total_jpeg2000_bytes: u64,
    pub total_saved_bytes: u64,
}

impl Jpeg2000FallbackStats {
    pub fn average_saved_bytes(&self) -> u64 {
        if self.replaced_files == 0 {
            0
        } else {
            self.total_saved_bytes / self.replaced_files as u64
        }
    }

    pub fn average_saved_percent(&self) -> f64 {
        if self.total_bpg_bytes_for_replaced == 0 {
            0.0
        } else {
            (self.total_saved_bytes as f64 / self.total_bpg_bytes_for_replaced as f64) * 100.0
        }
    }
}

#[derive(Debug)]
pub struct OrchestratorResult {
    pub discovered_files: Vec<PathBuf>,
    pub processed: Vec<ProcessedFile>,
    pub failed: Vec<FailedFile>,
    pub skipped_by_catalog: Vec<PathBuf>,
    pub dedup_groups: usize,
    pub tracking_report: Option<String>,
    pub staged_uncompressed_videos: Vec<PathBuf>,
    pub jpeg2000_fallback: Jpeg2000FallbackStats,
}

#[derive(Clone, Debug)]
struct WorkItem {
    idx: usize,
    input: PathBuf,
    source_rel_path: String,
    class: FileClass,
    original_format: Option<OriginalImageFormat>,
    /// SHA-256 of the source file from the Phase-1 hashing pass, when available.
    /// For arms that archive the file as a byte-for-byte copy this doubles as
    /// the output hash, saving a full second read of the file.
    source_hash: Option<String>,
}

#[derive(Clone, Debug)]
struct WorkDone {
    idx: usize,
    file_name: String,
}

pub fn collect_files(input_paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for path in input_paths {
        if path.is_file() {
            files.push(path.clone());
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                files.push(entry.path().to_path_buf());
            }
        }
    }
    files.sort_by_cached_key(|p| normalize_archive_rel_path(&p.to_string_lossy()));
    Ok(files)
}

fn append_rel_suffix(rel: &str, idx: usize) -> String {
    let path = Path::new(rel);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let file_name = if ext.is_empty() {
        format!("{}_{}", stem, idx)
    } else {
        format!("{}_{}.{}", stem, idx, ext)
    };
    if let Some(parent) = path.parent() {
        let parent_s = normalize_archive_rel_path(&parent.to_string_lossy());
        if parent_s.is_empty() || parent_s == "." {
            file_name
        } else {
            format!("{}/{}", parent_s, file_name)
        }
    } else {
        file_name
    }
}

fn choose_source_rel_path(file: &Path, input_paths: &[PathBuf]) -> String {
    let mut best: Option<PathBuf> = None;

    for root in input_paths {
        if root.is_file() {
            if root == file {
                let rel = file
                    .file_name()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("file"));
                if best
                    .as_ref()
                    .map(|b| rel.components().count() > b.components().count())
                    .unwrap_or(true)
                {
                    best = Some(rel);
                }
            }
            continue;
        }

        if root.is_dir() {
            if let Ok(rel) = file.strip_prefix(root) {
                let rel_buf = rel.to_path_buf();
                if best
                    .as_ref()
                    .map(|b| rel_buf.components().count() > b.components().count())
                    .unwrap_or(true)
                {
                    best = Some(rel_buf);
                }
            }
        }
    }

    let rel = best.unwrap_or_else(|| {
        file.file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("file"))
    });
    normalize_archive_rel_path(&rel.to_string_lossy())
}

fn build_relative_path_map(
    discovered: &[PathBuf],
    input_paths: &[PathBuf],
) -> HashMap<PathBuf, String> {
    let mut map = HashMap::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for file in discovered {
        let base_rel = choose_source_rel_path(file, input_paths);
        let count = seen.entry(base_rel.clone()).or_insert(0);
        let unique_rel = if *count == 0 {
            base_rel.clone()
        } else {
            append_rel_suffix(&base_rel, *count)
        };
        *count += 1;
        map.insert(file.clone(), unique_rel);
    }

    map
}

/// Classify file and determine original format
fn classify_file(path: &Path) -> (FileClass, Option<OriginalImageFormat>) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        // JPEG - direct encoding to BPG
        "jpg" | "jpeg" => (FileClass::Image, Some(OriginalImageFormat::Jpeg)),

        // PNG - encode via PNG (already PNG, so direct)
        "png" => (FileClass::Image, Some(OriginalImageFormat::Png)),

        // HEIC/HEIF (Samsung, Android, Apple) - encode via PNG intermediate
        "heic" | "heif" | "hif" => (FileClass::Image, Some(OriginalImageFormat::Heic)),

        // Camera RAW formats - encode via PNG intermediate
        "cr2" | "cr3" | "nef" | "arw" | "dng" | "orf" | "rw2" | "raf" | "pef" | "srw" => {
            (FileClass::Raw, Some(OriginalImageFormat::Raw))
        }

        // TIFF - encode via PNG intermediate
        "tiff" | "tif" => (FileClass::Image, Some(OriginalImageFormat::Tiff)),

        // BMP - encode via PNG intermediate
        "bmp" => (FileClass::Image, Some(OriginalImageFormat::Bmp)),

        // WebP - encode via PNG intermediate
        "webp" => (FileClass::Image, Some(OriginalImageFormat::WebP)),

        // Video formats
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" | "3gp" | "flv" | "wmv" | "mts" | "m2ts" => {
            (FileClass::Video, None)
        }

        // Everything else
        _ => (FileClass::Misc, None),
    }
}

fn safe_file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string())
}

pub fn create_archive(
    input_paths: &[PathBuf],
    output_archive: &Path,
    settings: OrchestratorSettings,
    progress: Option<Arc<ProgressFn>>,
) -> Result<OrchestratorResult> {
    emit_progress(&progress, 0, 1, "Discovering files...");
    let discovered = collect_files(input_paths)?;
    emit_progress(
        &progress,
        0,
        discovered.len().max(1),
        format!("Discovered {} files", discovered.len()),
    );
    if discovered.is_empty() {
        return Ok(OrchestratorResult {
            discovered_files: Vec::new(),
            processed: Vec::new(),
            failed: Vec::new(),
            skipped_by_catalog: Vec::new(),
            dedup_groups: 0,
            tracking_report: None,
            staged_uncompressed_videos: Vec::new(),
            jpeg2000_fallback: Jpeg2000FallbackStats::default(),
        });
    }

    let rel_path_map = build_relative_path_map(&discovered, input_paths);

    // Phase 1: Hash all files once — results shared by both tracking and dedup.
    // Hashing is I/O + SHA-256 bound and per-file independent, so fan it out
    // across the CPU; on NVMe this is several times faster than sequential.
    let mut file_hashes: HashMap<PathBuf, String> = HashMap::new();
    if settings.enable_tracking || settings.enable_dedup {
        use rayon::prelude::*;
        let hashed_count = std::sync::atomic::AtomicUsize::new(0);
        let total_files = discovered.len();
        file_hashes = discovered
            .par_iter()
            .filter_map(|p| {
                let done = hashed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if done == 0 || done + 1 == total_files || done % 16 == 0 {
                    emit_progress(
                        &progress,
                        done,
                        total_files,
                        format!("Hashing: {}", safe_file_name(p)),
                    );
                }
                hash::sha256_file_hex(p).ok().map(|h| (p.clone(), h))
            })
            .collect();
    }

    let tracker = if settings.enable_tracking {
        FileTracker::new().ok()
    } else {
        None
    };
    let tracking_duplicates = if let Some(ref tracker) = tracker {
        let unique_hashes: Vec<String> = file_hashes.values().cloned().collect();
        tracker.find_duplicates(&unique_hashes).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let catalog_path = settings
        .catalog_db_path
        .clone()
        .unwrap_or_else(|| output_archive.with_extension("catalog.sqlite"));
    let mut catalog = if settings.enable_catalog {
        if let Some(parent) = catalog_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create catalog directory {}", parent.display())
            })?;
        }
        Some(BackupCatalog::new(&catalog_path)?)
    } else {
        None
    };

    emit_progress(&progress, 0, discovered.len(), "Checking catalog...");
    let (skipped_by_catalog, to_process) = if let Some(ref cat) = catalog {
        cat.filter_files_to_backup(discovered.clone())?
    } else {
        (Vec::new(), discovered.clone())
    };

    let total = discovered.len();
    emit_progress(&progress, 0, total, "Preparing work queue...");

    let mut dedup_canon: HashMap<String, PathBuf> = HashMap::new();
    let mut duplicates_of: HashMap<PathBuf, PathBuf> = HashMap::new();

    if settings.enable_dedup {
        for p in &to_process {
            // Reuse the hash computed during Phase 1 — no second disk read.
            let h = if let Some(cached) = file_hashes.get(p) {
                cached.clone()
            } else {
                hash::sha256_file_hex(p)?
            };
            if let Some(prev) = dedup_canon.get(&h) {
                duplicates_of.insert(p.clone(), prev.clone());
            } else {
                dedup_canon.insert(h, p.clone());
            }
        }
    }

    let skipped_set: std::collections::HashSet<&PathBuf> = skipped_by_catalog.iter().collect();
    let mut work: Vec<WorkItem> = Vec::new();
    for (idx, p) in discovered.iter().enumerate() {
        if skipped_set.contains(p) {
            continue;
        }
        if settings.enable_dedup {
            if let Some(canon) = duplicates_of.get(p) {
                if canon != p {
                    continue;
                }
            }
        }
        let (class, original_format) = classify_file(p);
        work.push(WorkItem {
            idx,
            input: p.clone(),
            source_rel_path: rel_path_map
                .get(p)
                .cloned()
                .unwrap_or_else(|| safe_file_name(p)),
            class,
            original_format,
            source_hash: file_hashes.get(p).cloned(),
        });
    }

    let staging_root = settings
        .staging_dir
        .clone()
        .unwrap_or_else(std::env::temp_dir);
    let temp_dir = tempfile::Builder::new()
        .prefix("openarc")
        .tempdir_in(&staging_root)
        .with_context(|| format!("Failed to create temp dir in {}", staging_root.display()))?;

    let workspace_dir = if settings.output_folder_without_archive {
        fs::create_dir_all(output_archive).with_context(|| {
            format!(
                "Failed to create output folder {}",
                output_archive.display()
            )
        })?;
        output_archive.to_path_buf()
    } else {
        temp_dir.path().to_path_buf()
    };

    let media_dir = workspace_dir.join("media");
    let misc_dir = workspace_dir.join("misc");
    let raw_dir = workspace_dir.join("raw");
    fs::create_dir_all(&media_dir)?;
    fs::create_dir_all(&misc_dir)?;
    let has_raw_files = work.iter().any(|w| w.class == FileClass::Raw);
    if has_raw_files {
        fs::create_dir_all(&raw_dir)?;
    }

    let processed_mutex = Arc::new(parking_lot::Mutex::new(Vec::<ProcessedFile>::new()));
    let failed_mutex = Arc::new(parking_lot::Mutex::new(Vec::<FailedFile>::new()));
    let metadata_mutex = Arc::new(parking_lot::Mutex::new(ArchiveMetadata::default()));
    let staged_video_mutex = Arc::new(parking_lot::Mutex::new(Vec::<PathBuf>::new()));
    let jpeg2000_stats_mutex = Arc::new(parking_lot::Mutex::new(Jpeg2000FallbackStats::default()));
    let completed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let stage_label = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let video_stage_dir = if settings.output_folder_without_archive {
        workspace_dir.join("videos")
    } else {
        exe_dir
            .join("openarc_video_staging")
            .join(stage_label.to_string())
    };
    fs::create_dir_all(&video_stage_dir)?;

    let (tx, rx) = flume::unbounded::<WorkDone>();
    let progress_clone = progress.clone();
    let work_total = work.len();
    let progress_total = work_total + 5;
    emit_progress(&progress, 0, progress_total, "Processing media...");
    let progress_thread = std::thread::spawn(move || {
        if let Some(cb) = progress_clone {
            while let Ok(done) = rx.recv() {
                cb(done.idx + 1, progress_total, &done.file_name);
            }
        } else {
            while rx.recv().is_ok() {}
        }
    });

    let settings_clone = settings.clone();
    // Size scheduler backpressure from a "typical" 24 MPix image (6000×4000).
    let mut system_info = sysinfo::System::new();
    system_info.refresh_memory();
    let total_ram = system_info.total_memory();
    let available_ram = system_info.available_memory();
    // Budget from *currently available* RAM, not total. Other applications may
    // already hold a large share, and the C/x265 backend allocates aggressively;
    // committing 75% of total RAM regardless of what is free is what pushed the
    // machine into the OOM killer mid-batch. Use up to 70% of what is actually
    // free, capped at 75% of total, with a floor so we always make progress.
    let ram_budget = available_ram
        .saturating_mul(7)
        .saturating_div(10)
        .min(total_ram.saturating_mul(3) / 4)
        .max(512 * 1024 * 1024);
    let base_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let optimal_threads = get_optimal_thread_count(base_threads);
    // Key concurrency off the preset the encoder will actually run (see
    // `preset_peak_encoder_type`), not the reported encoder_type.
    let peak_encoder_type =
        preset_peak_encoder_type(settings.bpg_compression_level, settings.bpg_encoder_type);
    let image_heavy_capacity = safe_encode_concurrency(
        6000,
        4000,
        8,
        settings.bpg_chroma_format,
        peak_encoder_type,
        ram_budget,
    )
    .max(1);
    let memory_limiter = Arc::new(MemoryBudgetLimiter::new(ram_budget));
    let pipeline_runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(optimal_threads)
        .thread_stack_size(CODEC_THREAD_STACK_SIZE)
        .enable_all()
        .build()
        .context("Failed to create Tokio archive pipeline runtime")?;

    // Per-thread memory cache — refreshes at most once/second, avoiding a sysinfo syscall
    // on every image.  `thread_local!` gives each blocking worker its own copy.
    thread_local! {
        static MEM: std::cell::RefCell<MemoryCache> =
            std::cell::RefCell::new(MemoryCache::new());
    }

    pipeline_runtime.block_on(async {
        let mut tasks = JoinSet::new();
        let mut next_item = 0usize;
        let cpu_bounded_in_flight = optimal_threads.div_ceil(2).max(1);
        let max_in_flight = cpu_bounded_in_flight
            .min(image_heavy_capacity.saturating_mul(2).max(1))
            .max(1);

        loop {
            while next_item < work.len() && tasks.len() < max_in_flight {
                let item = work[next_item].clone();
                next_item += 1;

                let settings_clone = settings_clone.clone();
                let memory_limiter = memory_limiter.clone();
                let video_stage_dir = video_stage_dir.clone();
                let media_dir = media_dir.clone();
                let misc_dir = misc_dir.clone();
                let raw_dir = raw_dir.clone();
                let processed_mutex = processed_mutex.clone();
                let failed_mutex = failed_mutex.clone();
                let metadata_mutex = metadata_mutex.clone();
                let staged_video_mutex = staged_video_mutex.clone();
                let jpeg2000_stats_mutex = jpeg2000_stats_mutex.clone();
                let completed_count = completed_count.clone();
                let tx = tx.clone();

                tasks.spawn_blocking(move || -> Result<()> {
        let input = &item.input;
        let file_name = safe_file_name(input);

        let worker_result: Result<()> = (|| {
        // Backpressure safety valve (the budget limiter below is the primary
        // gate). If the machine is critically low on memory, wait for in-flight
        // encodes to release before starting another, rather than piling on and
        // tripping the OOM killer. Bounded so we never deadlock if memory stays
        // high for reasons outside our control.
        {
            let cap = std::time::Duration::from_secs(30);
            let mut waited = std::time::Duration::ZERO;
            loop {
                let memory_usage = MEM.with(|m| m.borrow_mut().usage());
                if memory_usage <= 0.90 || waited >= cap {
                    if memory_usage > 0.85 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
                waited += std::time::Duration::from_millis(250);
            }
        }

        let original_size = fs::metadata(input)?.len();
        let source_rel_path = item.source_rel_path.clone();
        let source_rel = Path::new(&source_rel_path);

        let stage_at = |base: &Path, rel: &str| -> Result<PathBuf> {
            let p = base.join(Path::new(rel));
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent)?;
            }
            Ok(p)
        };

        let (out_path, rel_path, skipped_processing, original_format, archived_class, is_byte_copy) = match item.class {
            FileClass::Image => {
                let original_format = item.original_format.unwrap_or(OriginalImageFormat::Png);
                let original_ext = input.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
                    .to_lowercase();
                let encoded_rel = normalize_archive_rel_path(&source_rel.with_extension("bpg").to_string_lossy());
                let jpeg2000_rel = normalize_archive_rel_path(&source_rel.with_extension("jp2").to_string_lossy());

                if !settings_clone.reencode_media {
                    // Archive image bytes as-is (no transcoding).
                    let source_rel = normalize_archive_rel_path(&source_rel.to_string_lossy());
                    let out = stage_at(&media_dir, &source_rel)?;
                    fs::copy(input, &out)?;
                    let rel_path = format!("media/{}", source_rel);
                    (out, rel_path, true, Some(original_format), FileClass::Image, true)
                } else {
                let out = stage_at(&media_dir, &encoded_rel)?;
                let jpeg2000_out = stage_at(&media_dir, &jpeg2000_rel)?;

                // Resume support: when writing a folder layout (no final
                // archive), a previously interrupted run may already have
                // produced this image's output directly in the destination.
                // Reuse it instead of re-encoding so a killed job can be
                // restarted by simply pointing at the same output folder.
                // (Archive mode stages into a fresh temp dir each run, so there
                // is nothing to resume from there.)
                if settings_clone.output_folder_without_archive {
                    let prior = if fs::metadata(&out).map(|m| m.len() > 0).unwrap_or(false) {
                        Some((out.clone(), encoded_rel.clone()))
                    } else if fs::metadata(&jpeg2000_out).map(|m| m.len() > 0).unwrap_or(false) {
                        Some((jpeg2000_out.clone(), jpeg2000_rel.clone()))
                    } else {
                        None
                    };
                    if let Some((prior_out, prior_rel)) = prior {
                        {
                            let mut meta = metadata_mutex.lock();
                            meta.images.push(ImageMetadata {
                                original_filename: file_name.clone(),
                                original_format,
                                original_extension: original_ext.clone(),
                                bpg_filename: prior_rel.clone(),
                            });
                        }
                        let rel_path = format!("media/{}", prior_rel);
                        let output_size = fs::metadata(&prior_out)?.len();
                        let sha = hash::sha256_file_hex(&prior_out).ok();
                        {
                            let mut guard = processed_mutex.lock();
                            guard.push(ProcessedFile {
                                original_path: input.clone(),
                                class: FileClass::Image,
                                archived_rel_path: rel_path,
                                output_path: prior_out,
                                original_size,
                                output_size,
                                sha256: sha,
                                skipped_processing: true,
                                original_format: Some(original_format),
                            });
                        }
                        return Ok(());
                    }
                }

                // Reserve memory budget per image based on dimensions/format.
                let image_reservation = estimate_image_reservation_bytes(
                    input,
                    original_format,
                    original_size,
                    &settings_clone,
                );
                let _memory_guard = memory_limiter.acquire(image_reservation);

                // For HEIC files, use direct YCbCr encoding (no RGB conversion, no intermediate PNG)
                // Pure Rust decoder is always available - provides perfect color accuracy
                //
                // For JPEG files, attempt direct YCbCr path too (no colorspace conversion).
                // If YCbCr decode fails (CMYK JPEG, etc.), falls through to generic RGB path.
                let jpeg_ycbcr: Option<crate::jpeg_decoder::YCbCrImage> =
                    if original_format == OriginalImageFormat::Jpeg {
                        fs::read(input).ok()
                            .and_then(|data| crate::jpeg_decoder::decode_jpeg_ycbcr(&data).ok())
                    } else {
                        None
                    };

                if original_format == OriginalImageFormat::Heic {
                    use codecs::heic::{matrix_coeffs_to_bpg_color_space, HeicChromaFormat, HeicCodec};
                    use codecs::bpg::NativeBPGEncoder;

                    let mut codec = HeicCodec::new()?;
                    let decoded = codec.decode_file_yuv(input)
                        .with_context(|| format!("Failed to decode HEIC file: {}", input.display()))?;

                    let bpg_format = match decoded.chroma_format {
                        HeicChromaFormat::Monochrome => codecs::bpg::BPGImageFormat::Gray,
                        HeicChromaFormat::YCbCr420 => codecs::bpg::BPGImageFormat::YCbCr420P,
                        HeicChromaFormat::YCbCr422 => codecs::bpg::BPGImageFormat::YCbCr422P,
                        HeicChromaFormat::YCbCr444 => codecs::bpg::BPGImageFormat::YCbCr444P,
                    };

                    // Encode directly from the decoded HEIC planes, preserving source fidelity.
                    let mut encoder = NativeBPGEncoder::new()
                        .context("Failed to create BPG encoder")?;
                    let mut cfg = NativeBPGEncoder::default_config();
                    if settings_clone.bpg_lossless {
                        warn!("Lossless BPG encoding not yet supported, using quality=0 (near-lossless high quality) instead for: {}", input.display());
                    }
                    apply_bpg_settings(
                        &mut cfg,
                        &settings_clone,
                        decoded.bit_depth as i32,
                        decoded.chroma_format.to_bpg_chroma_format(),
                    );
                    cfg.color_space = matrix_coeffs_to_bpg_color_space(decoded.matrix_coeffs);
                    cfg.limited_range = if decoded.full_range { 0 } else { 1 };
                    encoder.set_config(&cfg)
                        .context("Failed to apply BPG config")?;

                    let bpg_data = encoder.encode_from_planar_u16(
                        &decoded.y_plane,
                        &decoded.cb_plane,
                        &decoded.cr_plane,
                        decoded.width,
                        decoded.height,
                        decoded.y_stride,
                        decoded.cb_stride,
                        decoded.cr_stride,
                        bpg_format,
                    ).with_context(|| format!("Failed to encode HEIC to BPG via planar YUV: {}", input.display()))?;

                    fs::write(&out, &bpg_data)
                        .with_context(|| format!("Failed to write BPG file: {}", out.display()))?;
                    drop(bpg_data);

                    let (selected_out, selected_encoded_rel) = keep_smaller_jpeg2000_if_flagged(
                        input,
                        original_format,
                        out,
                        encoded_rel.clone(),
                        jpeg2000_out,
                        jpeg2000_rel.clone(),
                        decoded.width,
                        decoded.height,
                        &jpeg2000_stats_mutex,
                    )?;

                    // Record metadata
                    {
                        let mut meta = metadata_mutex.lock();
                        meta.images.push(ImageMetadata {
                            original_filename: file_name.clone(),
                            original_format,
                            original_extension: original_ext.clone(),
                            bpg_filename: selected_encoded_rel.clone(),
                        });
                    }

                    let rel_path = format!("media/{}", selected_encoded_rel);

                    // HEIC processing complete - return from match arm
                    (selected_out, rel_path, false, Some(original_format), FileClass::Image, false)
                } else if let Some(ycbcr) = jpeg_ycbcr {
                    // JPEG: direct YCbCr 4:2:0 → BPG — no colorspace conversion.
                    use codecs::bpg::NativeBPGEncoder;

                    let mut encoder = NativeBPGEncoder::new()
                        .context("Failed to create BPG encoder")?;
                    let mut cfg = NativeBPGEncoder::default_config();
                    if settings_clone.bpg_lossless {
                        warn!("Lossless BPG encoding not yet supported, using quality=0 (near-lossless high quality) instead for: {}", input.display());
                    }
                    apply_bpg_settings(&mut cfg, &settings_clone, 8, 0); // JPEG is 8-bit YCbCr 4:2:0
                    cfg.color_space = 0;   // YCbCr BT.601 (standard JPEG color space)
                    cfg.limited_range = 0; // JPEG uses full range
                    encoder.set_config(&cfg)
                        .context("Failed to apply BPG config")?;

                    let bpg_data = encoder.encode_from_ycbcr420_planar(
                        &ycbcr.y_plane,
                        &ycbcr.cb_plane,
                        &ycbcr.cr_plane,
                        ycbcr.width,
                        ycbcr.height,
                        ycbcr.y_stride,
                        ycbcr.cb_stride,
                        ycbcr.cr_stride,
                    ).with_context(|| format!("Failed to encode JPEG to BPG via YCbCr: {}", input.display()))?;

                    fs::write(&out, &bpg_data)
                        .with_context(|| format!("Failed to write BPG file: {}", out.display()))?;

                    drop(bpg_data);

                    let (selected_out, selected_encoded_rel) = keep_smaller_jpeg2000_if_flagged(
                        input,
                        original_format,
                        out,
                        encoded_rel.clone(),
                        jpeg2000_out,
                        jpeg2000_rel.clone(),
                        ycbcr.width,
                        ycbcr.height,
                        &jpeg2000_stats_mutex,
                    )?;

                    // Record metadata
                    {
                        let mut meta = metadata_mutex.lock();
                        meta.images.push(ImageMetadata {
                            original_filename: file_name.clone(),
                            original_format,
                            original_extension: original_ext.clone(),
                            bpg_filename: selected_encoded_rel.clone(),
                        });
                    }

                    let rel_path = format!("media/{}", selected_encoded_rel);

                    (selected_out, rel_path, false, Some(original_format), FileClass::Image, false)
                } else {
                    // Generic RGB path for all other formats (PNG, TIFF, BMP, WebP, etc.)
                    // Also used as fallback for JPEGs where YCbCr decode failed.
                    let img_result = image::open(input).map_err(|e| anyhow::anyhow!(e));

                // If the image can't be decoded (corrupt/truncated), copy the original
                // file as-is to preserve it in the archive without BPG encoding.
                let img = match img_result {
                    Ok(img) => img,
                    Err(_) => {
                        let source_rel = normalize_archive_rel_path(&source_rel.with_extension(&original_ext).to_string_lossy());
                        let copy_out = stage_at(&media_dir, &source_rel)?;
                        fs::copy(input, &copy_out)
                            .with_context(|| format!("Failed to copy unreadable image: {}", input.display()))?;
                        let rel_path = format!("media/{}", source_rel);
                        return Ok({
                            let output_size = fs::metadata(&copy_out)?.len();
                            let sha = item
                                .source_hash
                                .clone()
                                .or_else(|| hash::sha256_file_hex(&copy_out).ok());
                            {
                                let mut guard = processed_mutex.lock();
                                guard.push(ProcessedFile {
                                    original_path: input.clone(),
                                    class: item.class,
                                    archived_rel_path: rel_path,
                                    output_path: copy_out,
                                    original_size,
                                    output_size,
                                    sha256: sha,
                                    skipped_processing: true,
                                    original_format: Some(original_format),
                                });
                            }
                            ()
                        });
                    }
                };

                // Convert to RGB8 or RGBA8 for BPG encoding
                let target_bit_depth = detect_image_bit_depth(
                    &img,
                    original_format,
                    settings_clone.bpg_bit_depth,
                    settings_clone.bpg_encoder_type,
                );
                let wants_high_depth = target_bit_depth > 8;

                // Consume `img` by value so already-matching layouts hand over
                // their pixel buffer instead of cloning it (a 16-bit 24 MPix
                // image is ~140 MB — cloning it doubled peak memory per worker).
                let (width, height, pixel_data, format, bytes_per_sample) = if wants_high_depth {
                    match img {
                        image::DynamicImage::ImageRgb16(rgb) => {
                            let (w, h) = rgb.dimensions();
                            let data = cast_vec(rgb.into_raw());
                            (w, h, data, codecs::bpg::BPGImageFormat::RGB24, 2u32)
                        }
                        image::DynamicImage::ImageRgba16(rgba) => {
                            let (w, h) = rgba.dimensions();
                            let data = cast_vec(rgba.into_raw());
                            (w, h, data, codecs::bpg::BPGImageFormat::RGBA32, 2u32)
                        }
                        other => {
                            let rgb = other.to_rgb16();
                            let (w, h) = rgb.dimensions();
                            let data = cast_vec(rgb.into_raw());
                            (w, h, data, codecs::bpg::BPGImageFormat::RGB24, 2u32)
                        }
                    }
                } else {
                    match img {
                        image::DynamicImage::ImageRgb8(rgb) => {
                            let (w, h) = rgb.dimensions();
                            (w, h, rgb.into_raw(), codecs::bpg::BPGImageFormat::RGB24, 1u32)
                        }
                        image::DynamicImage::ImageRgba8(rgba) => {
                            let (w, h) = rgba.dimensions();
                            (w, h, rgba.into_raw(), codecs::bpg::BPGImageFormat::RGBA32, 1u32)
                        }
                        other => {
                            let rgb = other.to_rgb8();
                            let (w, h) = rgb.dimensions();
                            (w, h, rgb.into_raw(), codecs::bpg::BPGImageFormat::RGB24, 1u32)
                        }
                    }
                };

                // Encode to BPG in-memory
                let mut enc = NativeBPGEncoder::new().context("Failed to create BPG encoder")?;
                let mut cfg: BPGEncoderConfig = NativeBPGEncoder::default_config();
                if settings_clone.bpg_lossless {
                    warn!("Lossless BPG encoding not yet supported, using quality=0 (near-lossless high quality) instead for: {}", input.display());
                }

                // Auto-detect optimal bit depth based on source image.
                apply_bpg_settings(
                    &mut cfg,
                    &settings_clone,
                    target_bit_depth,
                    settings_clone.bpg_chroma_format,
                );
                enc.set_config(&cfg).context("Failed to apply BPG config")?;

                // Use in-memory encoding
                let channels = if format as i32 == codecs::bpg::BPGImageFormat::RGB24 as i32 { 3 } else { 4 };
                let stride = width * channels * bytes_per_sample;
                let bpg_data = enc.encode_from_memory(
                    &pixel_data,
                    width,
                    height,
                    stride,
                    format,
                ).with_context(|| format!("Failed to encode {} to BPG", input.display()))?;

                // Write BPG data to output file
                fs::write(&out, &bpg_data)
                    .with_context(|| format!("Failed to write BPG file: {}", out.display()))?;

                let (selected_out, selected_encoded_rel) = keep_smaller_jpeg2000_if_flagged(
                    input,
                    original_format,
                    out,
                    encoded_rel.clone(),
                    jpeg2000_out,
                    jpeg2000_rel.clone(),
                    width,
                    height,
                    &jpeg2000_stats_mutex,
                )?;

                // Record metadata for extraction
                {
                    let mut meta = metadata_mutex.lock();
                    meta.images.push(ImageMetadata {
                        original_filename: file_name.clone(),
                        original_format,
                        original_extension: original_ext,
                        bpg_filename: selected_encoded_rel.clone(),
                    });
                }

                // Explicitly drop large data structures to free memory immediately
                drop(pixel_data);
                drop(bpg_data);

                // Periodic cleanup check - yield to allow other threads to run
                if item.idx % 10 == 0 {  // Every 10th item
                    std::thread::yield_now();
                }

                let rel_path = format!("media/{}", selected_encoded_rel);
                (selected_out, rel_path, false, Some(original_format), FileClass::Image, false)
                }  // End of else block for non-HEIC image processing
                }
            }
            FileClass::Video => {
                let should_archive_as_misc = safe_analyze_video(input)
                    .map(|a| a.is_efficiently_compressed)
                    .unwrap_or(false);

                if should_archive_as_misc {
                    let source_rel = normalize_archive_rel_path(&source_rel.to_string_lossy());
                    let out = stage_at(&misc_dir, &source_rel)?;
                    fs::copy(input, &out)?;
                    let rel_path = format!("misc/{}", source_rel);
                    (out, rel_path, true, None, FileClass::Misc, true)
                } else {
                    let source_rel = normalize_archive_rel_path(&source_rel.to_string_lossy());
                    let out = stage_at(&video_stage_dir, &source_rel)?;
                    fs::copy(input, &out)?;
                    {
                        let mut guard = staged_video_mutex.lock();
                        guard.push(out.clone());
                    }
                    return Ok(());
                }
            }
            FileClass::Raw => {
                let source_rel = normalize_archive_rel_path(&source_rel.to_string_lossy());
                let out = stage_at(&raw_dir, &source_rel)?;
                fs::copy(input, &out)?;
                let rel_path = format!("raw/{}", source_rel);
                (out, rel_path, true, Some(OriginalImageFormat::Raw), FileClass::Raw, true)
            }
            FileClass::Misc => {
                let source_rel = normalize_archive_rel_path(&source_rel.to_string_lossy());
                let out = stage_at(&misc_dir, &source_rel)?;
                fs::copy(input, &out)?;
                let rel_path = format!("misc/{}", source_rel);
                (out, rel_path, false, None, FileClass::Misc, true)
            }
        };

        let output_size = fs::metadata(&out_path)?.len();
        // Byte-for-byte copies have the same hash as the source, which Phase 1
        // already computed — reuse it instead of reading the file again.
        let sha = if is_byte_copy {
            item.source_hash
                .clone()
                .or_else(|| hash::sha256_file_hex(&out_path).ok())
        } else {
            hash::sha256_file_hex(&out_path).ok()
        };

        {
            let mut guard = processed_mutex.lock();
            guard.push(ProcessedFile {
                original_path: input.clone(),
                    class: archived_class,
                archived_rel_path: rel_path,
                output_path: out_path,
                original_size,
                output_size,
                sha256: sha,
                skipped_processing,
                original_format,
            });
        }

        Ok(())
        })();

        match worker_result {
            Ok(()) => {
                let seq = completed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let _ = tx.send(WorkDone { idx: seq, file_name });
            }
            Err(err) => {
                let err_text = format!("{err:#}");
                warn!("Failed processing {}: {}", input.display(), err_text);
                {
                    let mut guard = failed_mutex.lock();
                    guard.push(FailedFile {
                        original_path: input.clone(),
                        class: item.class,
                        error: err_text,
                    });
                }
                let seq = completed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let _ = tx.send(WorkDone {
                    idx: seq,
                    file_name: format!("FAILED: {file_name}"),
                });
            }
        }

        Ok(())
                });
            }

            if tasks.is_empty() {
                break;
            }

            if let Some(result) = tasks.join_next().await {
                if let Err(e) = result {
                    warn!("Archive pipeline worker join failure: {}", e);
                }
            }
        }

        Ok::<(), anyhow::Error>(())
    })?;

    drop(tx);
    let _ = progress_thread.join();

    let mut processed = Arc::try_unwrap(processed_mutex)
        .map_err(|_| anyhow!("Failed to unwrap processed results"))?
        .into_inner();
    processed.sort_by(|a, b| a.archived_rel_path.cmp(&b.archived_rel_path));

    let mut failed = Arc::try_unwrap(failed_mutex)
        .map_err(|_| anyhow!("Failed to unwrap failed results"))?
        .into_inner();
    failed.sort_by(|a, b| a.original_path.cmp(&b.original_path));

    let mut staged_uncompressed_videos = Arc::try_unwrap(staged_video_mutex)
        .map_err(|_| anyhow!("Failed to unwrap staged video results"))?
        .into_inner();
    staged_uncompressed_videos.sort();

    let jpeg2000_fallback = Arc::try_unwrap(jpeg2000_stats_mutex)
        .map_err(|_| anyhow!("Failed to unwrap JPEG 2000 fallback stats"))?
        .into_inner();

    let metadata = Arc::try_unwrap(metadata_mutex)
        .map_err(|_| anyhow!("Failed to unwrap metadata"))?
        .into_inner();

    // Write metadata JSON
    emit_progress(&progress, work_total, progress_total, "Writing metadata...");
    let metadata_path = workspace_dir.join("OPENARC_METADATA.json");
    let metadata_json = serde_json::to_string_pretty(&metadata)?;
    fs::write(&metadata_path, &metadata_json)?;

    emit_progress(
        &progress,
        work_total + 1,
        progress_total,
        "Bundling misc files...",
    );
    let misc_arc_path = workspace_dir.join("misc.arc");
    create_lzma2_bundle(
        &processed
            .iter()
            .filter(|p| p.class == FileClass::Misc)
            .collect::<Vec<_>>(),
        &misc_arc_path,
        settings.misc_compression_level,
    )?;

    emit_progress(
        &progress,
        work_total + 2,
        progress_total,
        "Bundling RAW files...",
    );
    let raw_arc_path = workspace_dir.join("raw.arc");
    create_lzma2_bundle(
        &processed
            .iter()
            .filter(|p| p.class == FileClass::Raw)
            .collect::<Vec<_>>(),
        &raw_arc_path,
        9,
    )?;

    emit_progress(
        &progress,
        work_total + 3,
        progress_total,
        "Writing manifest...",
    );
    let manifest_path = workspace_dir.join("MANIFEST.txt");
    write_manifest(&processed, &skipped_by_catalog, &manifest_path)?;

    let hashes_path = workspace_dir.join("HASHES.sha256");
    write_hashes(
        &processed,
        &hashes_path,
        &misc_arc_path,
        &raw_arc_path,
        &manifest_path,
    )?;

    if misc_dir.exists() {
        fs::remove_dir_all(&misc_dir).with_context(|| {
            format!(
                "Failed to remove staged misc directory {}",
                misc_dir.display()
            )
        })?;
    }
    if raw_dir.exists() {
        fs::remove_dir_all(&raw_dir).with_context(|| {
            format!(
                "Failed to remove staged raw directory {}",
                raw_dir.display()
            )
        })?;
    }

    if settings.output_folder_without_archive {
        emit_progress(
            &progress,
            work_total + 4,
            progress_total,
            "Finalizing output folder...",
        );
    } else {
        emit_progress(
            &progress,
            work_total + 4,
            progress_total,
            "Compressing final archive...",
        );
        let output_parent = output_archive.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(output_parent)?;
        let temp_output = tempfile::Builder::new()
            .prefix("openarc-")
            .suffix(".tmp")
            .tempfile_in(output_parent)
            .with_context(|| {
                format!(
                    "Failed to create temp archive in {}",
                    output_parent.display()
                )
            })?;
        let temp_output_path = temp_output.path().to_path_buf();
        drop(temp_output);

        arcmax::tar_zst::archive_dir_tar_zst(
            &workspace_dir,
            &temp_output_path,
            settings.compression_level,
        )
        .with_context(|| format!("Failed to create archive at {}", output_archive.display()))?;

        if output_archive.exists() {
            fs::remove_file(output_archive).with_context(|| {
                format!(
                    "Failed to replace existing archive {}",
                    output_archive.display()
                )
            })?;
        }
        fs::rename(&temp_output_path, output_archive)
            .with_context(|| format!("Failed to finalize archive {}", output_archive.display()))?;
    }

    emit_progress(
        &progress,
        work_total + 5,
        progress_total,
        "Updating catalog/tracking...",
    );
    // Record archive information in the database
    if let Some(ref mut cat) = catalog {
        record_catalog_entries(cat, &processed, output_archive)?;

        // Also record archive tracking information
        let archive_metadata = std::fs::metadata(output_archive).with_context(|| {
            format!(
                "Failed to get metadata for output: {}",
                output_archive.display()
            )
        })?;

        let archive_record = ArchiveRecord {
            id: None,
            archive_path: output_archive.to_string_lossy().to_string(),
            archive_size: if archive_metadata.is_file() {
                archive_metadata.len()
            } else {
                0
            },
            creation_date: 0, // Will be set by the database
            original_location: output_archive
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string()),
            destination_location: None, // Will be set later when moved
            description: Some(format!("Archive with {} files", processed.len())),
            file_count: processed.len() as u32,
        };

        // Create archive tracker using the same connection as the backup catalog
        if let Ok(mut tracker) = ArchiveTracker::new(cat.get_connection_mut()) {
            if let Ok(archive_id) = tracker.record_archive(archive_record) {
                // Record the files in this archive
                let file_mappings: Vec<ArchiveFileMapping> = processed
                    .iter()
                    .map(|p| {
                        ArchiveFileMapping {
                            id: None,
                            archive_id,
                            file_path: p.archived_rel_path.clone(),
                            original_path: p.original_path.to_string_lossy().to_string(),
                            file_size: p.original_size,
                            archived_at: 0, // Will be set by the database
                        }
                    })
                    .collect();

                if let Err(e) = tracker.record_archive_files(archive_id, file_mappings) {
                    eprintln!("Warning: Failed to record archive files: {}", e);
                }
            } else {
                eprintln!("Warning: Failed to record archive in tracker");
            }
        } else {
            eprintln!("Warning: Could not create archive tracker");
        }
    }

    let dedup_groups = if settings.enable_dedup {
        let mut duplicate_canons = std::collections::HashSet::new();
        for canon in duplicates_of.values() {
            duplicate_canons.insert(canon);
        }
        duplicate_canons.len()
    } else {
        0
    };

    // Phase 3 (tracking): Batch record all processed files, generate & save log
    let tracking_report = if let Some(ref tracker) = tracker {
        let now = crate::file_tracker::iso8601_now();
        let archive_name = output_archive
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        let archive_hash = hash::sha256_file_hex(output_archive).ok();

        let records: Vec<ProcessedFileRecord> = processed
            .iter()
            .map(|p| {
                let file_hash = file_hashes
                    .get(&p.original_path)
                    .cloned()
                    .unwrap_or_default();
                ProcessedFileRecord {
                    file_name: p
                        .original_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    file_hash,
                    file_size: p.original_size as i64,
                    processed_at: now.clone(),
                    run_id: tracker.run_id().to_string(),
                    archive_name: archive_name.clone(),
                    archive_hash: archive_hash.clone(),
                    output_path: p.output_path.to_string_lossy().to_string(),
                    processing_mode: "archive".to_string(),
                }
            })
            .collect();

        if let Err(e) = tracker.record_batch(&records) {
            eprintln!("Warning: Failed to record tracking data: {}", e);
        }

        let log_content =
            tracker.generate_run_log(&tracking_duplicates, processed.len(), "archive");
        if let Err(e) = tracker.write_run_log(&log_content) {
            eprintln!("Warning: Failed to write run log: {}", e);
        }

        if !tracking_duplicates.is_empty() {
            FileTracker::print_duplicate_report(&tracking_duplicates);
        }

        Some(log_content)
    } else {
        None
    };

    emit_progress(&progress, progress_total, progress_total, "Complete");

    Ok(OrchestratorResult {
        discovered_files: discovered,
        processed,
        failed,
        skipped_by_catalog,
        dedup_groups,
        tracking_report,
        staged_uncompressed_videos,
        jpeg2000_fallback,
    })
}

/// Pack a list of processed files into a tar archive compressed with LZMA2.
///
/// Uses a 128 MiB dictionary for maximum solid-compression quality (7-Zip "Ultra"
/// equivalent). The `level` parameter is clamped to the 1–9 range that `lzma-rust2`
/// accepts.
fn create_lzma2_bundle(files: &[&ProcessedFile], output_arc: &Path, level: i32) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }

    // Stream the tar through an in-memory pipe straight into the LZMA2
    // compressor. The previous implementation spooled the full tar to a temp
    // file and read it back — an extra full write + read of every bundled byte
    // (multi-GB for RAW-heavy runs).
    let (pipe_reader, pipe_writer) = std::io::pipe().context("Failed to create bundle pipe")?;

    let tar_inputs: Vec<(PathBuf, String)> = files
        .iter()
        .map(|item| {
            let bundle_rel = item
                .archived_rel_path
                .split_once('/')
                .map(|(_, rest)| rest.to_string())
                .unwrap_or_else(|| item.archived_rel_path.clone());
            (item.output_path.clone(), bundle_rel)
        })
        .collect();

    let tar_thread = thread::spawn(move || -> Result<()> {
        let mut ar = tar::Builder::new(std::io::BufWriter::new(pipe_writer));
        for (path, bundle_rel) in &tar_inputs {
            ar.append_path_with_name(path, Path::new(bundle_rel))
                .with_context(|| format!("Failed to append {} to tar", bundle_rel))?;
        }
        ar.into_inner()
            .context("Failed to finish tar stream")?
            .flush()
            .context("Failed to flush tar stream")?;
        Ok(())
    });

    let opts = LzmaOptions {
        lzma2: true,
        dict_size: 128 * 1024 * 1024,
        level: Some(level.clamp(1, 9) as u8),
        ..Default::default()
    };
    let mut codec = LzmaCodec::new(opts);
    let f = fs::File::create(output_arc)
        .with_context(|| format!("Failed to create {}", output_arc.display()))?;
    let mut writer = std::io::BufWriter::new(f);
    let mut tar_reader = std::io::BufReader::new(pipe_reader);
    let compress_result = codec
        .compress(&mut tar_reader, &mut writer)
        .with_context(|| format!("LZMA2 compression failed for {}", output_arc.display()));

    // Close the read end before joining so a failed compress can't leave the
    // tar thread blocked on a full pipe.
    drop(tar_reader);
    let tar_result = tar_thread
        .join()
        .map_err(|_| anyhow!("Bundle tar thread panicked"))?;
    // Report the tar-side failure first: a compressor error is usually just the
    // broken pipe that follows it.
    tar_result?;
    compress_result?;
    writer.flush().context("Failed to flush bundle output")?;

    Ok(())
}

fn write_manifest(
    processed: &[ProcessedFile],
    skipped: &[PathBuf],
    manifest_path: &Path,
) -> Result<()> {
    let mut f = std::fs::File::create(manifest_path)?;

    writeln!(f, "OpenArc Archive Manifest")?;
    writeln!(f, "========================")?;
    writeln!(f)?;

    writeln!(f, "Processed files: {}", processed.len())?;
    writeln!(f, "Skipped by catalog: {}", skipped.len())?;
    writeln!(f)?;

    for p in processed {
        let format_info = p
            .original_format
            .map(|f| format!(" [orig: {:?}]", f))
            .unwrap_or_default();
        writeln!(
            f,
            "{} -> {} ({} -> {}){}{}",
            p.original_path.display(),
            p.archived_rel_path,
            p.original_size,
            p.output_size,
            if p.skipped_processing {
                " [skipped_processing]"
            } else {
                ""
            },
            format_info
        )?;
    }

    Ok(())
}

fn write_hashes(
    processed: &[ProcessedFile],
    hashes_path: &Path,
    misc_arc_path: &Path,
    raw_arc_path: &Path,
    manifest_path: &Path,
) -> Result<()> {
    let mut hashes: Vec<(String, String)> = Vec::new();

    for p in processed {
        if let Some(ref h) = p.sha256 {
            hashes.push((h.clone(), p.archived_rel_path.clone()));
        }
    }

    if misc_arc_path.exists() {
        let h = hash::sha256_file_hex(misc_arc_path)?;
        hashes.push((h, "misc.arc".to_string()));
    }

    if raw_arc_path.exists() {
        let h = hash::sha256_file_hex(raw_arc_path)?;
        hashes.push((h, "raw.arc".to_string()));
    }

    if manifest_path.exists() {
        let h = hash::sha256_file_hex(manifest_path)?;
        hashes.push((h, "MANIFEST.txt".to_string()));
    }

    hash::write_hashes_file(&hashes, hashes_path)?;
    Ok(())
}

fn record_catalog_entries(
    catalog: &mut BackupCatalog,
    processed: &[ProcessedFile],
    output_archive: &Path,
) -> Result<()> {
    let mut entries = Vec::new();
    let archive_id = output_archive
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());

    for p in processed {
        let md = fs::metadata(&p.original_path)?;
        let mtime_secs = md
            .modified()?
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        entries.push(BackupEntry {
            path: normalize_path(&p.original_path),
            size: md.len(),
            mtime_secs,
            sha256: p.sha256.clone(),
            backed_up_at: 0,
            archive_id: archive_id.clone(),
        });
    }

    catalog.record_backups(entries)
}

/// Update the destination location of an archive in the tracking database
pub fn update_archive_destination(
    catalog_db_path: &Path,
    archive_path: &str,
    destination_path: &str,
) -> Result<()> {
    let mut catalog = BackupCatalog::new(catalog_db_path)?;
    if let Ok(mut tracker) = ArchiveTracker::new(catalog.get_connection_mut()) {
        tracker.update_archive_destination(archive_path, destination_path)?;
    }
    Ok(())
}

/// Result of archive extraction
#[derive(Debug)]
pub struct ExtractionResult {
    pub files_extracted: usize,
    pub total_size: u64,
    pub decoded_files: usize,
}

/// Settings for extraction with decoding
#[derive(Clone, Debug)]
pub struct ExtractionSettings {
    /// Decode BPG files back to original formats
    pub decode_images: bool,
    /// Quality for HEIC re-encoding (1-100)
    pub heic_quality: u8,
    /// Quality for JPEG output (1-100)
    pub jpeg_quality: u8,
}

impl Default for ExtractionSettings {
    fn default() -> Self {
        Self {
            decode_images: true,
            heic_quality: 90,
            jpeg_quality: 92,
        }
    }
}

/// Extract a .tar.zst archive to a directory with progress reporting.
pub fn extract_archive(
    archive_path: &Path,
    output_dir: &Path,
    progress: Option<Arc<ProgressFn>>,
) -> Result<ExtractionResult> {
    let settings = ExtractionSettings::default();
    extract_archive_with_decoding(archive_path, output_dir, settings, progress)
}

/// Extract archive and decode images back to original formats
pub fn extract_archive_with_decoding(
    archive_path: &Path,
    output_dir: &Path,
    settings: ExtractionSettings,
    progress: Option<Arc<ProgressFn>>,
) -> Result<ExtractionResult> {
    if !archive_path.exists() {
        return Err(anyhow!("Archive not found: {}", archive_path.display()));
    }

    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    if let Some(ref cb) = progress {
        cb(0, 1, "Extracting archive...");
    }

    arcmax::tar_zst::extract_tar_zst(archive_path, output_dir)
        .with_context(|| format!("Failed to extract archive: {}", archive_path.display()))?;

    // HASHES.sha256 lists both top-level files (media/*, misc.arc, raw.arc,
    // MANIFEST.txt) and the per-file contents of the LZMA2 bundles (misc/*,
    // raw/*, videos/*). Verify in two phases: what exists now, and the bundle
    // contents after they are unpacked — verifying everything up front always
    // failed for archives with misc/RAW files because those entries only exist
    // inside the bundles at this point.
    let hashes_file = output_dir.join("HASHES.sha256");
    let deferred_hash_entries: Vec<(String, String)> = if hashes_file.exists() {
        let entries = hash::read_hashes_file(&hashes_file)?;
        let (present, deferred): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|(_, rel)| output_dir.join(rel).is_file());
        hash::verify_hash_entries(output_dir, &present).with_context(|| {
            format!(
                "Archive checksum verification failed for {}",
                archive_path.display()
            )
        })?;
        deferred
    } else {
        Vec::new()
    };

    let mut decoded_count = 0usize;

    extract_lzma2_bundle(&output_dir.join("misc.arc"), &output_dir.join("misc"))?;
    extract_lzma2_bundle(&output_dir.join("raw.arc"), &output_dir.join("raw"))?;
    extract_lzma2_bundle(&output_dir.join("videos.arc"), &output_dir.join("videos"))?;

    if !deferred_hash_entries.is_empty() {
        hash::verify_hash_entries(output_dir, &deferred_hash_entries).with_context(|| {
            format!(
                "Bundle checksum verification failed for {}",
                archive_path.display()
            )
        })?;
    }

    // Load metadata if available
    let metadata_path = output_dir.join("OPENARC_METADATA.json");
    let metadata: Option<ArchiveMetadata> = if metadata_path.exists() {
        let content = fs::read_to_string(&metadata_path)?;
        serde_json::from_str(&content).ok()
    } else {
        None
    };

    // Decode images if settings allow and metadata exists.
    // Decodes are independent per file and CPU-bound, so fan them out across a
    // dedicated rayon pool with the large stacks the codecs need.
    if settings.decode_images {
        if let Some(meta) = metadata {
            use rayon::prelude::*;
            let total_images = meta.images.len();
            let done = std::sync::atomic::AtomicUsize::new(0);
            let decoded = std::sync::atomic::AtomicUsize::new(0);

            let pool = rayon::ThreadPoolBuilder::new()
                .stack_size(CODEC_THREAD_STACK_SIZE)
                .build()
                .context("Failed to create decode thread pool")?;

            pool.install(|| {
                meta.images.par_iter().for_each(|img_meta| {
                    let idx = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if let Some(ref cb) = progress {
                        cb(idx, total_images, &img_meta.bpg_filename);
                    }

                    let bpg_path = output_dir.join("media").join(&img_meta.bpg_filename);
                    if !bpg_path.exists() {
                        return;
                    }

                    let result = decode_bpg_to_original(
                        &bpg_path,
                        img_meta.original_format,
                        &img_meta.original_filename,
                        &settings,
                    );

                    match result {
                        Ok(output_path) => {
                            // Remove the BPG file after successful decode
                            let _ = fs::remove_file(&bpg_path);
                            decoded.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                            // Rename to original filename if different
                            let ext = output_path
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or_else(|| img_meta.original_format.extraction_extension());
                            let target_name = format!(
                                "{}.{}",
                                Path::new(&img_meta.original_filename)
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("image"),
                                ext
                            );
                            let target_path = output_path.parent().unwrap().join(&target_name);
                            if output_path != target_path {
                                let _ = fs::rename(&output_path, &target_path);
                            }
                        }
                        Err(e) => {
                            warn!("decode_failed file={} error={}", img_meta.bpg_filename, e);
                        }
                    }
                });
            });
            decoded_count = decoded.load(std::sync::atomic::Ordering::Relaxed);
        }

        // Clean up metadata file
        let _ = fs::remove_file(&metadata_path);
    }

    // Calculate final stats
    let mut files_extracted = 0usize;
    let mut total_size = 0u64;

    for entry in walkdir::WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        files_extracted += 1;
        if let Ok(meta) = entry.metadata() {
            total_size += meta.len();
        }
    }

    if let Some(ref cb) = progress {
        cb(1, 1, "Extraction complete");
    }

    Ok(ExtractionResult {
        files_extracted,
        total_size,
        decoded_files: decoded_count,
    })
}

fn extract_lzma2_bundle(archive_path: &Path, output_dir: &Path) -> Result<()> {
    if !archive_path.exists() {
        return Ok(());
    }

    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    // Stream decompression straight into the tar unpacker through an in-memory
    // pipe — no multi-GB temp .tar spooled to disk and read back.
    let (pipe_reader, pipe_writer) = std::io::pipe().context("Failed to create extract pipe")?;

    let archive_path_owned = archive_path.to_path_buf();
    let decompress_thread = thread::spawn(move || -> Result<()> {
        let f = fs::File::open(&archive_path_owned)
            .with_context(|| format!("Failed to open {}", archive_path_owned.display()))?;
        let mut reader = std::io::BufReader::new(f);
        let opts = LzmaOptions {
            lzma2: true,
            dict_size: 128 * 1024 * 1024,
            ..Default::default()
        };
        let mut codec = LzmaCodec::new(opts);
        let mut writer = std::io::BufWriter::new(pipe_writer);
        codec.decompress(&mut reader, &mut writer).with_context(|| {
            format!(
                "LZMA2 decompression failed for {}",
                archive_path_owned.display()
            )
        })?;
        writer.flush().context("Failed to flush decompressed tar")?;
        Ok(())
    });

    let tar_reader = std::io::BufReader::new(pipe_reader);
    let mut archive = tar::Archive::new(tar_reader);
    let unpack_result = archive
        .unpack(output_dir)
        .with_context(|| format!("Failed to unpack {}", archive_path.display()));

    // Close the read end before joining so a failed unpack can't leave the
    // decompressor blocked on a full pipe.
    drop(archive);
    let decompress_result = decompress_thread
        .join()
        .map_err(|_| anyhow!("Bundle decompress thread panicked"))?;
    decompress_result?;
    unpack_result?;

    let _ = fs::remove_file(archive_path);
    Ok(())
}

/// Append a `videos.arc` bundle to an existing OpenArc archive.
///
/// `encoded_video_root` should contain the externally re-encoded video files.
pub fn append_external_video_bundle(
    archive_path: &Path,
    encoded_video_root: &Path,
    misc_compression_level: i32,
    compression_level: i32,
) -> Result<usize> {
    if !encoded_video_root.exists() {
        return Err(anyhow!(
            "Encoded video path not found: {}",
            encoded_video_root.display()
        ));
    }

    let mut video_files: Vec<PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(encoded_video_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let p = entry.path().to_path_buf();
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(
            ext.as_str(),
            "mp4" | "mov" | "m4v" | "avi" | "mkv" | "wmv" | "webm" | "mts" | "m2ts"
        ) {
            video_files.push(p);
        }
    }

    if video_files.is_empty() {
        return Ok(0);
    }

    video_files.sort();

    let work_dir = tempfile::Builder::new()
        .prefix("openarc-append-videos-")
        .tempdir()
        .context("Failed to create temp work directory")?;
    let extracted_dir = work_dir.path().join("base");
    fs::create_dir_all(&extracted_dir)?;
    arcmax::tar_zst::extract_tar_zst(archive_path, &extracted_dir)
        .with_context(|| format!("Failed to extract {}", archive_path.display()))?;

    let mut synthetic: Vec<ProcessedFile> = Vec::with_capacity(video_files.len());
    for p in &video_files {
        let rel = p
            .strip_prefix(encoded_video_root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        let rel = normalize_archive_rel_path(&rel);
        let size = fs::metadata(p)?.len();
        synthetic.push(ProcessedFile {
            original_path: p.clone(),
            class: FileClass::Misc,
            archived_rel_path: format!("videos/{}", rel),
            output_path: p.clone(),
            original_size: size,
            output_size: size,
            sha256: None,
            skipped_processing: true,
            original_format: None,
        });
    }

    let videos_arc = extracted_dir.join("videos.arc");
    let refs: Vec<&ProcessedFile> = synthetic.iter().collect();
    create_lzma2_bundle(&refs, &videos_arc, misc_compression_level)?;

    let hashes_path = extracted_dir.join("HASHES.sha256");
    if hashes_path.exists() {
        let mut hashes = hash::read_hashes_file(&hashes_path)?;
        let h = hash::sha256_file_hex(&videos_arc)?;
        hashes.retain(|(_, name)| name != "videos.arc");
        hashes.push((h, "videos.arc".to_string()));
        hash::write_hashes_file(&hashes, &hashes_path)?;
    }

    let manifest_path = extracted_dir.join("MANIFEST.txt");
    if manifest_path.exists() {
        let mut f = fs::OpenOptions::new().append(true).open(&manifest_path)?;
        writeln!(f, "")?;
        writeln!(
            f,
            "Externally encoded videos bundled: {}",
            video_files.len()
        )?;
        writeln!(f, "Bundle: videos.arc")?;
    }

    let out_parent = archive_path.parent().unwrap_or_else(|| Path::new("."));
    let tmp_out = tempfile::Builder::new()
        .prefix("openarc-merged-")
        .suffix(".tmp")
        .tempfile_in(out_parent)
        .with_context(|| format!("Failed to create temp archive in {}", out_parent.display()))?;
    let tmp_out_path = tmp_out.path().to_path_buf();
    drop(tmp_out);

    arcmax::tar_zst::archive_dir_tar_zst(&extracted_dir, &tmp_out_path, compression_level)
        .with_context(|| format!("Failed to re-pack {}", archive_path.display()))?;

    if archive_path.exists() {
        fs::remove_file(archive_path)?;
    }
    fs::rename(&tmp_out_path, archive_path)?;

    Ok(video_files.len())
}

/// Decode a BPG file back to its original format
fn decode_bpg_to_original(
    bpg_path: &Path,
    original_format: OriginalImageFormat,
    _original_filename: &str,
    settings: &ExtractionSettings,
) -> Result<PathBuf> {
    let stem = bpg_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let parent = bpg_path.parent().unwrap_or(Path::new("."));
    let is_jpeg2000 = bpg_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "jp2" | "j2k" | "j2c" | "jpc"
            )
        })
        .unwrap_or(false);

    match original_format {
        OriginalImageFormat::Jpeg => {
            // BPG → JPEG directly
            let output_path = parent.join(format!("{}.jpg", stem));
            if is_jpeg2000 {
                decode_jpeg2000_to_jpeg(bpg_path, &output_path, settings.jpeg_quality)?;
            } else {
                decode_bpg_to_jpeg(bpg_path, &output_path, settings.jpeg_quality)?;
            }
            Ok(output_path)
        }
        OriginalImageFormat::Heic => {
            // BPG → PNG (HEIC encoding not yet implemented in pure Rust decoder)
            // Note: Decoding works perfectly, but encoding requires external tools
            let output_path = parent.join(format!("{}.png", stem));
            if is_jpeg2000 {
                decode_jpeg2000_to_png(bpg_path, &output_path)?;
            } else {
                decode_bpg_to_png(bpg_path, &output_path)?;
            }
            Ok(output_path)
        }
        OriginalImageFormat::Raw
        | OriginalImageFormat::Png
        | OriginalImageFormat::Tiff
        | OriginalImageFormat::Bmp
        | OriginalImageFormat::WebP => {
            // BPG → PNG (RAW cannot be recreated, others convert to PNG for compatibility)
            let output_path = parent.join(format!("{}.png", stem));
            if is_jpeg2000 {
                decode_jpeg2000_to_png(bpg_path, &output_path)?;
            } else {
                decode_bpg_to_png(bpg_path, &output_path)?;
            }
            Ok(output_path)
        }
    }
}

fn decode_jpeg2000_to_png(jp2_path: &Path, output_path: &Path) -> Result<()> {
    let img = codecs::jpeg2000::decode_jpeg2000_file(jp2_path)?;
    img.save(output_path)?;
    Ok(())
}

fn decode_jpeg2000_to_jpeg(jp2_path: &Path, output_path: &Path, quality: u8) -> Result<()> {
    let img = codecs::jpeg2000::decode_jpeg2000_file(jp2_path)?;
    let rgb = img.to_rgb8();
    let mut file = fs::File::create(output_path)?;
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, quality);
    rgb.write_with_encoder(encoder)?;
    Ok(())
}

/// Decode BPG to PNG
fn decode_bpg_to_png(bpg_path: &Path, output_path: &Path) -> Result<()> {
    // Try native decoder first
    match codecs::bpg::decode_file(&bpg_path.to_string_lossy()) {
        Ok((data, width, height, _format)) => {
            image::save_buffer(output_path, &data, width, height, image::ColorType::Rgba8)?;
            Ok(())
        }
        Err(_) => {
            // Fall back to JS decoder
            if codecs::bpg_js::is_bpg_js_available() {
                codecs::bpg_js::bpg_js_to_png(bpg_path, output_path)
            } else {
                Err(anyhow!("No BPG decoder available"))
            }
        }
    }
}

/// Decode BPG to JPEG
fn decode_bpg_to_jpeg(bpg_path: &Path, output_path: &Path, quality: u8) -> Result<()> {
    // Try native decoder first
    match codecs::bpg::decode_file(&bpg_path.to_string_lossy()) {
        Ok((data, width, height, _format)) => {
            // Convert RGBA to RGB
            let mut rgb_data: Vec<u8> = Vec::with_capacity((data.len() / 4) * 3);
            for rgba in data.chunks_exact(4) {
                rgb_data.extend_from_slice(&rgba[..3]);
            }

            let img = image::RgbImage::from_raw(width, height, rgb_data)
                .ok_or_else(|| anyhow!("Failed to create image buffer"))?;

            let mut file = fs::File::create(output_path)?;
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, quality);
            img.write_with_encoder(encoder)?;
            Ok(())
        }
        Err(_) => {
            // Fall back to PNG then convert
            let temp_png = output_path.with_extension("temp.png");
            if codecs::bpg_js::is_bpg_js_available() {
                codecs::bpg_js::bpg_js_to_png(bpg_path, &temp_png)?;
                let img = image::open(&temp_png)?;
                let rgb = img.to_rgb8();
                let mut file = fs::File::create(output_path)?;
                let encoder =
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, quality);
                rgb.write_with_encoder(encoder)?;
                let _ = fs::remove_file(&temp_png);
                Ok(())
            } else {
                Err(anyhow!("No BPG decoder available"))
            }
        }
    }
}
