//! Interactive CLI wizard for OpenArc
//! Provides a friendly, guided interface with drag-and-drop support

use anyhow::{Result, anyhow, bail};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use indicatif::{ProgressBar, ProgressStyle};

use crate::bpg_wrapper::{self, BpgConfig};
use crate::orchestrator::{create_archive, OrchestratorSettings};
use codecs::ffmpeg::{FfmpegEncodeOptions, FFmpegEncoder, VideoCodec, VideoSpeedPreset};
use codecs::video_analyzer::analyze_video_compression;

// ============================================================================
// COLOR CONFIGURATION
// ============================================================================
pub struct ColorConfig {
    pub prompt: &'static str,
    pub info: &'static str,
    pub highlight: &'static str,
    pub success: &'static str,
    pub warning: &'static str,
    pub error: &'static str,
    pub processing: &'static str,
    pub reset: &'static str,
}

pub const COLORS: ColorConfig = ColorConfig {
    prompt: "\x1b[97m",          // Bright white
    info: "\x1b[36m",            // Cyan
    highlight: "\x1b[35m",       // Magenta
    success: "\x1b[92m",         // Bright green
    warning: "\x1b[93m",         // Bright yellow
    error: "\x1b[91m",           // Bright red
    processing: "\x1b[96m",      // Bright cyan
    reset: "\x1b[0m",            // Reset
};

// ============================================================================
// Processing Settings
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessingMode {
    EncodeOnly,
    EncodeAndArchive,
}

pub struct InteractiveConfig {
    pub bpg_quality: i32,
    pub bpg_lossless: bool,
    pub bpg_bit_depth: u8,
    pub video_codec: String,
    pub video_preset: String,
    pub video_crf: i32,
    pub compression_level: i32,
    pub enable_catalog: bool,
    pub enable_dedup: bool,
    pub skip_compressed_videos: bool,
    pub mode: ProcessingMode,
    pub output_path: PathBuf,
    pub input_paths: Vec<PathBuf>,
}

impl Default for InteractiveConfig {
    fn default() -> Self {
        Self {
            bpg_quality: 28,
            bpg_lossless: false,
            bpg_bit_depth: 8,
            video_codec: "h264".to_string(),
            video_preset: "medium".to_string(),
            video_crf: 23,
            compression_level: 22,
            enable_catalog: true,
            enable_dedup: true,
            skip_compressed_videos: true,
            mode: ProcessingMode::EncodeAndArchive,
            output_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            input_paths: Vec::new(),
        }
    }
}

// ============================================================================
// Main Interactive Entry Point
// ============================================================================

pub fn run_interactive() -> Result<()> {
    println!("{}╔════════════════════════════════════════╗{}", COLORS.info, COLORS.reset);
    println!("{}║   OpenArc - Media Archival Wizard     ║{}", COLORS.info, COLORS.reset);
    println!("{}╚════════════════════════════════════════╝{}", COLORS.info, COLORS.reset);
    println!();
    println!("Welcome! This wizard will guide you through compressing");
    println!("images (to BPG) and videos (to H.264/H.265).\n");

    let mut config = InteractiveConfig::default();

    // Step 1: Collect input paths (most important - emphasized)
    println!("{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", COLORS.highlight, COLORS.reset);
    println!("{}Step 1/4: Input Files & Folders{}", COLORS.highlight, COLORS.reset);
    println!("{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", COLORS.highlight, COLORS.reset);
    config.input_paths = collect_input_paths()?;

    if config.input_paths.is_empty() {
        println!("{}No files selected. Exiting.{}", COLORS.warning, COLORS.reset);
        return Ok(());
    }

    let media_files = validate_and_expand_paths(&config.input_paths)?;
    println!("\n{}✓ Found {} media files{}", COLORS.success, media_files.len(), COLORS.reset);

    // Step 2: Image & Video Settings
    println!("\n{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", COLORS.highlight, COLORS.reset);
    println!("{}Step 2/4: Compression Settings{}", COLORS.highlight, COLORS.reset);
    println!("{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", COLORS.highlight, COLORS.reset);
    prompt_compression_settings(&mut config)?;

    // Step 3: Processing Mode
    println!("\n{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", COLORS.highlight, COLORS.reset);
    println!("{}Step 3/4: Processing Mode{}", COLORS.highlight, COLORS.reset);
    println!("{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", COLORS.highlight, COLORS.reset);
    config.mode = prompt_processing_mode()?;

    // Step 4: Output Directory
    println!("\n{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", COLORS.highlight, COLORS.reset);
    println!("{}Step 4/4: Output Location{}", COLORS.highlight, COLORS.reset);
    println!("{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", COLORS.highlight, COLORS.reset);
    config.output_path = prompt_output_location(&config.mode)?;

    // Summary and confirmation
    print_summary(&config, &media_files)?;

    println!("\n{}Press Enter to start processing, or Ctrl+C to cancel...{}",
        COLORS.prompt, COLORS.reset);
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm)?;

    // Process!
    process_files(&config, media_files)?;

    Ok(())
}

