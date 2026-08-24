use anyhow::{anyhow, Context, Result};
use arcmax::codec::traits::Codec;
use arcmax::codec::{LzmaCodec, LzmaOptions};
use codecs::jxl::{estimate_encode_peak, safe_encode_concurrency};
use codecs::video_analyzer::analyze_video_compression;
use image;
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Condvar, Mutex as StdMutex};
use std::thread;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

const CODEC_THREAD_STACK_SIZE: usize = 16 * 1024 * 1024;

// A representative 24 MP RAW -> JPEG XL `best` encode consumes about nine
// logical CPUs on this pipeline. Admitting one heavy image for every two
// logical CPUs only makes the same global Rayon pool fight itself: it did not
// improve throughput in measurement and multiplied the resident set until the
// kernel killed the process. Ten threads per in-flight image leaves a little
// room for the Tokio/control work while still permitting useful overlap on
// larger machines.
const CPU_THREADS_PER_HEAVY_IMAGE: usize = 10;

// `/usr/bin/time -v` measured the complete 24 MP RAW -> JXL path about 10%
// above the sum of the component models. Reserve 25% extra for allocator
// fragmentation, thread-local arenas and model drift instead of scheduling at
// the exact measured edge.
const IMAGE_RESERVATION_HEADROOM_DIVISOR: u64 = 4;

fn image_reservation_with_headroom(estimated_bytes: u64) -> u64 {
    estimated_bytes
        .saturating_add(estimated_bytes.div_ceil(IMAGE_RESERVATION_HEADROOM_DIVISOR))
}

fn image_ram_budget(total_bytes: u64, available_bytes: u64) -> u64 {
    // This application is commonly run on desktop systems without swap. Keep
    // half of both physical RAM and the memory currently available outside the
    // image pipeline, then rely on the per-image limiter for the exact bound.
    // The floor lets one image make progress on a constrained machine; a
    // request larger than the capacity is clamped to exclusive access.
    available_bytes
        .saturating_div(2)
        .min(total_bytes.saturating_div(2))
        .max(512 * 1024 * 1024)
}

fn cpu_bounded_image_capacity(worker_threads: usize) -> usize {
    worker_threads
        .max(1)
        .div_ceil(CPU_THREADS_PER_HEAVY_IMAGE)
        .max(1)
}

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

/// Estimated peak RAM for one image, used to gate encode concurrency.
///
/// The JPEG XL working set is dominated by the three resident `f32` XYB planes
/// and the rate loop's scratch, both of which scale with **pixels** — not with
/// the source's byte width and not with a chroma format, neither of which
/// exists on this path any more. So the estimate needs dimensions and little
/// else, which is why this is considerably shorter than the BPG version it
/// replaced.
fn estimate_image_reservation_bytes(
    input: &Path,
    original_format: OriginalImageFormat,
    original_size: u64,
    settings: &OrchestratorSettings,
) -> u64 {
    let lossless = settings.jxl_effort.is_lossless();

    if original_format == OriginalImageFormat::Raw {
        // raw-autotune has a header-only RAW dimension probe and a measured
        // full-pipeline peak model. The rendered RGB16 buffer stays live while
        // the encoder builds its float planes, so both are charged.
        let pixels = raw_autotune::memory::raw_pixels(input)
            .unwrap_or(original_size)
            .max(1);
        let (width, height) = raw_autotune::memory::raw_dimensions(input).unwrap_or_else(|| {
            let side = (pixels as f64).sqrt().ceil().max(1.0) as u32;
            (
                side,
                pixels.div_ceil(u64::from(side)).min(u64::from(u32::MAX)) as u32,
            )
        });
        return raw_autotune::memory::peak_bytes(pixels)
            .saturating_add(estimate_encode_peak(width, height, lossless))
            .saturating_add(pixels.saturating_mul(6))
            .max(512 * 1024 * 1024);
    }

    // The `image` crate cannot read HEIC, so probing it there always fails and
    // forces the worst-case fallback below. The bpg-decode HEIF parser reads
    // real dimensions from the container headers without decoding, so consult
    // it up front for an accurate reservation.
    let heic_info = if original_format == OriginalImageFormat::Heic {
        codecs::heic::HeicCodec::read_info(input).ok()
    } else {
        None
    };

    let dims = heic_info
        .map(|i| (i.width, i.height))
        .filter(|&(w, h)| w > 0 && h > 0)
        .or_else(|| image::image_dimensions(input).ok());

    let estimate_from_dims = |w: u32, h: u32| {
        let px = u64::from(w) * u64::from(h);
        // Decode-side scratch: the interleaved source buffer, at up to four
        // 16-bit channels, plus the decoder's own temporaries.
        let decode_bytes = px.saturating_mul(8);
        estimate_encode_peak(w, h, lossless)
            .saturating_add(decode_bytes.saturating_mul(2))
            .saturating_add(original_size.min(256 * 1024 * 1024))
            .max(128 * 1024 * 1024)
    };

    if let Some((w, h)) = dims {
        return estimate_from_dims(w, h);
    }

    // Fallback when dimensions cannot be probed from headers.
    let fallback = estimate_encode_peak(6000, 4000, lossless)
        .saturating_add(original_size.saturating_mul(4))
        .max(256 * 1024 * 1024);

    if original_format == OriginalImageFormat::Heic {
        fallback.max(1024 * 1024 * 1024)
    } else {
        fallback
    }
}

