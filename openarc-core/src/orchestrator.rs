use anyhow::{anyhow, Context, Result};
use arcmax::formats::freearc::writer::{ArchiveOptions, FreeArcWriter};
use codecs::bpg::{BPGEncoderConfig, NativeBPGEncoder};
use codecs::ffmpeg::{FfmpegEncodeOptions, FFmpegEncoder, VideoCodec, VideoSpeedPreset};
use codecs::video_analyzer::analyze_video_compression;
#[cfg(feature = "heif")]
use codecs::heic::{HeicCodec, HeicEncoderConfig, HeifCompressionFormat};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Condvar, Mutex as StdMutex};
use std::thread;
use std::time::Duration;
use bytemuck::cast_vec;
use log::warn;
use tempfile::TempDir;
use zstd_archive::{ZstdCodec, ZstdOptions};
use image;
use std::io::Read;

/// Bounded limiter for heavy tasks (videos/very large images)
struct HeavyLimiter {
    count: StdMutex<usize>,
    cvar: Condvar,
    capacity: usize,
}

/// Analyze video compression with a timeout to avoid hangs
fn safe_analyze_video(path: &Path) -> Option<codecs::video_analyzer::VideoAnalysis> {
    let path = path.to_path_buf();
    let thread_path = path.clone();
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        let _ = tx.send(std::panic::catch_unwind(|| analyze_video_compression(&thread_path)));
    });

    let result = rx.recv_timeout(Duration::from_secs(5)).ok().and_then(|r| match r {
        Ok(Ok(v)) => Some(v),
        Ok(Err(e)) => {
            warn!("Video analysis failed for {}: {}", path.display(), e);
            None
        }
        Err(_) => {
            warn!("Video analysis panicked for {}", path.display());
            None
        }
    });

    if handle.join().is_err() {
        warn!("Video analysis thread panicked for {}", path.display());
    }

    result
}

impl HeavyLimiter {
    fn new(capacity: usize) -> Self {
        Self {
            count: StdMutex::new(capacity),
            cvar: Condvar::new(),
            capacity,
        }
    }

    fn acquire(&self) -> HeavyGuard<'_> {
        let mut guard = self.count.lock().unwrap();
        while *guard == 0 {
            guard = self.cvar.wait(guard).unwrap();
        }
        *guard -= 1;
        HeavyGuard { limiter: self }
    }
}

/// RAII guard that releases a heavy task permit when dropped
struct HeavyGuard<'a> {
    limiter: &'a HeavyLimiter,
}

impl<'a> Drop for HeavyGuard<'a> {
    fn drop(&mut self) {
        let mut guard = self.limiter.count.lock().unwrap();
        *guard = (*guard + 1).min(self.limiter.capacity);
        self.limiter.cvar.notify_one();
    }
}

use crate::archive_tracker::{ArchiveTracker, ArchiveRecord, ArchiveFileMapping};
use crate::backup_catalog::{normalize_path, BackupCatalog, BackupEntry};
use crate::hash;

/// Check current memory usage and return the percentage of memory used
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
        // For 16-bit source images, use 10 or 12 bit depending on user preference
        // Cap at 12 since that's BPG's maximum
        match user_setting {
            10 | 12 => user_setting,
            9..=11 => 10,
            _ => 12, // 12+ maps to 12-bit
        }
    } else {
        // For 8-bit source images, always use 8-bit
        // (no point in encoding 8-bit data at higher bit depth)
        8
    }
}