// ============================================================================
// Input Collection
// ============================================================================

fn collect_input_paths() -> Result<Vec<PathBuf>> {
    println!("{}Drag-and-drop or paste file/folder paths below{}", COLORS.info, COLORS.reset);
    println!("{}(One per line, or space-separated. Press Enter twice when done){}", COLORS.info, COLORS.reset);
    println!();
    print!("{}> {}", COLORS.prompt, COLORS.reset);
    io::stdout().flush()?;

    let mut paths = Vec::new();
    let mut empty_lines = 0;

    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();

        if trimmed.is_empty() {
            empty_lines += 1;
            if empty_lines >= 2 || !paths.is_empty() {
                break;
            }
            continue;
        }

        empty_lines = 0;

        // Parse paths from input
        let new_paths = parse_path_input(trimmed);
        paths.extend(new_paths.into_iter().map(PathBuf::from));

        if !trimmed.is_empty() {
            print!("{}> {}", COLORS.prompt, COLORS.reset);
            io::stdout().flush()?;
        }
    }

    Ok(paths)
}

fn parse_path_input(input: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for ch in input.chars() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
                if !current.is_empty() {
                    paths.push(current.clone());
                    current.clear();
                }
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    paths.push(current.clone());
                    current.clear();
                }
            }
            c => current.push(c),
        }
    }

    if !current.is_empty() {
        paths.push(current);
    }

    if paths.is_empty() && !input.trim().is_empty() {
        paths.push(input.trim().to_string());
    }

    paths
}

fn validate_and_expand_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut media_files = Vec::new();

    for path in paths {
        if !path.exists() {
            eprintln!("{}⚠ Path does not exist: {}{}",
                COLORS.warning, path.display(), COLORS.reset);
            continue;
        }

        if path.is_file() {
            if is_media_file(path) {
                media_files.push(path.clone());
            }
        } else if path.is_dir() {
            let dir_files = find_media_files(path)?;
            media_files.extend(dir_files);
        }
    }

    if media_files.is_empty() {
        bail!("No valid media files found");
    }

    Ok(media_files)
}

fn is_media_file(path: &PathBuf) -> bool {
    const IMAGE_EXTS: &[&str] = &[
        "jpg", "jpeg", "png", "heic", "heif", "bpg", "tiff", "tif", "bmp",
        "webp", "dng", "cr2", "nef", "arw", "orf", "rw2", "raf", "jp2", "j2k"
    ];
    const VIDEO_EXTS: &[&str] = &[
        "mp4", "mov", "avi", "mkv", "webm", "m4v", "wmv"
    ];

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        IMAGE_EXTS.contains(&ext_lower.as_str()) || VIDEO_EXTS.contains(&ext_lower.as_str())
    } else {
        false
    }
}

fn find_media_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_media_file(&path) {
                files.push(path);
            } else if path.is_dir() {
                let sub_files = find_media_files(&path)?;
                files.extend(sub_files);
            }
        }
    }

    Ok(files)
}

// ============================================================================
// Settings Prompts
// ============================================================================