use crate::archive_tracker::{ArchiveFileMapping, ArchiveRecord, ArchiveTracker};
use crate::backup_catalog::{normalize_path, BackupCatalog, BackupEntry};
use crate::image_source;
use crate::jxl_wrapper::{JxlConfig, JxlEffort};
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

/// The source format an archived image was encoded from.
///
/// Recorded so extraction can reverse the conversion: a JPEG goes back out as a
/// JPEG, and everything else as a PNG, which is lossless and universally
/// readable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OriginalImageFormat {
    /// JPEG - re-encoded to JPEG on extraction.
    Jpeg,
    /// PNG - written back as PNG.
    Png,
    /// HEIC/HEIF (Samsung, Android, Apple) - written back as PNG.
    Heic,
    /// Camera RAW formats - written back as PNG (RAW cannot be recreated).
    Raw,
    /// TIFF - written back as PNG.
    Tiff,
    /// BMP - written back as PNG.
    Bmp,
    /// WebP - written back as PNG.
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

    /// Whether extraction writes this format back as PNG rather than as itself.
    pub fn extracts_as_png(&self) -> bool {
        !matches!(self, Self::Jpeg)
    }
}

/// Metadata for a compressed image file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub original_filename: String,
    pub original_format: OriginalImageFormat,
    pub original_extension: String,
    /// Archive-relative name of the encoded image, normally `<stem>.jxl`.
    ///
    /// The serde alias keeps archives written before the JPEG XL switch
    /// readable: their metadata spells this field `bpg_filename`, and those
    /// entries are `.bpg` files that extraction still decodes.
    #[serde(alias = "bpg_filename")]
    pub encoded_filename: String,
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
        "jxl" | "bpg" | "jpg" | "jpeg" | "png" | "bmp" | "tif" | "tiff" | "webp" | "heic"
        | "heif"
        | "ico" | "jp2" | "j2k" | "j2c" | "jpc" | "jpt" | "jph" | "jhc" | "dng" | "cr2" | "nef"
        | "arw" | "orf" | "rw2" | "raf" => 1,
        "mp4" | "mov" | "m4v" | "avi" | "mkv" | "wmv" | "webm" | "3gp" | "flv" | "mts" | "m2ts" => {
            2
        }
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
    /// JPEG XL preset: `best` (default), `fast` or `lossless`.
    pub jxl_effort: JxlEffort,
    /// Overrides the preset's bits-per-pixel target when set.
    ///
    /// There is no bit-depth or chroma-format setting to go with this: depth
    /// comes from the source image itself, and JPEG XL has no chroma
    /// subsampling to configure.
    pub jxl_bits_per_pixel: Option<f64>,
    /// ZSTD level (1-22) for the final archive container. The container wraps
    /// already-compressed JPEG XL images, x265/AV1/x266 video, and LZMA2
    /// bundles, so a low level (1-6) is recommended; high levels burn CPU for
    /// negligible gain.
    pub compression_level: i32,
    /// LZMA2 level (1-9) for `misc.arc`, the bundle of small/likely-uncompressible
    /// misc files (documents, configs, etc.).
    pub misc_compression_level: i32,
    pub enable_catalog: bool,
    /// Optional custom catalog database path. If unset, uses OpenArc's durable,
    /// per-user tracking database (AppData on Windows).
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

impl OrchestratorSettings {
    /// The encoder configuration these settings describe.
    pub fn jxl_config(&self) -> JxlConfig {
        JxlConfig {
            effort: self.jxl_effort,
            bits_per_pixel: self.jxl_bits_per_pixel,
            container: false,
        }
    }
}