/// Memory-constrained video encoding with additional safety checks
fn encode_video_with_memory_constraints(
    input: &Path,
    output: &Path,
    opts: FfmpegEncodeOptions,
    _settings: &OrchestratorSettings
) -> Result<()> {
    // Video encoding is memory-intensive, so we need to be extra careful
    let memory_usage = check_memory_usage();

    // If memory usage is very high, we should wait or potentially fail gracefully
    if memory_usage > 0.95 {
        return Err(anyhow!("Insufficient memory to start video encoding ({}% used)", memory_usage * 100.0));
    } else if memory_usage > 0.90 {
        // Wait a bit more before starting video encoding
        std::thread::sleep(std::time::Duration::from_millis(1000));
    } else if memory_usage > 0.85 {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // Video encoding can be CPU intensive too, so we might want to adjust settings based on system load
    let enc = FFmpegEncoder::with_options(opts);
    enc.encode_file(input, output)?;

    // Suggest cleanup after processing
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);

    Ok(())
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
    /// HEIC/HEIF (Samsung, Android, Apple) - decode BPG → PNG → HEIC
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
            Self::Heic => "heic",
            Self::Raw => "png",  // RAW cannot be recreated
            Self::Tiff => "png", // Convert to PNG for compatibility
            Self::Bmp => "png",  // Convert to PNG for compatibility
            Self::WebP => "png", // Convert to PNG for compatibility
        }
    }

    /// Should this format be encoded via PNG intermediate for quality preservation?
    pub fn needs_png_intermediate(&self) -> bool {
        match self {
            Self::Jpeg => false,  // JPEG goes directly to BPG
            _ => true,            // All others go through PNG to preserve quality
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
    p.trim_start_matches('/')
        .replace('\\', "/")
}

fn detect_file_type_from_name(name: &str) -> i32 {
    let lower = name.to_ascii_lowercase();
    let ext = std::path::Path::new(&lower)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match ext {
        "bpg" | "jpg" | "jpeg" | "png" | "bmp" | "tif" | "tiff" | "webp" | "heic" | "heif" | "ico" |
        "jp2" | "j2k" | "j2c" | "jpc" | "jpt" | "jph" | "jhc" |
        "dng" | "cr2" | "nef" | "arw" | "orf" | "rw2" | "raf" => 1,
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
        if !line.contains(" -> ") {
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
    let file = std::fs::File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;
    let decoder = zstd::stream::read::Decoder::new(file)
        .with_context(|| format!("Failed to create zstd decoder for {}", archive_path.display()))?;
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
            entry.read_to_string(&mut buf)
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

pub fn extract_archive_entry(archive_path: &Path, entry_name: &str, output_path: &Path) -> Result<()> {
    let entry_name = normalize_archive_rel_path(entry_name);

    let file = std::fs::File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;
    let decoder = zstd::stream::read::Decoder::new(file)
        .with_context(|| format!("Failed to create zstd decoder for {}", archive_path.display()))?;
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
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create output directory: {}", parent.display()))?;
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
    pub bpg_quality: i32,
    pub bpg_lossless: bool,
    pub bpg_bit_depth: i32,
    pub bpg_chroma_format: i32,
    pub bpg_encoder_type: i32,
    pub bpg_compression_level: i32,
    pub video_preset: i32,
    pub video_crf: i32,
    pub compression_level: i32,
    pub enable_catalog: bool,
    pub enable_dedup: bool,
    pub skip_already_compressed_videos: bool,
    /// Optional staging directory for temp work (defaults to system temp)
    pub staging_dir: Option<PathBuf>,
    /// Quality for HEIC re-encoding during extraction (1-100)
    pub heic_quality: u8,
    /// Quality for JPEG output during extraction (1-100)
    pub jpeg_quality: u8,
}

impl Default for OrchestratorSettings {
    fn default() -> Self {
        Self {
            bpg_quality: 25,
            bpg_lossless: false,
            bpg_bit_depth: 8,
            bpg_chroma_format: 1,
            bpg_encoder_type: 0,
            bpg_compression_level: 8,
            video_preset: 0,
            video_crf: 23,
            compression_level: 22,
            enable_catalog: true,
            enable_dedup: true,
            skip_already_compressed_videos: true,
            staging_dir: None,
            heic_quality: 90,
            jpeg_quality: 92,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileClass {
    Image,
    Video,
    Misc,
}

pub type ProgressFn = dyn Fn(usize, usize, &str) + Send + Sync;

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

#[derive(Debug)]
pub struct OrchestratorResult {
    pub discovered_files: Vec<PathBuf>,
    pub processed: Vec<ProcessedFile>,
    pub skipped_by_catalog: Vec<PathBuf>,
    pub dedup_groups: usize,
}

#[derive(Clone, Debug)]
struct WorkItem {
    idx: usize,
    input: PathBuf,
    class: FileClass,
    original_format: Option<OriginalImageFormat>,
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
    Ok(files)
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
        "cr2" | "cr3" | "nef" | "arw" | "dng" | "orf" | "rw2" | "raf" | "pef" | "srw" =>
            (FileClass::Image, Some(OriginalImageFormat::Raw)),

        // TIFF - encode via PNG intermediate
        "tiff" | "tif" => (FileClass::Image, Some(OriginalImageFormat::Tiff)),

        // BMP - encode via PNG intermediate
        "bmp" => (FileClass::Image, Some(OriginalImageFormat::Bmp)),

        // WebP - encode via PNG intermediate
        "webp" => (FileClass::Image, Some(OriginalImageFormat::WebP)),

        // Video formats
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" | "3gp" | "flv" | "wmv" | "mts" | "m2ts" =>
            (FileClass::Video, None),

        // Everything else
        _ => (FileClass::Misc, None),
    }
}

fn safe_file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string())
}

/// Convert non-JPEG image to PNG for quality preservation before BPG encoding
fn convert_to_png_intermediate(input: &Path, output: &Path, format: OriginalImageFormat) -> Result<()> {
    match format {
        #[cfg(feature = "heif")]
        OriginalImageFormat::Heic => {
            // HEIC → PNG via libheif
            if HeicCodec::is_available() {
                let codec = HeicCodec::new()?;
                codec.decode_to_png(input, output)?;
            } else {
                return Err(anyhow!("HEIC decoding not available - libheif not found"));
            }
        }
        #[cfg(not(feature = "heif"))]
        OriginalImageFormat::Heic => {
            return Err(anyhow!("HEIC support not compiled - enable heif feature"));
        }
        OriginalImageFormat::Raw => {
            // RAW → PNG via libraw (through codecs::raw)
            // Fall back to image crate if libraw not available
            let img = image::open(input)
                .with_context(|| format!("Failed to open RAW image: {}", input.display()))?;
            img.save(output)?;
        }
        OriginalImageFormat::Png | OriginalImageFormat::Tiff |
        OriginalImageFormat::Bmp | OriginalImageFormat::WebP => {
            // These formats can be opened by image crate and saved as PNG
            let img = image::open(input)
                .with_context(|| format!("Failed to open image: {}", input.display()))?;
            img.save(output)?;
        }
        OriginalImageFormat::Jpeg => {
            // JPEG should not go through this function
            unreachable!("JPEG should not use PNG intermediate");
        }
    }
    Ok(())
}

pub fn create_archive(
    input_paths: &[PathBuf],
    output_archive: &Path,
    settings: OrchestratorSettings,
    progress: Option<Arc<ProgressFn>>,
) -> Result<OrchestratorResult> {
    let discovered = collect_files(input_paths)?;
    if discovered.is_empty() {
        return Ok(OrchestratorResult {
            discovered_files: Vec::new(),
            processed: Vec::new(),
            skipped_by_catalog: Vec::new(),
            dedup_groups: 0,
        });
    }

    let catalog_path = output_archive.with_extension("catalog.sqlite");
    let mut catalog = if settings.enable_catalog {
        Some(BackupCatalog::new(&catalog_path)?)
    } else {
        None
    };

    let (skipped_by_catalog, to_process) = if let Some(ref cat) = catalog {
        cat.filter_files_to_backup(discovered.clone())?
    } else {
        (Vec::new(), discovered.clone())
    };

    let total = discovered.len();
    if let Some(ref cb) = progress {
        cb(0, total, "Preparing...");
    }

    let mut dedup_canon: HashMap<String, PathBuf> = HashMap::new();
    let mut duplicates_of: HashMap<PathBuf, PathBuf> = HashMap::new();

    if settings.enable_dedup {
        for p in &to_process {
            let h = hash::sha256_file_hex(p)?;
            if let Some(prev) = dedup_canon.get(&h) {
                duplicates_of.insert(p.clone(), prev.clone());
            } else {
                dedup_canon.insert(h, p.clone());
            }
        }
    }

    let mut work: Vec<WorkItem> = Vec::new();
    for (idx, p) in discovered.iter().enumerate() {
        if skipped_by_catalog.contains(p) {
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
            class,
            original_format,
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
    let media_dir = temp_dir.path().join("media");
    let misc_dir = temp_dir.path().join("misc");
    fs::create_dir_all(&media_dir)?;
    // Only create misc/ if there are actually misc files to archive.
    // An empty misc/ directory can cause issues with tar on Windows.
    let has_misc_files = work.iter().any(|w| w.class == FileClass::Misc);
    if has_misc_files {
        fs::create_dir_all(&misc_dir)?;
    }

    let processed_mutex = Arc::new(parking_lot::Mutex::new(Vec::<ProcessedFile>::new()));
    let metadata_mutex = Arc::new(parking_lot::Mutex::new(ArchiveMetadata::default()));
    let completed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let (tx, rx) = flume::unbounded::<WorkDone>();
    let progress_clone = progress.clone();
    let work_total = work.len();
    let progress_thread = std::thread::spawn(move || {
        if let Some(cb) = progress_clone {
            while let Ok(done) = rx.recv() {
                cb(done.idx + 1, work_total, &done.file_name);
            }
        } else {
            while rx.recv().is_ok() {}
        }
    });

    let settings_clone = settings.clone();
    let heavy_limiter = Arc::new(HeavyLimiter::new(2));
    let optimal_threads = get_optimal_thread_count(5); // Base thread count of 5
    let encoding_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(optimal_threads)
        .build()
        .context("Failed to create encoding thread pool")?;
    encoding_pool.install(|| {
    let heavy_limiter = heavy_limiter.clone();
    work.par_iter().try_for_each(|item| -> Result<()> {
        // Check memory usage before processing each item
        let memory_usage = check_memory_usage();
        if memory_usage > 0.85 { // 85% threshold
            // Brief pause to allow garbage collection
            std::thread::sleep(std::time::Duration::from_millis(100));
        } else if memory_usage > 0.90 { // 90% threshold
            // More significant pause
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        let input = &item.input;
        let file_name = safe_file_name(input);
        let original_size = fs::metadata(input)?.len();

        let (out_path, rel_path, skipped_processing, original_format) = match item.class {
            FileClass::Image => {
                let original_format = item.original_format.unwrap_or(OriginalImageFormat::Png);
                let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
                let original_ext = input.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
                    .to_lowercase();

                let out = media_dir.join(format!("{}_{}.bpg", stem, item.idx));

                // Throttle massive images to avoid OOM alongside videos
                let _heavy_guard = if original_size > 50_000_000 {
                    Some(heavy_limiter.acquire())
                } else {
                    None
                };

                // Load image into memory and convert to raw pixel data
                let img_result = if original_format == OriginalImageFormat::Heic {
                    #[cfg(feature = "heif")]
                    {
                        if HeicCodec::is_available() {
                            let codec = HeicCodec::new()?;
                            let temp_png = media_dir.join(format!("{}_heic_temp.png", stem));
                            codec.decode_to_png(input, &temp_png)?;
                            let img = image::open(&temp_png);
                            let _ = fs::remove_file(&temp_png);
                            img.map_err(|e| anyhow::anyhow!(e))
                        } else {
                            Err(anyhow!("HEIC decoding not available - libheif not found"))
                        }
                    }
                    #[cfg(not(feature = "heif"))]
                    {
                        Err(anyhow!("HEIC support not compiled - enable heif feature"))
                    }
                } else {
                    image::open(input).map_err(|e| anyhow::anyhow!(e))
                };

                // If the image can't be decoded (corrupt/truncated), copy the original
                // file as-is to preserve it in the archive without BPG encoding.
                let img = match img_result {
                    Ok(img) => img,
                    Err(_) => {
                        let copy_name = format!("{}_{}.{}", stem, item.idx, original_ext);
                        let copy_out = media_dir.join(&copy_name);
                        fs::copy(input, &copy_out)
                            .with_context(|| format!("Failed to copy unreadable image: {}", input.display()))?;
                        let rel_path = format!("media/{}", copy_name);
                        return Ok({
                            let output_size = fs::metadata(&copy_out)?.len();
                            let sha = hash::sha256_file_hex(&copy_out).ok();
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
                            let seq = completed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let _ = tx.send(WorkDone { idx: seq, file_name });
                        });
                    }
                };

                // Convert to RGB8 or RGBA8 for BPG encoding
                let target_bit_depth = detect_image_bit_depth(&img, original_format, settings_clone.bpg_bit_depth);
                let wants_high_depth = target_bit_depth > 8;

                let (width, height, pixel_data, format, bytes_per_sample) = if wants_high_depth {
                    match &img {
                        image::DynamicImage::ImageRgb16(rgb) => {
                            let (w, h) = rgb.dimensions();
                            let data = cast_vec(rgb.clone().into_raw());
                            (w, h, data, codecs::bpg::BPGImageFormat::RGB24, 2u32)
                        }
                        image::DynamicImage::ImageRgba16(rgba) => {
                            let (w, h) = rgba.dimensions();
                            let data = cast_vec(rgba.clone().into_raw());
                            (w, h, data, codecs::bpg::BPGImageFormat::RGBA32, 2u32)
                        }
                        _ => {
                            let rgb = img.to_rgb16();
                            let (w, h) = rgb.dimensions();
                            let data = cast_vec(rgb.into_raw());
                            (w, h, data, codecs::bpg::BPGImageFormat::RGB24, 2u32)
                        }
                    }
                } else {
                    match &img {
                        image::DynamicImage::ImageRgb8(rgb) => {
                            let (w, h) = rgb.dimensions();
                            (w, h, rgb.clone().into_raw(), codecs::bpg::BPGImageFormat::RGB24, 1u32)
                        }
                        image::DynamicImage::ImageRgba8(rgba) => {
                            let (w, h) = rgba.dimensions();
                            (w, h, rgba.clone().into_raw(), codecs::bpg::BPGImageFormat::RGBA32, 1u32)
                        }
                        _ => {
                            let rgb = img.to_rgb8();
                            let (w, h) = rgb.dimensions();
                            (w, h, rgb.into_raw(), codecs::bpg::BPGImageFormat::RGB24, 1u32)
                        }
                    }
                };

                // Encode to BPG in-memory
                let mut enc = NativeBPGEncoder::new().context("Failed to create BPG encoder")?;
                let mut cfg: BPGEncoderConfig = NativeBPGEncoder::default_config();
                cfg.quality = settings_clone.bpg_quality;
                cfg.lossless = if settings_clone.bpg_lossless { 1 } else { 0 };

                // Auto-detect optimal bit depth based on source image
                cfg.bit_depth = target_bit_depth;

                cfg.chroma_format = settings_clone.bpg_chroma_format;
                cfg.encoder_type = settings_clone.bpg_encoder_type;
                cfg.compress_level = settings_clone.bpg_compression_level;
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

                // Record metadata for extraction
                {
                    let mut meta = metadata_mutex.lock();
                    meta.images.push(ImageMetadata {
                        original_filename: file_name.clone(),
                        original_format,
                        original_extension: original_ext,
                        bpg_filename: format!("{}_{}.bpg", stem, item.idx),
                    });
                }

                // Explicitly drop large data structures to free memory immediately
                drop(pixel_data);
                drop(bpg_data);

                // Periodic cleanup check - yield to allow other threads to run
                if item.idx % 10 == 0 {  // Every 10th item
                    std::thread::yield_now();
                }

                let rel_path = format!("media/{}", out.file_name().unwrap().to_string_lossy());
                (out, rel_path, false, Some(original_format))
            }
            FileClass::Video => {
                let should_skip = if settings_clone.skip_already_compressed_videos {
                    safe_analyze_video(input)
                        .map(|a| a.is_efficiently_compressed)
                        .unwrap_or(false)
                } else {
                    false
                };

                if should_skip {
                    let out = media_dir.join(input.file_name().unwrap());
                    fs::copy(input, &out)?;
                    let rel_path = format!("media/{}", out.file_name().unwrap().to_string_lossy());
                    (out, rel_path, true, None)
                } else {
                    // Limit concurrent heavy video encodes to prevent memory spikes
                    let _heavy_guard = heavy_limiter.acquire();

                    let (codec, preset) = match settings_clone.video_preset {
                        1 => (VideoCodec::H265, VideoSpeedPreset::Medium),
                        2 => (VideoCodec::H264, VideoSpeedPreset::Fast),
                        3 => (VideoCodec::H265, VideoSpeedPreset::Slow),
                        _ => (VideoCodec::H264, VideoSpeedPreset::Medium),
                    };

                    let out = media_dir.join(format!(
                        "{}.mp4",
                        input.file_stem().and_then(|s| s.to_str()).unwrap_or("video")
                    ));

                    let opts = FfmpegEncodeOptions {
                        codec,
                        speed: preset,
                        crf: Some(settings_clone.video_crf as u8),
                        copy_audio: true,
                    };

                    // Use memory-constrained video encoding
                    encode_video_with_memory_constraints(input, &out, opts, &settings_clone)?;

                    let rel_path = format!("media/{}", out.file_name().unwrap().to_string_lossy());
                    (out, rel_path, false, None)
                }
            }
            FileClass::Misc => {
                let out = misc_dir.join(input.file_name().unwrap());
                fs::copy(input, &out)?;
                let rel_path = format!("misc/{}", out.file_name().unwrap().to_string_lossy());
                (out, rel_path, false, None)
            }
        };

        let output_size = fs::metadata(&out_path)?.len();
        let sha = hash::sha256_file_hex(&out_path).ok();

        {
            let mut guard = processed_mutex.lock();
            guard.push(ProcessedFile {
                original_path: input.clone(),
                class: item.class,
                archived_rel_path: rel_path,
                output_path: out_path,
                original_size,
                output_size,
                sha256: sha,
                skipped_processing,
                original_format,
            });
        }

        let seq = completed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let _ = tx.send(WorkDone { idx: seq, file_name });
        Ok(())
    })
    })?;

    drop(tx);
    let _ = progress_thread.join();

    let processed = Arc::try_unwrap(processed_mutex)
        .map_err(|_| anyhow!("Failed to unwrap processed results"))?
        .into_inner();

    let metadata = Arc::try_unwrap(metadata_mutex)
        .map_err(|_| anyhow!("Failed to unwrap metadata"))?
        .into_inner();

    // Write metadata JSON
    let metadata_path = temp_dir.path().join("OPENARC_METADATA.json");
    let metadata_json = serde_json::to_string_pretty(&metadata)?;
    fs::write(&metadata_path, &metadata_json)?;

    let misc_arc_path = temp_dir.path().join("misc.arc");
    create_misc_arc(&processed, &misc_arc_path, settings.compression_level)?;

    let manifest_path = temp_dir.path().join("MANIFEST.txt");
    write_manifest(&processed, &skipped_by_catalog, &manifest_path)?;

    let hashes_path = temp_dir.path().join("HASHES.sha256");
    write_hashes(&processed, &hashes_path, &misc_arc_path, &manifest_path)?;

    let zstd = make_zstd(3);
    zstd.archive_dir_tar_zst(temp_dir.path(), output_archive)
        .with_context(|| format!("Failed to create zstd archive at {}", output_archive.display()))?;

    // Record archive information in the database
    if let Some(ref mut cat) = catalog {
        record_catalog_entries(cat, &processed, output_archive)?;

        // Also record archive tracking information
        let archive_metadata = std::fs::metadata(output_archive)
            .with_context(|| format!("Failed to get metadata for archive: {}", output_archive.display()))?;

        let archive_record = ArchiveRecord {
            id: None,
            archive_path: output_archive.to_string_lossy().to_string(),
            archive_size: archive_metadata.len(),
            creation_date: 0, // Will be set by the database
            original_location: output_archive.parent()
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
                let file_mappings: Vec<ArchiveFileMapping> = processed.iter().map(|p| {
                    ArchiveFileMapping {
                        id: None,
                        archive_id,
                        file_path: p.archived_rel_path.clone(),
                        original_path: p.original_path.to_string_lossy().to_string(),
                        file_size: p.original_size,
                        archived_at: 0, // Will be set by the database
                    }
                }).collect();

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

    let dedup_groups = if settings.enable_dedup { dedup_canon.len() } else { 0 };

    Ok(OrchestratorResult {
        discovered_files: discovered,
        processed,
        skipped_by_catalog,
        dedup_groups,
    })
}

fn create_misc_arc(processed: &[ProcessedFile], output_arc: &Path, compression_level: i32) -> Result<()> {
    let misc: Vec<&ProcessedFile> = processed.iter().filter(|p| p.class == FileClass::Misc).collect();
    if misc.is_empty() {
        return Ok(());
    }

    let f = std::fs::File::create(output_arc)
        .with_context(|| format!("Failed to create {}", output_arc.display()))?;
    let mut writer = std::io::BufWriter::new(f);

    let mut arc = FreeArcWriter::new(
        &mut writer,
        ArchiveOptions {
            compression: "lzma".to_string(),
            compression_level,
            encryption: None,
            password: None,
        },
    )?;

    let mut name_counts: HashMap<String, usize> = HashMap::new();

    for item in misc {
        let data = std::fs::read(&item.output_path)?;
        let mut name = item
            .output_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();

        let c = name_counts.entry(name.clone()).or_insert(0);
        if *c > 0 {
            name = format!("{}_{}", *c, name);
        }
        *c += 1;

        arc.add_file(&name, &data)?;
    }

    let inner = arc.finish()?;
    inner.flush()?;

    Ok(())
}

fn write_manifest(processed: &[ProcessedFile], skipped: &[PathBuf], manifest_path: &Path) -> Result<()> {
    let mut f = std::fs::File::create(manifest_path)?;

    writeln!(f, "OpenArc Archive Manifest")?;
    writeln!(f, "========================")?;
    writeln!(f)?;

    writeln!(f, "Processed files: {}", processed.len())?;
    writeln!(f, "Skipped by catalog: {}", skipped.len())?;
    writeln!(f)?;

    for p in processed {
        let format_info = p.original_format
            .map(|f| format!(" [orig: {:?}]", f))
            .unwrap_or_default();
        writeln!(
            f,
            "{} -> {} ({} -> {}){}{}",
            p.original_path.display(),
            p.archived_rel_path,
            p.original_size,
            p.output_size,
            if p.skipped_processing { " [skipped_processing]" } else { "" },
            format_info
        )?;
    }

    Ok(())
}

fn write_hashes(processed: &[ProcessedFile], hashes_path: &Path, misc_arc_path: &Path, manifest_path: &Path) -> Result<()> {
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

    if manifest_path.exists() {
        let h = hash::sha256_file_hex(manifest_path)?;
        hashes.push((h, "MANIFEST.txt".to_string()));
    }

    hash::write_hashes_file(&hashes, hashes_path)?;
    Ok(())
}

fn record_catalog_entries(catalog: &mut BackupCatalog, processed: &[ProcessedFile], output_archive: &Path) -> Result<()> {
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

fn make_zstd(level: i32) -> ZstdCodec {
    let mut opts = ZstdOptions::default();
    opts.level = level;
    ZstdCodec::new(opts)
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
    compression_level: i32,
    progress: Option<Arc<ProgressFn>>,
) -> Result<ExtractionResult> {
    let settings = ExtractionSettings::default();
    extract_archive_with_decoding(archive_path, output_dir, compression_level, settings, progress)
}

/// Extract archive and decode images back to original formats
pub fn extract_archive_with_decoding(
    archive_path: &Path,
    output_dir: &Path,
    compression_level: i32,
    settings: ExtractionSettings,
    progress: Option<Arc<ProgressFn>>,
) -> Result<ExtractionResult> {
    if !archive_path.exists() {
        return Err(anyhow!("Archive not found: {}", archive_path.display()));
    }

    fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;

    if let Some(ref cb) = progress {
        cb(0, 1, "Extracting archive...");
    }

    // Extract the archive
    let zstd = make_zstd(compression_level);
    zstd.extract_tar_zst(archive_path, output_dir)
        .with_context(|| format!("Failed to extract archive: {}", archive_path.display()))?;

    let mut decoded_count = 0usize;

    // Load metadata if available
    let metadata_path = output_dir.join("OPENARC_METADATA.json");
    let metadata: Option<ArchiveMetadata> = if metadata_path.exists() {
        let content = fs::read_to_string(&metadata_path)?;
        serde_json::from_str(&content).ok()
    } else {
        None
    };

    // Decode images if settings allow and metadata exists
    if settings.decode_images {
        if let Some(meta) = metadata {
            let total_images = meta.images.len();

            for (idx, img_meta) in meta.images.iter().enumerate() {
                if let Some(ref cb) = progress {
                    cb(idx, total_images, &img_meta.bpg_filename);
                }

                let bpg_path = output_dir.join("media").join(&img_meta.bpg_filename);
                if !bpg_path.exists() {
                    continue;
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
                        decoded_count += 1;

                        // Rename to original filename if different
                        let target_name = format!(
                            "{}.{}",
                            Path::new(&img_meta.original_filename)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("image"),
                            img_meta.original_format.extraction_extension()
                        );
                        let target_path = output_path.parent().unwrap().join(&target_name);
                        if output_path != target_path {
                            let _ = fs::rename(&output_path, &target_path);
                        }
                    }
                    Err(e) => {
                        warn!(
                            "decode_failed file={} error={}",
                            img_meta.bpg_filename,
                            e
                        );
                    }
                }
            }
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

/// Decode a BPG file back to its original format
fn decode_bpg_to_original(
    bpg_path: &Path,
    original_format: OriginalImageFormat,
    _original_filename: &str,
    settings: &ExtractionSettings,
) -> Result<PathBuf> {
    let stem = bpg_path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let parent = bpg_path.parent().unwrap_or(Path::new("."));

    match original_format {
        OriginalImageFormat::Jpeg => {
            // BPG → JPEG directly
            let output_path = parent.join(format!("{}.jpg", stem));
            decode_bpg_to_jpeg(bpg_path, &output_path, settings.jpeg_quality)?;
            Ok(output_path)
        }
        #[cfg(feature = "heif")]
        OriginalImageFormat::Heic => {
            // BPG → PNG → HEIC
            let temp_png = parent.join(format!("{}_temp.png", stem));
            let output_path = parent.join(format!("{}.heic", stem));

            decode_bpg_to_png(bpg_path, &temp_png)?;

            if HeicCodec::is_available() {
                let codec = HeicCodec::new()?;
                let config = HeicEncoderConfig {
                    quality: settings.heic_quality,
                    lossless: false,
                    format: HeifCompressionFormat::HEVC,
                };
                codec.png_to_heic(&temp_png, &output_path, &config)?;
                let _ = fs::remove_file(&temp_png);
                Ok(output_path)
            } else {
                // Fallback to PNG if HEIC encoding not available
                let png_output = parent.join(format!("{}.png", stem));
                fs::rename(&temp_png, &png_output)?;
                Ok(png_output)
            }
        }
        #[cfg(not(feature = "heif"))]
        OriginalImageFormat::Heic => {
            // Fallback to PNG when HEIC support is not compiled
            let output_path = parent.join(format!("{}.png", stem));
            decode_bpg_to_png(bpg_path, &output_path)?;
            Ok(output_path)
        }
        OriginalImageFormat::Raw | OriginalImageFormat::Png |
        OriginalImageFormat::Tiff | OriginalImageFormat::Bmp | OriginalImageFormat::WebP => {
            // BPG → PNG (RAW cannot be recreated, others convert to PNG for compatibility)
            let output_path = parent.join(format!("{}.png", stem));
            decode_bpg_to_png(bpg_path, &output_path)?;
            Ok(output_path)
        }
    }
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
            let rgb_data: Vec<u8> = data.chunks(4)
                .flat_map(|rgba| [rgba[0], rgba[1], rgba[2]])
                .collect();

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
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, quality);
                rgb.write_with_encoder(encoder)?;
                let _ = fs::remove_file(&temp_png);
                Ok(())
            } else {
                Err(anyhow!("No BPG decoder available"))
            }
        }
    }
}