fn prompt_compression_settings(config: &mut InteractiveConfig) -> Result<()> {
    println!("\n{}Image Settings (BPG Format):{}", COLORS.info, COLORS.reset);
    println!("Default: Quality=28 (0-51, lower=better), 8-bit depth");

    print!("{}Use defaults? (Y/n):{} ", COLORS.prompt, COLORS.reset);
    io::stdout().flush()?;

    if read_yes_no(true)? {
        println!("{}✓ Using default image settings{}", COLORS.success, COLORS.reset);
    } else {
        print!("{}BPG Quality (0-51) [28]:{} ", COLORS.prompt, COLORS.reset);
        io::stdout().flush()?;
        config.bpg_quality = read_number_or_default(28, 0, 51)?;

        print!("{}Bit Depth (8/10/12/14) [8]:{} ", COLORS.prompt, COLORS.reset);
        io::stdout().flush()?;
        config.bpg_bit_depth = read_number_or_default(8, 8, 14)? as u8;
    }

    println!("\n{}Video Settings:{}", COLORS.info, COLORS.reset);
    println!("Default: H.264, CRF=23, Medium speed");

    print!("{}Use defaults? (Y/n):{} ", COLORS.prompt, COLORS.reset);
    io::stdout().flush()?;

    if read_yes_no(true)? {
        println!("{}✓ Using default video settings{}", COLORS.success, COLORS.reset);
    } else {
        println!("Codec: [1] H.264 (compatible)  [2] H.265 (better compression)");
        print!("{}Choice [1]:{} ", COLORS.prompt, COLORS.reset);
        io::stdout().flush()?;
        let codec_choice = read_number_or_default(1, 1, 2)?;
        config.video_codec = if codec_choice == 2 { "h265".to_string() } else { "h264".to_string() };

        print!("{}CRF Quality (18-28, lower=better) [23]:{} ", COLORS.prompt, COLORS.reset);
        io::stdout().flush()?;
        config.video_crf = read_number_or_default(23, 0, 51)?;

        println!("Speed: [1] Fast  [2] Medium  [3] Slow");
        print!("{}Choice [2]:{} ", COLORS.prompt, COLORS.reset);
        io::stdout().flush()?;
        let speed_choice = read_number_or_default(2, 1, 3)?;
        config.video_preset = match speed_choice {
            1 => "fast".to_string(),
            3 => "slow".to_string(),
            _ => "medium".to_string(),
        };
    }

    Ok(())
}

fn prompt_processing_mode() -> Result<ProcessingMode> {
    println!("\n{}Choose processing mode:{}", COLORS.info, COLORS.reset);
    println!("[1] {}Encode Only{} - Compress files, save to output folder",
        COLORS.highlight, COLORS.reset);
    println!("[2] {}Encode + Archive{} - Compress AND create .oarc archive (recommended)",
        COLORS.highlight, COLORS.reset);
    println!();
    print!("{}Choice [2]:{} ", COLORS.prompt, COLORS.reset);
    io::stdout().flush()?;

    let choice = read_number_or_default(2, 1, 2)?;

    Ok(if choice == 1 {
        ProcessingMode::EncodeOnly
    } else {
        ProcessingMode::EncodeAndArchive
    })
}

fn prompt_output_location(mode: &ProcessingMode) -> Result<PathBuf> {
    let default_dir = std::env::current_dir()?;

    match mode {
        ProcessingMode::EncodeOnly => {
            println!("\n{}Output folder for compressed files:{}", COLORS.info, COLORS.reset);
            println!("Default: {}", default_dir.display());
            print!("{}Path [current directory]:{} ", COLORS.prompt, COLORS.reset);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let trimmed = input.trim();

            if trimmed.is_empty() {
                return Ok(default_dir);
            }

            let path = PathBuf::from(trimmed);
            if !path.exists() {
                fs::create_dir_all(&path)?;
                println!("{}✓ Created directory: {}{}",
                    COLORS.success, path.display(), COLORS.reset);
            }

            Ok(path)
        }
        ProcessingMode::EncodeAndArchive => {
            println!("\n{}Archive output file (.oarc):{}", COLORS.info, COLORS.reset);
            let default_archive = default_dir.join("openarc_archive.oarc");
            println!("Default: {}", default_archive.display());
            print!("{}Path [{}]:{} ",
                COLORS.prompt, default_archive.display(), COLORS.reset);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let trimmed = input.trim();

            if trimmed.is_empty() {
                Ok(default_archive)
            } else {
                Ok(PathBuf::from(trimmed))
            }
        }
    }
}

// ============================================================================
// Processing
// ============================================================================