impl Default for OrchestratorSettings {
    fn default() -> Self {
        Self {
            jxl_effort: JxlEffort::default(),
            jxl_bits_per_pixel: None,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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

#[derive(Debug, Clone)]
pub struct ProcessedFile {
    pub original_path: PathBuf,
    /// Stable path relative to the selected input root.
    pub source_rel_path: String,
    pub class: FileClass,
    pub archived_rel_path: String,
    pub output_path: PathBuf,
    pub original_size: u64,
    pub output_size: u64,
    pub sha256: Option<String>,
    /// Hash of the original bytes, captured before any encoding.
    pub source_sha256: Option<String>,
    pub skipped_processing: bool,
    pub original_format: Option<OriginalImageFormat>,
}

#[derive(Debug, Clone)]
pub struct FailedFile {
    pub original_path: PathBuf,
    pub class: FileClass,
    pub error: String,
}

#[derive(Debug)]
pub struct OrchestratorResult {
    pub discovered_files: Vec<PathBuf>,
    pub processed: Vec<ProcessedFile>,
    pub failed: Vec<FailedFile>,
    pub skipped_by_catalog: Vec<PathBuf>,
    /// Camera JPEG companions omitted because a same-directory, same-stem RAW
    /// file was developed instead.
    pub dropped_paired_jpegs: Vec<PathBuf>,
    pub dedup_groups: usize,
    pub tracking_report: Option<String>,
    pub staged_uncompressed_videos: Vec<PathBuf>,
    /// Exact root containing videos that require external encoding. Present
    /// only when processing staged at least one video.
    pub video_staging_dir: Option<PathBuf>,
}

#[derive(Debug, serde::Serialize)]
struct ArchiveIndex<'a> {
    format_version: u32,
    created_at_unix: u64,
    files: Vec<ArchiveIndexEntry<'a>>,
}

#[derive(Debug, serde::Serialize)]
struct ArchiveIndexEntry<'a> {
    source_relative_path: &'a str,
    archived_path: &'a str,
    class: FileClass,
    source_size: u64,
    archived_size: u64,
    source_sha256: Option<&'a str>,
    archived_sha256: Option<&'a str>,
    media_transcoded: bool,
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

fn extension_lowercase(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn is_camera_raw_extension(extension: &str) -> bool {
    raw_autotune::files::is_supported_raw_extension(extension)
}

fn pair_key(path: &Path) -> Option<(PathBuf, String)> {
    Some((
        path.parent().unwrap_or_else(|| Path::new("")).to_path_buf(),
        path.file_stem()?.to_string_lossy().to_lowercase(),
    ))
}

/// JPEG companions generated by camera JPEG+RAW mode. Pairing is deliberately
/// directory-local and stem-based, so an unrelated JPEG elsewhere is retained.
fn paired_raw_jpegs(discovered: &[PathBuf]) -> std::collections::HashSet<PathBuf> {
    let raw_keys: std::collections::HashSet<(PathBuf, String)> = discovered
        .iter()
        .filter(|path| is_camera_raw_extension(&extension_lowercase(path)))
        .filter_map(|path| pair_key(path))
        .collect();

    discovered
        .iter()
        .filter(|path| matches!(extension_lowercase(path).as_str(), "jpg" | "jpeg"))
        .filter(|path| {
            pair_key(path)
                .as_ref()
                .is_some_and(|key| raw_keys.contains(key))
        })
        .cloned()
        .collect()
}

/// Classify file and determine original format
fn classify_file(path: &Path) -> (FileClass, Option<OriginalImageFormat>) {
    let ext = extension_lowercase(path);

    match ext.as_str() {
        // JPEG - direct encoding to BPG
        "jpg" | "jpeg" => (FileClass::Image, Some(OriginalImageFormat::Jpeg)),

        // PNG - encode via PNG (already PNG, so direct)
        "png" => (FileClass::Image, Some(OriginalImageFormat::Png)),

        // HEIC/HEIF (Samsung, Android, Apple) - encode via PNG intermediate
        "heic" | "heif" | "hif" => (FileClass::Image, Some(OriginalImageFormat::Heic)),

        // Camera RAW formats are developed by raw-autotune and then follow the
        // same BPG/archive path as every other image.
        ext if is_camera_raw_extension(ext) => (FileClass::Image, Some(OriginalImageFormat::Raw)),

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
    create_archive_from_discovered(input_paths, discovered, output_archive, settings, progress)
}

/// Create an archive from an already-confirmed discovery snapshot.
pub fn create_archive_from_discovered(
    input_paths: &[PathBuf],
    discovered: Vec<PathBuf>,
    output_archive: &Path,
    settings: OrchestratorSettings,
    progress: Option<Arc<ProgressFn>>,
) -> Result<OrchestratorResult> {
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
            dropped_paired_jpegs: Vec::new(),
            dedup_groups: 0,
            tracking_report: None,
            staged_uncompressed_videos: Vec::new(),
            video_staging_dir: None,
        });
    }

    let rel_path_map = build_relative_path_map(&discovered, input_paths);

    // Phase 1: Hash all files once — results shared by both tracking and dedup.
    // Hashing is I/O + SHA-256 bound and per-file independent, so fan it out
    // across the CPU; on NVMe this is several times faster than sequential.
    let mut file_hashes: HashMap<PathBuf, String> = HashMap::new();
    // Source hashes are archive-index identities as well as tracking keys, so
    // calculate them once even when optional duplicate reporting is disabled.
    {
        use rayon::prelude::*;
        let hashed_count = std::sync::atomic::AtomicUsize::new(0);
        let total_files = discovered.len();
        file_hashes = discovered
            .par_iter()
            .map(|p| -> Result<(PathBuf, String)> {
                let done = hashed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if done == 0 || done + 1 == total_files || done % 16 == 0 {
                    emit_progress(
                        &progress,
                        done,
                        total_files,
                        format!("Hashing: {}", safe_file_name(p)),
                    );
                }
                Ok((p.clone(), hash::sha256_file_hex(p)?))
            })
            .collect::<Result<HashMap<_, _>>>()?;
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
        .unwrap_or_else(|| crate::file_tracker::openarc_data_dir().join("tracking.db"));
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

    emit_progress(&progress, 0, discovered.len(), "Checking history...");
    // Every .oarc is standalone and creation replaces the destination. Files
    // seen in an earlier job therefore cannot be omitted from this one: doing
    // so produced incomplete (or even empty) replacement archives. Cross-job
    // matches are reported by FileTracker; folder-mode resume is handled by
    // checking the actual destination files.
    let skipped_by_catalog = Vec::new();
    let paired_jpegs = paired_raw_jpegs(&discovered);
    let to_process: Vec<PathBuf> = discovered
        .iter()
        .filter(|path| !paired_jpegs.contains(*path))
        .cloned()
        .collect();

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
        if skipped_set.contains(p) || paired_jpegs.contains(p) {
            continue;
        }
        // Keep duplicate paths independently extractable. The tar container has
        // no content-reference entry, so dropping non-canonical paths corrupts
        // the logical file set. `duplicates_of` remains useful for reporting.
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
    let completed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let may_stage_external_video =
        settings.reencode_media && work.iter().any(|w| w.class == FileClass::Video);
    let video_stage_dir = if settings.output_folder_without_archive {
        workspace_dir.join("videos")
    } else if may_stage_external_video {
        create_external_video_stage_dir(output_archive)?
    } else {
        temp_dir.path().join("unused-video-stage")
    };
    if settings.output_folder_without_archive || may_stage_external_video {
        fs::create_dir_all(&video_stage_dir)?;
    }

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
    let ram_budget = image_ram_budget(total_ram, available_ram);
    let base_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let optimal_threads = get_optimal_thread_count(base_threads);
    // Size scheduler backpressure off a "typical" 24 MPix image. The JPEG XL
    // working set depends on pixels and on whether the encode is lossless, and
    // on nothing else — no bit depth, no chroma format.
    let image_heavy_capacity = safe_encode_concurrency(
        6000,
        4000,
        settings.jxl_effort.is_lossless(),
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
        let cpu_bounded_in_flight = cpu_bounded_image_capacity(optimal_threads);
        let max_in_flight = cpu_bounded_in_flight
            .min(image_heavy_capacity)
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
                let encoded_rel = normalize_archive_rel_path(
                    &source_rel.with_extension(crate::jxl_wrapper::JXL_EXTENSION).to_string_lossy(),
                );

                // Storing the source unchanged, used both when re-encoding is
                // off and when the image carries transparency the encoder
                // cannot yet represent.
                let store_original = |reason: Option<&str>| -> Result<(PathBuf, String)> {
                    if let Some(reason) = reason {
                        warn!("{}: stored unchanged ({reason})", input.display());
                    }
                    let rel = normalize_archive_rel_path(&source_rel.to_string_lossy());
                    let out = stage_at(&media_dir, &rel)?;
                    fs::copy(input, &out)?;
                    Ok((out, format!("media/{}", rel)))
                };

                if !settings_clone.reencode_media {
                    let (out, rel_path) = store_original(None)?;
                    (out, rel_path, true, Some(original_format), FileClass::Image, true)
                } else {
                let out = stage_at(&media_dir, &encoded_rel)?;

                // Resume support: when writing a folder layout (no final
                // archive), a previously interrupted run may already have
                // produced this output directly in the destination. Reuse it
                // instead of re-encoding, so a killed job can be restarted by
                // pointing at the same output folder. (Archive mode stages into
                // a fresh temp dir each run, so there is nothing to resume.)
                if settings_clone.output_folder_without_archive
                    && fs::metadata(&out).map(|m| m.len() > 0).unwrap_or(false)
                {
                    {
                        let mut meta = metadata_mutex.lock();
                        meta.images.push(ImageMetadata {
                            original_filename: file_name.clone(),
                            original_format,
                            original_extension: original_ext.clone(),
                            encoded_filename: encoded_rel.clone(),
                        });
                    }
                    let rel_path = format!("media/{}", encoded_rel);
                    let output_size = fs::metadata(&out)?.len();
                    let sha = hash::sha256_file_hex(&out).ok();
                    {
                        let mut guard = processed_mutex.lock();
                        guard.push(ProcessedFile {
                            original_path: input.clone(),
                            source_rel_path: source_rel_path.clone(),
                            class: FileClass::Image,
                            archived_rel_path: rel_path,
                            output_path: out,
                            original_size,
                            output_size,
                            sha256: sha,
                            source_sha256: item.source_hash.clone(),
                            skipped_processing: true,
                            original_format: Some(original_format),
                        });
                    }
                    return Ok(());
                }

                // Reserve memory budget per image based on dimensions/format.
                let image_reservation = image_reservation_with_headroom(
                    estimate_image_reservation_bytes(
                        input,
                        original_format,
                        original_size,
                        &settings_clone,
                    ),
                );
                let _memory_guard = memory_limiter.acquire(image_reservation);

                // One path for every source format. `image_source::load` picks
                // the right decoder and hands back the widest representation
                // the file actually carries - 16-bit stays 16-bit, a 10-bit
                // HEIC stays 10-bit, greyscale stays greyscale - and the
                // encoder declares that same depth. Nothing here narrows the
                // image, and JPEG XL has no chroma subsampling to apply, so the
                // four divergent BPG branches this replaced (RAW, HEIC, JPEG
                // YCbCr, generic RGB) collapse into one.
                let prepared = match image_source::load(input, original_format) {
                    Ok(prepared) => prepared,
                    Err(err) => {
                        // Undecodable (corrupt or truncated): keep the bytes
                        // rather than lose the file.
                        let (copy_out, rel_path) =
                            store_original(Some(&format!("could not be decoded: {err}")))?;
                        let output_size = fs::metadata(&copy_out)?.len();
                        let sha = item
                            .source_hash
                            .clone()
                            .or_else(|| hash::sha256_file_hex(&copy_out).ok());
                        let mut guard = processed_mutex.lock();
                        guard.push(ProcessedFile {
                            original_path: input.clone(),
                            source_rel_path: source_rel_path.clone(),
                            class: item.class,
                            archived_rel_path: rel_path,
                            output_path: copy_out,
                            original_size,
                            output_size,
                            sha256: sha,
                            source_sha256: item.source_hash.clone(),
                            skipped_processing: true,
                            original_format: Some(original_format),
                        });
                        return Ok(());
                    }
                };

                if prepared.has_transparency {
                    // The JPEG XL encoder has no extra-channel support yet, so
                    // encoding this would silently drop the alpha. Keeping the
                    // original is lossless and honest; revisit when the encoder
                    // grows extra channels.
                    let (copy_out, rel_path) = store_original(Some(
                        "has a transparency channel the JPEG XL encoder cannot yet carry",
                    ))?;
                    let output_size = fs::metadata(&copy_out)?.len();
                    let sha = item
                        .source_hash
                        .clone()
                        .or_else(|| hash::sha256_file_hex(&copy_out).ok());
                    let mut guard = processed_mutex.lock();
                    guard.push(ProcessedFile {
                        original_path: input.clone(),
                        source_rel_path: source_rel_path.clone(),
                        class: item.class,
                        archived_rel_path: rel_path,
                        output_path: copy_out,
                        original_size,
                        output_size,
                        sha256: sha,
                        source_sha256: item.source_hash.clone(),
                        skipped_processing: true,
                        original_format: Some(original_format),
                    });
                    return Ok(());
                }

                let jxl_data = codecs::jxl::encode(
                    &prepared.as_jxl_image(),
                    &settings_clone.jxl_config(),
                )
                .with_context(|| {
                    format!("Failed to encode {} to JPEG XL", input.display())
                })?;

                fs::write(&out, &jxl_data)
                    .with_context(|| format!("Failed to write JPEG XL file: {}", out.display()))?;

                // Explicitly drop the large buffers before the next image.
                drop(jxl_data);
                drop(prepared);

                {
                    let mut meta = metadata_mutex.lock();
                    meta.images.push(ImageMetadata {
                        original_filename: file_name.clone(),
                        original_format,
                        original_extension: original_ext,
                        encoded_filename: encoded_rel.clone(),
                    });
                }

                // Periodic cleanup check - yield to allow other threads to run
                if item.idx % 10 == 0 {
                    std::thread::yield_now();
                }

                let rel_path = format!("media/{}", encoded_rel);
                (out, rel_path, false, Some(original_format), FileClass::Image, false)
                }
            }
            FileClass::Video => {
                let should_store = !settings_clone.reencode_media
                    || safe_analyze_video(input)
                        .map(|a| a.is_efficiently_compressed)
                        .unwrap_or(false);

                if should_store {
                    let source_rel = normalize_archive_rel_path(&source_rel.to_string_lossy());
                    // Compressed video belongs directly under media/. Sending
                    // it through the solid LZMA2 misc bundle wastes minutes for
                    // negligible or negative compression.
                    let out = stage_at(&media_dir, &source_rel)?;
                    fs::copy(input, &out)?;
                    let rel_path = format!("media/{}", source_rel);
                    (out, rel_path, true, None, FileClass::Video, true)
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
                source_rel_path: source_rel_path.clone(),
                class: archived_class,
                archived_rel_path: rel_path,
                output_path: out_path,
                original_size,
                output_size,
                sha256: sha,
                source_sha256: item.source_hash.clone(),
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
    let video_staging_dir = if staged_uncompressed_videos.is_empty() {
        if !settings.output_folder_without_archive && video_stage_dir.exists() {
            let _ = fs::remove_dir_all(&video_stage_dir);
        }
        None
    } else {
        Some(video_stage_dir.clone())
    };

    let metadata = Arc::try_unwrap(metadata_mutex)
        .map_err(|_| anyhow!("Failed to unwrap metadata"))?
        .into_inner();

    // Write metadata JSON
    emit_progress(&progress, work_total, progress_total, "Writing metadata...");
    let metadata_path = workspace_dir.join("OPENARC_METADATA.json");
    let metadata_json = serde_json::to_string_pretty(&metadata)?;
    fs::write(&metadata_path, &metadata_json)?;

    // Machine-readable, content-addressed index. Unlike MANIFEST.txt this keeps
    // source and archived hashes distinct, which is essential after transcoding.
    let archive_index_path = workspace_dir.join("OPENARC_INDEX.json");
    write_archive_index(&processed, &archive_index_path)?;

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
        &archive_index_path,
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
        // Per-payload hashes and the bundle hashes are already embedded in the
        // archive. Avoid a second full read of a potentially multi-terabyte
        // .oarc merely to duplicate that integrity data in local history.
        let archive_hash = None;

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
    let mut dropped_paired_jpegs: Vec<PathBuf> = paired_jpegs.into_iter().collect();
    dropped_paired_jpegs.sort();

    Ok(OrchestratorResult {
        discovered_files: discovered,
        processed,
        failed,
        skipped_by_catalog,
        dropped_paired_jpegs,
        dedup_groups,
        tracking_report,
        staged_uncompressed_videos,
        video_staging_dir,
    })
}

fn create_external_video_stage_dir(output_archive: &Path) -> Result<PathBuf> {
    let parent = output_archive
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir()?);
    fs::create_dir_all(&parent)?;

    let stem = output_archive
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("archive");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    for attempt in 0..100u32 {
        let candidate = parent.join(format!(
            "{stem}.openarc-video-staging-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "Failed to create external-video staging directory {}",
                        candidate.display()
                    )
                });
            }
        }
    }