fn print_summary(config: &InteractiveConfig, media_files: &[PathBuf]) -> Result<()> {
    println!("\n{}╔════════════════════════════════════════╗{}", COLORS.success, COLORS.reset);
    println!("{}║          Processing Summary            ║{}", COLORS.success, COLORS.reset);
    println!("{}╚════════════════════════════════════════╝{}", COLORS.success, COLORS.reset);

    println!("\n{}Files to process:{} {}", COLORS.info, COLORS.reset, media_files.len());

    // Count images vs videos
    let image_count = media_files.iter().filter(|p| is_image_file(p)).count();
    let video_count = media_files.len() - image_count;

    if image_count > 0 {
        println!("  • {} images → BPG (quality: {}, {}-bit)",
            image_count, config.bpg_quality, config.bpg_bit_depth);
    }
    if video_count > 0 {
        println!("  • {} videos → {} (CRF: {}, {})",
            video_count, config.video_codec.to_uppercase(), config.video_crf, config.video_preset);
    }

    println!("\n{}Mode:{} {}",
        COLORS.info, COLORS.reset,
        if config.mode == ProcessingMode::EncodeOnly {
            "Encode Only"
        } else {
            "Encode + Archive"
        });

    if config.mode == ProcessingMode::EncodeAndArchive {
        println!("{}Archive:{} {}", COLORS.info, COLORS.reset, config.output_path.display());
        println!("  • Compression: level {}", config.compression_level);
        println!("  • Catalog: {}", if config.enable_catalog { "enabled" } else { "disabled" });
        println!("  • Deduplication: {}", if config.enable_dedup { "enabled" } else { "disabled" });
    } else {
        println!("{}Output folder:{} {}", COLORS.info, COLORS.reset, config.output_path.display());
    }

    Ok(())
}

fn process_files(config: &InteractiveConfig, media_files: Vec<PathBuf>) -> Result<()> {
    println!("\n{}Starting processing...{}", COLORS.processing, COLORS.reset);

    match config.mode {
        ProcessingMode::EncodeAndArchive => {
            // Use existing archive creation
            let settings = OrchestratorSettings {
                bpg_quality: config.bpg_quality,
                bpg_lossless: config.bpg_lossless,
                bpg_bit_depth: config.bpg_bit_depth as i32,
                bpg_chroma_format: 1,
                bpg_encoder_type: 0,
                bpg_compression_level: 8,
                video_preset: video_preset_to_int(&config.video_codec, &config.video_preset),
                video_crf: config.video_crf,
                compression_level: config.compression_level,
                enable_catalog: config.enable_catalog,
                enable_dedup: config.enable_dedup,
                skip_already_compressed_videos: config.skip_compressed_videos,
                staging_dir: None,
                heic_quality: 90,
                jpeg_quality: 92,
            };

            let pb = ProgressBar::new(100);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                    .unwrap()
                    .progress_chars("#>-"),
            );

            let pb_clone = pb.clone();
            let progress_fn = Arc::new(move |current: usize, total: usize, msg: &str| {
                pb_clone.set_length(total as u64);
                pb_clone.set_position(current as u64);
                pb_clone.set_message(msg.to_string());
            });

            let result = create_archive(&config.input_paths, &config.output_path, settings, Some(progress_fn))?;

            pb.finish_with_message("Complete!");

            println!("\n{}╔════════════════════════════════════════╗{}", COLORS.success, COLORS.reset);
            println!("{}║         Processing Complete!           ║{}", COLORS.success, COLORS.reset);
            println!("{}╚════════════════════════════════════════╝{}", COLORS.success, COLORS.reset);

            println!("\n{}Statistics:{}", COLORS.info, COLORS.reset);
            println!("  • Processed: {} files", result.processed.len());
            println!("  • Skipped: {} files", result.skipped_by_catalog.len());
            if result.dedup_groups > 0 {
                println!("  • Deduplicated: {} groups", result.dedup_groups);
            }

            let total_original: u64 = result.processed.iter().map(|p| p.original_size).sum();
            let total_compressed: u64 = result.processed.iter().map(|p| p.output_size).sum();
            let ratio = if total_original > 0 {
                (total_compressed as f64 / total_original as f64) * 100.0
            } else {
                100.0
            };

            println!("\n{}Compression:{}", COLORS.info, COLORS.reset);
            println!("  • Original: {} MB", total_original / 1_000_000);
            println!("  • Compressed: {} MB", total_compressed / 1_000_000);
            println!("  • Ratio: {}{:.1}%{} of original size",
                if ratio < 50.0 { COLORS.success } else { COLORS.info },
                ratio,
                COLORS.reset);

            println!("\n{}Output:{} {}", COLORS.highlight, COLORS.reset, config.output_path.display());
        }
        ProcessingMode::EncodeOnly => {
            encode_only_mode(config, &media_files)?;
        }
    }

    Ok(())
}

// ============================================================================
// Encode-Only Mode
// ============================================================================

fn encode_only_mode(config: &InteractiveConfig, media_files: &[PathBuf]) -> Result<()> {
    let output_dir = &config.output_path;
    fs::create_dir_all(output_dir)?;

    let total = media_files.len();
    let start = Instant::now();

    let pb = ProgressBar::new(total as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let bpg_config = BpgConfig {
        quality: config.bpg_quality as u8,
        lossless: config.bpg_lossless,
        bit_depth: config.bpg_bit_depth,
        encoder_type: 0, // x265
        compression_level: 8,
    };

    let (codec, speed) = parse_video_settings(config);

    let mut encoded_count = 0u64;
    let mut skipped_count = 0u64;
    let mut error_count = 0u64;
    let mut total_original: u64 = 0;
    let mut total_output: u64 = 0;

    for (idx, path) in media_files.iter().enumerate() {
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        pb.set_position(idx as u64);

        if is_image_file(path) {
            // Encode image to BPG
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
            let out_path = output_dir.join(format!("{}.bpg", stem));

            pb.set_message(format!("BPG: {}", file_name));

            let original_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

            match bpg_wrapper::encode_image_to_bpg(path, &out_path, &bpg_config) {
                Ok(()) => {
                    let output_size = fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
                    total_original += original_size;
                    total_output += output_size;
                    encoded_count += 1;
                }
                Err(e) => {
                    pb.suspend(|| {
                        eprintln!("{}  ✗ Image error ({}): {}{}",
                            COLORS.error, file_name, e, COLORS.reset);
                    });
                    error_count += 1;
                }
            }
        } else if is_video_file(path) {
            // Check if video is already efficiently compressed
            pb.set_message(format!("Analyzing: {}", file_name));

            let analysis = safe_analyze_video(path);
            let should_skip = analysis
                .as_ref()
                .map(|a| a.is_efficiently_compressed)
                .unwrap_or(false);

            if should_skip {
                let reason = analysis.as_ref()
                    .map(|a| a.compression_reason.as_str())
                    .unwrap_or("already compressed");
                pb.suspend(|| {
                    println!("{}  → Skipped ({}): {}{}",
                        COLORS.info, reason, file_name, COLORS.reset);
                });

                // Copy as-is to output
                let out_path = output_dir.join(path.file_name().unwrap());
                let original_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                if let Err(e) = fs::copy(path, &out_path) {
                    pb.suspend(|| {
                        eprintln!("{}  ✗ Copy error ({}): {}{}",
                            COLORS.error, file_name, e, COLORS.reset);
                    });
                    error_count += 1;
                } else {
                    total_original += original_size;
                    total_output += original_size;
                    skipped_count += 1;
                }
            } else {
                // Re-encode video
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
                let out_path = output_dir.join(format!("{}.mp4", stem));

                pb.set_message(format!("{}: {}", config.video_codec.to_uppercase(), file_name));

                let original_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

                let opts = FfmpegEncodeOptions {
                    codec: codec.clone(),
                    speed: speed.clone(),
                    crf: Some(config.video_crf as u8),
                    copy_audio: true,
                };

                let enc = FFmpegEncoder::with_options(opts);
                match enc.encode_file(path, &out_path) {
                    Ok(()) => {
                        let output_size = fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
                        total_original += original_size;
                        total_output += output_size;
                        encoded_count += 1;
                    }
                    Err(e) => {
                        pb.suspend(|| {
                            eprintln!("{}  ✗ Video error ({}): {}{}",
                                COLORS.error, file_name, e, COLORS.reset);
                        });
                        error_count += 1;
                    }
                }
            }
        }
        // Non-media files are silently ignored (already filtered)
    }

    pb.finish_and_clear();

    // Print results
    let elapsed = start.elapsed();
    let minutes = elapsed.as_secs() / 60;
    let seconds = elapsed.as_secs() % 60;

    println!("\n{}╔════════════════════════════════════════╗{}", COLORS.success, COLORS.reset);
    println!("{}║         Encoding Complete!             ║{}", COLORS.success, COLORS.reset);
    println!("{}╚════════════════════════════════════════╝{}", COLORS.success, COLORS.reset);

    println!("\n{}Statistics:{}", COLORS.info, COLORS.reset);
    println!("  • Encoded: {} files", encoded_count);
    println!("  • Skipped (already compressed): {} videos", skipped_count);
    if error_count > 0 {
        println!("  {}• Errors: {} files{}", COLORS.error, error_count, COLORS.reset);
    }
    println!("  • Time: {}m {}s", minutes, seconds);

    if total_original > 0 {
        let ratio = (total_output as f64 / total_original as f64) * 100.0;
        let saved_mb = (total_original - total_output.min(total_original)) / 1_000_000;

        println!("\n{}Compression:{}", COLORS.info, COLORS.reset);
        println!("  • Original: {:.1} MB", total_original as f64 / 1_000_000.0);
        println!("  • Output: {:.1} MB", total_output as f64 / 1_000_000.0);
        println!("  • Ratio: {}{:.1}%{} of original",
            if ratio < 50.0 { COLORS.success } else { COLORS.info },
            ratio, COLORS.reset);
        println!("  • Saved: {:.1} MB", saved_mb as f64);
    }

    println!("\n{}Output:{} {}", COLORS.highlight, COLORS.reset, output_dir.display());

    Ok(())
}

/// Analyze video with timeout to prevent hangs (mirrors orchestrator logic)
fn safe_analyze_video(path: &Path) -> Option<codecs::video_analyzer::VideoAnalysis> {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let path = path.to_path_buf();
    let thread_path = path.clone();
    let (tx, rx) = mpsc::channel();

    let _handle = thread::spawn(move || {
        let _ = tx.send(std::panic::catch_unwind(|| analyze_video_compression(&thread_path)));
    });

    rx.recv_timeout(Duration::from_secs(5)).ok().and_then(|r| match r {
        Ok(Ok(v)) => Some(v),
        _ => None,
    })
}

fn parse_video_settings(config: &InteractiveConfig) -> (VideoCodec, VideoSpeedPreset) {
    let codec = if config.video_codec == "h265" {
        VideoCodec::H265
    } else {
        VideoCodec::H264
    };

    let speed = match config.video_preset.as_str() {
        "fast" => VideoSpeedPreset::Fast,
        "slow" => VideoSpeedPreset::Slow,
        _ => VideoSpeedPreset::Medium,
    };

    (codec, speed)
}

fn is_video_file(path: &PathBuf) -> bool {
    const VIDEO_EXTS: &[&str] = &[
        "mp4", "mov", "avi", "mkv", "webm", "m4v", "3gp", "flv", "wmv", "mts", "m2ts"
    ];

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        VIDEO_EXTS.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}

fn is_image_file(path: &PathBuf) -> bool {
    const IMAGE_EXTS: &[&str] = &[
        "jpg", "jpeg", "png", "heic", "heif", "bpg", "tiff", "tif", "bmp",
        "webp", "dng", "cr2", "nef", "arw", "orf", "rw2", "raf", "jp2", "j2k"
    ];

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        IMAGE_EXTS.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}

fn video_preset_to_int(codec: &str, preset: &str) -> i32 {
    match (codec, preset) {
        ("h264", "fast") => 2,
        ("h264", "medium") => 0,
        ("h265", "medium") => 1,
        ("h265", "slow") => 3,
        _ => 0,
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn read_number_or_default(default: i32, min: i32, max: i32) -> Result<i32> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Ok(default);
    }

    let value: i32 = trimmed.parse()
        .map_err(|_| anyhow!("Invalid number: {}", trimmed))?;

    if value < min || value > max {
        bail!("Value must be between {} and {}", min, max);
    }

    Ok(value)
}

fn read_yes_no(default: bool) -> Result<bool> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_lowercase();

    if trimmed.is_empty() {
        return Ok(default);
    }

    Ok(matches!(trimmed.as_str(), "y" | "yes" | "true" | "1"))
}