    Err(anyhow!(
        "Failed to allocate a unique external-video staging directory beside {}",
        output_archive.display()
    ))
}

/// Pack a list of processed files into a tar archive compressed with LZMA2.
///
/// Uses a level-appropriate dictionary rather than allocating 128 MiB even for
/// low/medium levels. The `level` parameter is clamped to the 1–9 range that
/// `lzma-rust2` accepts.
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

    let level = level.clamp(1, 9);
    let dict_size = match level {
        1..=2 => 4 * 1024 * 1024,
        3..=4 => 16 * 1024 * 1024,
        5..=6 => 32 * 1024 * 1024,
        7..=8 => 64 * 1024 * 1024,
        _ => 128 * 1024 * 1024,
    };
    let opts = LzmaOptions {
        lzma2: true,
        dict_size,
        level: Some(level as u8),
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
    archive_index_path: &Path,
) -> Result<()> {
    let mut hashes: Vec<(String, String)> = Vec::new();

    for p in processed {
        // misc/* and raw/* are members of their respective bundle, not
        // top-level paths in the .oarc. Only list directly addressable files.
        if matches!(p.class, FileClass::Misc | FileClass::Raw) {
            continue;
        }
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

    if archive_index_path.exists() {
        let h = hash::sha256_file_hex(archive_index_path)?;
        hashes.push((h, "OPENARC_INDEX.json".to_string()));
    }

    hash::write_hashes_file(&hashes, hashes_path)?;
    Ok(())
}

fn write_archive_index(processed: &[ProcessedFile], path: &Path) -> Result<()> {
    let files = processed
        .iter()
        .map(|p| ArchiveIndexEntry {
            source_relative_path: &p.source_rel_path,
            archived_path: &p.archived_rel_path,
            class: p.class,
            source_size: p.original_size,
            archived_size: p.output_size,
            source_sha256: p.source_sha256.as_deref(),
            archived_sha256: p.sha256.as_deref(),
            media_transcoded: !p.skipped_processing,
        })
        .collect();
    let index = ArchiveIndex {
        format_version: 1,
        created_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        files,
    };
    fs::write(path, serde_json::to_vec(&index)?)?;
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
            sha256: p.source_sha256.clone(),
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

    // Verify top-level payloads before decoding. Older archives may also list
    // individual paths inside solid bundles, so retain the deferred second
    // phase for backward compatibility.
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
                        cb(idx, total_images, &img_meta.encoded_filename);
                    }

                    let encoded_path = output_dir.join("media").join(&img_meta.encoded_filename);
                    if !encoded_path.exists() {
                        return;
                    }

                    let result = decode_encoded_to_original(
                        &encoded_path,
                        img_meta.original_format,
                        &img_meta.original_filename,
                        &settings,
                    );

                    match result {
                        Ok(output_path) => {
                            // Remove the encoded file after a successful decode
                            let _ = fs::remove_file(&encoded_path);
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
                            warn!("decode_failed file={} error={}", img_meta.encoded_filename, e);
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
        codec
            .decompress(&mut reader, &mut writer)
            .with_context(|| {
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

/// Merge externally encoded videos into an existing OpenArc archive.
///
/// Encoded video is already compressed, so files are stored directly under
/// `media/` and only the outer Zstandard container is rebuilt. Older
/// `videos.arc` archives remain extractable, but new archives do not create one.
/// `encoded_video_root` should contain the externally re-encoded video files.
pub fn append_external_video_bundle(
    archive_path: &Path,
    encoded_video_root: &Path,
    expected_video_count: usize,
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
            "mp4" | "mov" | "m4v" | "avi" | "mkv" | "wmv" | "webm" | "3gp" | "flv" | "mts" | "m2ts"
        ) {
            video_files.push(p);
        }
    }

    if video_files.is_empty() {
        return Err(anyhow!(
            "No encoded video files found under {}",
            encoded_video_root.display()
        ));
    }
    if video_files.len() != expected_video_count {
        return Err(anyhow!(
            "Expected {} encoded video files, found {} under {}. Use a clean output folder containing one encoded result per staged video.",
            expected_video_count,
            video_files.len(),
            encoded_video_root.display()
        ));
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
    let legacy_videos_arc = extracted_dir.join("videos.arc");
    if legacy_videos_arc.exists() {
        fs::remove_file(&legacy_videos_arc)?;
    }

    let media_dir = extracted_dir.join("media");
    fs::create_dir_all(&media_dir)?;
    let mut synthetic: Vec<ProcessedFile> = Vec::with_capacity(video_files.len());
    for p in &video_files {
        let rel = p
            .strip_prefix(encoded_video_root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        let rel = normalize_archive_rel_path(&rel);
        let size = fs::metadata(p)?.len();
        let output_path = media_dir.join(Path::new(&rel));
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(p, &output_path).with_context(|| {
            format!(
                "Failed to add externally encoded video {} as media/{}",
                p.display(),
                rel
            )
        })?;
        let sha256 = hash::sha256_file_hex(&output_path)?;
        synthetic.push(ProcessedFile {
            original_path: p.clone(),
            source_rel_path: rel.clone(),
            class: FileClass::Video,
            archived_rel_path: format!("media/{}", rel),
            output_path,
            original_size: size,
            output_size: size,
            sha256: Some(sha256.clone()),
            source_sha256: Some(sha256),
            skipped_processing: false,
            original_format: None,
        });
    }

    let manifest_path = extracted_dir.join("MANIFEST.txt");
    if manifest_path.exists() {
        let mut f = fs::OpenOptions::new().append(true).open(&manifest_path)?;
        writeln!(f, "")?;
        writeln!(f, "Externally encoded videos: {}", video_files.len())?;
        for video in &synthetic {
            writeln!(
                f,
                "{} -> {} ({} -> {}) [externally_encoded]",
                video.original_path.display(),
                video.archived_rel_path,
                video.original_size,
                video.output_size
            )?;
        }
    }

    let archive_index_path = extracted_dir.join("OPENARC_INDEX.json");
    append_external_videos_to_index(&archive_index_path, &synthetic)?;

    let hashes_path = extracted_dir.join("HASHES.sha256");
    if hashes_path.exists() {
        let mut hashes = hash::read_hashes_file(&hashes_path)?;
        let replaced: std::collections::HashSet<&str> = synthetic
            .iter()
            .map(|video| video.archived_rel_path.as_str())
            .collect();
        hashes.retain(|(_, name)| {
            name != "videos.arc"
                && name != "MANIFEST.txt"
                && name != "OPENARC_INDEX.json"
                && !replaced.contains(name.as_str())
        });
        for video in &synthetic {
            if let Some(ref sha256) = video.sha256 {
                hashes.push((sha256.clone(), video.archived_rel_path.clone()));
            }
        }
        if manifest_path.exists() {
            hashes.push((
                hash::sha256_file_hex(&manifest_path)?,
                "MANIFEST.txt".to_string(),
            ));
        }
        if archive_index_path.exists() {
            hashes.push((
                hash::sha256_file_hex(&archive_index_path)?,
                "OPENARC_INDEX.json".to_string(),
            ));
        }
        hash::write_hashes_file(&hashes, &hashes_path)?;
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

fn append_external_videos_to_index(
    archive_index_path: &Path,
    videos: &[ProcessedFile],
) -> Result<()> {
    if !archive_index_path.exists() {
        return Ok(());
    }

    let mut index: serde_json::Value = serde_json::from_slice(&fs::read(archive_index_path)?)
        .context("Failed to parse OPENARC_INDEX.json while adding external videos")?;
    let files = index
        .get_mut("files")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| anyhow!("OPENARC_INDEX.json has no files array"))?;

    let replaced: std::collections::HashSet<&str> = videos
        .iter()
        .map(|video| video.archived_rel_path.as_str())
        .collect();
    files.retain(|entry| {
        entry
            .get("archived_path")
            .and_then(serde_json::Value::as_str)
            .map(|path| !replaced.contains(path))
            .unwrap_or(true)
    });

    for video in videos {
        files.push(serde_json::json!({
            "source_relative_path": video.source_rel_path,
            "archived_path": video.archived_rel_path,
            "class": "Video",
            "source_size": video.original_size,
            "archived_size": video.output_size,
            "source_sha256": video.source_sha256,
            "archived_sha256": video.sha256,
            "media_transcoded": true
        }));
    }

    fs::write(archive_index_path, serde_json::to_vec(&index)?)?;
    Ok(())
}

/// Decode a BPG file back to its original format
/// Decodes one archived image back to its original format.
///
/// Handles both `.jxl` (everything this build writes) and `.bpg`/`.jp2`
/// (archives written before the JPEG XL switch), dispatching on the file
/// extension rather than assuming, so an old archive extracts correctly with a
/// new binary.
fn decode_encoded_to_original(
    encoded_path: &Path,
    original_format: OriginalImageFormat,
    _original_filename: &str,
    settings: &ExtractionSettings,
) -> Result<PathBuf> {
    let stem = encoded_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let parent = encoded_path.parent().unwrap_or(Path::new("."));

    let image = decode_archived_image(encoded_path)?;

    match original_format {
        OriginalImageFormat::Jpeg => {
            let output_path = parent.join(format!("{}.jpg", stem));
            let rgb = image.to_rgb8();
            let mut file = fs::File::create(&output_path)?;
            let encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, settings.jpeg_quality);
            rgb.write_with_encoder(encoder)?;
            Ok(output_path)
        }
        // Everything else goes back out as PNG: lossless, universally readable,
        // and able to carry the 16-bit buffers the JPEG XL path now preserves.
        OriginalImageFormat::Heic
        | OriginalImageFormat::Raw
        | OriginalImageFormat::Png
        | OriginalImageFormat::Tiff
        | OriginalImageFormat::Bmp
        | OriginalImageFormat::WebP => {
            let output_path = parent.join(format!("{}.png", stem));
            image.save(&output_path)?;
            Ok(output_path)
        }
    }
}

/// Decodes an archived image from whichever codec actually produced it.
fn decode_archived_image(path: &Path) -> Result<image::DynamicImage> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "jxl" => codecs::jxl::decode_file(path),
        "bpg" => codecs::bpg_legacy::decode_file(path),
        "jp2" | "j2k" | "j2c" | "jpc" => codecs::jpeg2000::decode_jpeg2000_file(path),
        // No recognised extension: sniff the magic instead of guessing.
        _ => {
            let data = fs::read(path)
                .with_context(|| format!("Failed to read archived image: {}", path.display()))?;
            if codecs::jxl::is_jxl(&data) {
                codecs::jxl::decode(&data)
            } else if codecs::bpg_legacy::is_bpg(&data) {
                codecs::bpg_legacy::decode(&data)
            } else {
                image::load_from_memory(&data).map_err(|e| {
                    anyhow!("unrecognised archived image {}: {e}", path.display())
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavy_image_admission_reflects_measured_cpu_saturation() {
        assert_eq!(cpu_bounded_image_capacity(1), 1);
        assert_eq!(cpu_bounded_image_capacity(8), 1);
        assert_eq!(cpu_bounded_image_capacity(20), 2);
        assert_eq!(cpu_bounded_image_capacity(32), 4);
    }

    #[test]
    fn image_budget_keeps_half_of_total_and_available_ram_outside_the_pipeline() {
        let gib = 1_u64 << 30;
        assert_eq!(image_ram_budget(32 * gib, 24 * gib), 12 * gib);
        assert_eq!(image_ram_budget(32 * gib, 10 * gib), 5 * gib);
        assert_eq!(image_ram_budget(8 * gib, 20 * gib), 4 * gib);
    }

    #[test]
    fn image_reservations_include_allocator_and_model_headroom() {
        assert_eq!(image_reservation_with_headroom(4_000), 5_000);
        assert_eq!(image_reservation_with_headroom(1), 2);
        assert_eq!(image_reservation_with_headroom(u64::MAX), u64::MAX);
    }

    #[test]
    fn camera_raw_files_follow_the_image_pipeline() {
        for extension in [
            "cr2", "CR3", "nef", "ArW", "dng", "orf", "rw2", "raf", "pef", "srw",
        ] {
            assert_eq!(
                classify_file(Path::new(&format!("photo.{extension}"))),
                (FileClass::Image, Some(OriginalImageFormat::Raw))
            );
        }
    }

    #[test]
    fn jpeg_raw_pairs_suppress_only_the_directory_local_jpeg() {
        let discovered = vec![
            PathBuf::from("roll/IMG_0001.JPG"),
            PathBuf::from("roll/img_0001.cr3"),
            PathBuf::from("roll/IMG_0002.jpeg"),
            PathBuf::from("other/IMG_0001.jpg"),
            PathBuf::from("other/IMG_0003.NEF"),
        ];

        let suppressed = paired_raw_jpegs(&discovered);
        assert_eq!(
            suppressed,
            std::collections::HashSet::from([PathBuf::from("roll/IMG_0001.JPG")])
        );
    }

    #[test]
    fn external_videos_are_merged_directly_under_media() {
        let temp = tempfile::tempdir().expect("temp dir");
        let base = temp.path().join("base");
        fs::create_dir_all(base.join("media")).expect("media dir");
        fs::write(
            base.join("MANIFEST.txt"),
            "OpenArc Archive Manifest\n========================\n",
        )
        .expect("manifest");
        fs::write(
            base.join("OPENARC_INDEX.json"),
            br#"{"format_version":1,"created_at_unix":0,"files":[]}"#,
        )
        .expect("index");
        let initial_hashes = vec![
            (
                hash::sha256_file_hex(base.join("MANIFEST.txt")).expect("manifest hash"),
                "MANIFEST.txt".to_string(),
            ),
            (
                hash::sha256_file_hex(base.join("OPENARC_INDEX.json")).expect("index hash"),
                "OPENARC_INDEX.json".to_string(),
            ),
        ];
        hash::write_hashes_file(&initial_hashes, base.join("HASHES.sha256")).expect("hash list");

        let archive = temp.path().join("archive.oarc");
        arcmax::tar_zst::archive_dir_tar_zst(&base, &archive, 1).expect("base archive");

        let encoded = temp.path().join("encoded");
        fs::create_dir_all(encoded.join("nested")).expect("encoded dir");
        fs::write(encoded.join("nested/clip.mp4"), b"encoded-video").expect("encoded video");

        let merged = append_external_video_bundle(&archive, &encoded, 1, 1).expect("merge videos");
        assert_eq!(merged, 1);

        let extracted = temp.path().join("extracted");
        arcmax::tar_zst::extract_tar_zst(&archive, &extracted).expect("extract merged archive");
        assert_eq!(
            fs::read(extracted.join("media/nested/clip.mp4")).expect("merged video"),
            b"encoded-video"
        );
        assert!(!extracted.join("videos.arc").exists());
        hash::verify_dir_against_hashes(&extracted, extracted.join("HASHES.sha256"))
            .expect("updated hashes");

        let index: serde_json::Value =
            serde_json::from_slice(&fs::read(extracted.join("OPENARC_INDEX.json")).expect("index"))
                .expect("valid index");
        assert_eq!(index["files"][0]["archived_path"], "media/nested/clip.mp4");
        assert_eq!(index["files"][0]["class"], "Video");
    }
}
