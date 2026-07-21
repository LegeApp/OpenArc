//! Interactive CLI wizard for OpenArc
//! Provides a friendly, guided interface with drag-and-drop support

use anyhow::{anyhow, bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::bpg_wrapper::{BpgAq, BpgEffort};
use crate::orchestrator::{
    append_external_video_bundle, create_archive_from_discovered, extract_archive_with_decoding,
    ExtractionSettings, OrchestratorSettings,
};
use crate::phone_backup;

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
    prompt: "\x1b[97m",     // Bright white
    info: "\x1b[36m",       // Cyan
    highlight: "\x1b[35m",  // Magenta
    success: "\x1b[92m",    // Bright green
    warning: "\x1b[93m",    // Bright yellow
    error: "\x1b[91m",      // Bright red
    processing: "\x1b[96m", // Bright cyan
    reset: "\x1b[0m",       // Reset
};

// ============================================================================
// Processing Settings
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessingMode {
    EncodeAndArchive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartAction {
    ArchiveNoReencode,
    ArchiveWithReencode,
    FolderWithReencode,
    ExtractNoReencode,
    ExtractWithReencode,
}

pub struct InteractiveConfig {
    pub bpg_effort: BpgEffort,
    pub bpg_aq: BpgAq,
    pub bpg_bit_depth: u8,
    /// Advanced/testing numeric override. Normal interactive mode leaves this as None.
    pub bpg_quality_override: Option<i32>,
    /// ZSTD level (1-22) for the final archive container (low, since it wraps
    /// already-compressed media).
    pub compression_level: i32,
    /// LZMA2 level (1-9) for misc.arc.
    pub misc_compression_level: i32,
    pub enable_catalog: bool,
    pub enable_dedup: bool,
    pub enable_tracking: bool,
    pub mode: ProcessingMode,
    pub output_path: PathBuf,
    pub input_paths: Vec<PathBuf>,
    pub reencode_media: bool,
    pub output_folder_without_archive: bool,
    pub catalog_db_path: Option<PathBuf>,
}

impl Default for InteractiveConfig {
    fn default() -> Self {
        Self {
            bpg_effort: BpgEffort::Best,
            bpg_aq: BpgAq::default(),
            bpg_bit_depth: 8,
            bpg_quality_override: None,
            compression_level: 3,
            misc_compression_level: 6,
            enable_catalog: true,
            enable_dedup: true,
            enable_tracking: true,
            mode: ProcessingMode::EncodeAndArchive,
            output_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            input_paths: Vec::new(),
            reencode_media: true,
            output_folder_without_archive: false,
            catalog_db_path: None,
        }
    }
}

// ============================================================================
// Main Interactive Entry Point
// ============================================================================

pub fn run_interactive() -> Result<()> {
    println!(
        "{}╔════════════════════════════════════════╗{}",
        COLORS.info, COLORS.reset
    );
    println!(
        "{}║   OpenArc - Media Archival Wizard      ║{}",
        COLORS.info, COLORS.reset
    );
    println!(
        "{}╚════════════════════════════════════════╝{}",
        COLORS.info, COLORS.reset
    );
    let action = prompt_start_action()?;
    if matches!(
        action,
        StartAction::ExtractNoReencode | StartAction::ExtractWithReencode
    ) {
        let decode_images = matches!(action, StartAction::ExtractWithReencode);
        return run_extract_interactive(decode_images);
    }

    let mut config = InteractiveConfig::default();
    config.reencode_media = matches!(
        action,
        StartAction::ArchiveWithReencode | StartAction::FolderWithReencode
    );
    config.output_folder_without_archive = matches!(action, StartAction::FolderWithReencode);

    // Step 1: Collect input paths (or auto-stage phone if detected)
    println!(
        "{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
        COLORS.highlight, COLORS.reset
    );
    println!(
        "{}Step 1/3: Input Files & Folders{}",
        COLORS.highlight, COLORS.reset
    );
    println!(
        "{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
        COLORS.highlight, COLORS.reset
    );
    if let Some(staged_phone) = maybe_prepare_phone_input()? {
        config.input_paths = vec![staged_phone.staged_root.clone()];
        config.catalog_db_path = Some(staged_phone.catalog_db_path.clone());
        println!(
            "{}✓ Phone detected: {}{}",
            COLORS.success, staged_phone.display_name, COLORS.reset
        );
        println!(
            "{}  Staged files: {} (copied: {}, reused: {}){}",
            COLORS.info,
            staged_phone.total_files,
            staged_phone.copied_files,
            staged_phone.reused_files,
            COLORS.reset
        );
        println!(
            "{}  Staging folder: {}{}",
            COLORS.info,
            staged_phone.staged_root.display(),
            COLORS.reset
        );
    } else {
        config.input_paths = collect_input_paths()?;
    }

    if config.input_paths.is_empty() {
        println!(
            "{}No files selected. Exiting.{}",
            COLORS.warning, COLORS.reset
        );
        return Ok(());
    }

    // Use the core discovery rules here and pass this exact snapshot into the
    // pipeline.  The old wizard counted only known media, then rescanned and
    // silently added misc files after confirmation, so totals could change.
    let media_files = crate::orchestrator::collect_files(&config.input_paths)?;
    if media_files.is_empty() {
        println!("{}No files found. Exiting.{}", COLORS.warning, COLORS.reset);
        return Ok(());
    }
    println!(
        "\n{}✓ Found {} files{}",
        COLORS.success,
        media_files.len(),
        COLORS.reset
    );

    // Step 2: Image & Video Settings
    println!(
        "\n{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
        COLORS.highlight, COLORS.reset
    );
    println!(
        "{}Step 2/3: {}{}",
        COLORS.highlight,
        if config.reencode_media {
            "Encoding & Archive Settings"
        } else {
            "Archive Settings"
        },
        COLORS.reset
    );
    println!(
        "{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
        COLORS.highlight, COLORS.reset
    );
    prompt_compression_settings(&mut config)?;

    // Step 3: Output Location
    println!(
        "\n{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
        COLORS.highlight, COLORS.reset
    );
    println!(
        "{}Step 3/3: Output Location{}",
        COLORS.highlight, COLORS.reset
    );
    println!(
        "{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
        COLORS.highlight, COLORS.reset
    );
    config.mode = ProcessingMode::EncodeAndArchive;
    config.output_path = prompt_output_location(&config)?;

    // Summary and confirmation
    print_summary(&config, &media_files)?;

    println!(
        "\n{}Press Enter to start processing, or Ctrl+C to cancel...{}",
        COLORS.prompt, COLORS.reset
    );
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm)?;

    // Process!
    process_files(&config, media_files)?;

    Ok(())
}

fn prompt_start_action() -> Result<StartAction> {
    println!("{}Choose action:{}", COLORS.info, COLORS.reset);
    println!(
        "[1] {}Archive Originals{} - Preserve media bytes; lossless container compression only",
        COLORS.highlight, COLORS.reset
    );
    println!(
        "[2] {}Archive + Re-encode Media{} - Images→BPG, inefficient videos→external staging",
        COLORS.highlight, COLORS.reset
    );
    println!(
        "[3] {}Encode to Folder (No Zip){} - Re-encode media into an OpenArc folder layout",
        COLORS.highlight, COLORS.reset
    );
    println!(
        "[4] {}Extract (No Re-encode){} - Keep archived encoded files",
        COLORS.highlight, COLORS.reset
    );
    println!(
        "[5] {}Extract (Re-encode){} - Decode media back from archive",
        COLORS.highlight, COLORS.reset
    );
    print!("{}Choice [2]:{} ", COLORS.prompt, COLORS.reset);
    io::stdout().flush()?;

    let choice = read_number_or_default(2, 1, 5)?;
    Ok(match choice {
        1 => StartAction::ArchiveNoReencode,
        2 => StartAction::ArchiveWithReencode,
        3 => StartAction::FolderWithReencode,
        4 => StartAction::ExtractNoReencode,
        _ => StartAction::ExtractWithReencode,
    })
}

fn run_extract_interactive(decode_images: bool) -> Result<()> {
    println!(
        "\n{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
        COLORS.highlight, COLORS.reset
    );
    println!("{}Archive Extraction{}", COLORS.highlight, COLORS.reset);
    println!(
        "{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
        COLORS.highlight, COLORS.reset
    );

    print!("{}Archive path (.oarc):{} ", COLORS.prompt, COLORS.reset);
    io::stdout().flush()?;
    let mut archive_input = String::new();
    io::stdin().read_line(&mut archive_input)?;
    let archive_raw = archive_input.trim().trim_matches('"');
    let archive_path = PathBuf::from(archive_raw);
    if archive_path.as_os_str().is_empty() {
        return Err(anyhow!("No archive path provided"));
    }
    if !archive_path.exists() {
        return Err(anyhow!("Archive not found: {}", archive_path.display()));
    }

    let default_output = std::env::current_dir()?.join("extracted_openarc");
    print!(
        "{}Output folder [{}]:{} ",
        COLORS.prompt,
        default_output.display(),
        COLORS.reset
    );
    io::stdout().flush()?;
    let mut output_input = String::new();
    io::stdin().read_line(&mut output_input)?;
    let output_dir = if output_input.trim().is_empty() {
        default_output
    } else {
        PathBuf::from(output_input.trim().trim_matches('"'))
    };

    println!("\nArchive: {}", archive_path.display());
    println!("Output:  {}", output_dir.display());
    println!(
        "Decode mode: {}",
        if decode_images {
            "re-encode on extract"
        } else {
            "no re-encode (keep archived media formats)"
        }
    );
    println!(
        "\n{}Press Enter to start extraction, or Ctrl+C to cancel...{}",
        COLORS.prompt, COLORS.reset
    );
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm)?;

    let pb = ProgressBar::new(1);
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

    let result = extract_archive_with_decoding(
        &archive_path,
        &output_dir,
        ExtractionSettings {
            decode_images,
            ..ExtractionSettings::default()
        },
        Some(progress_fn),
    )?;
    pb.finish_with_message("Complete!");

    println!(
        "\n{}╔════════════════════════════════════════╗{}",
        COLORS.success, COLORS.reset
    );
    println!(
        "{}║         Extraction Complete!           ║{}",
        COLORS.success, COLORS.reset
    );
    println!(
        "{}╚════════════════════════════════════════╝{}",
        COLORS.success, COLORS.reset
    );
    println!("\n{}Statistics:{}", COLORS.info, COLORS.reset);
    println!("  • Files extracted: {}", result.files_extracted);
    println!("  • Total size: {} MB", result.total_size / 1_000_000);
    println!("  • Decoded images: {}", result.decoded_files);
    println!("  • Output: {}", output_dir.display());

    Ok(())
}

fn maybe_prepare_phone_input() -> Result<Option<phone_backup::StagedPhoneInput>> {
    let phones = match phone_backup::detect_phones() {
        Ok(phones) => phones,
        Err(e) => {
            eprintln!(
                "{}Phone detection failed (continuing with manual paths): {}{}",
                COLORS.warning, e, COLORS.reset
            );
            return Ok(None);
        }
    };

    if phones.is_empty() {
        return Ok(None);
    }

    println!(
        "{}Detected connected phone source(s):{}",
        COLORS.info, COLORS.reset
    );
    for (idx, phone) in phones.iter().enumerate() {
        let source = match phone.source_kind {
            phone_backup::PhoneSourceKind::Mtp => "MTP",
            phone_backup::PhoneSourceKind::MountedFilesystem => "mounted storage",
        };
        println!("  [{}] {} ({})", idx + 1, phone.display_name, source);
    }

    let selected = if phones.len() == 1 {
        phones[0].clone()
    } else {
        print!("{}Choose phone source [1]:{} ", COLORS.prompt, COLORS.reset);
        io::stdout().flush()?;
        let selected_idx = read_number_or_default(1, 1, phones.len() as i32)? as usize - 1;
        phones
            .get(selected_idx)
            .cloned()
            .ok_or_else(|| anyhow!("Invalid phone selection"))?
    };

    print!(
        "{}Use this phone as input and stage files now? (Y/n):{} ",
        COLORS.prompt, COLORS.reset
    );
    io::stdout().flush()?;
    if !read_yes_no(true)? {
        return Ok(None);
    }

    println!(
        "{}Staging phone media folders (Documents/Downloads/Pictures/DCIM/Videos)...{}",
        COLORS.processing, COLORS.reset
    );
    let staged = phone_backup::stage_phone_media(&selected)?;
    Ok(Some(staged))
}

// ============================================================================
// Input Collection
// ============================================================================

fn collect_input_paths() -> Result<Vec<PathBuf>> {
    println!(
        "{}Drag-and-drop or paste file/folder paths below{}",
        COLORS.info, COLORS.reset
    );
    println!(
        "{}(One per line, or space-separated. Press Enter twice when done){}",
        COLORS.info, COLORS.reset
    );
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

// ============================================================================
// Settings Prompts
// ============================================================================

fn bpg_effort_hint(effort: BpgEffort) -> String {
    effort.hint().to_string()
}

#[cfg(feature = "bpg-rs")]
fn bpg_aq_hint(aq: BpgAq) -> &'static str {
    match aq {
        BpgAq::Off => "uniform QP; fastest and most predictable",
        BpgAq::TwoPass => "recommended measured two-pass AQ; only keeps a winning candidate",
        BpgAq::Perceptual => "single-pass luma perceptual AQ",
        BpgAq::PerceptualChroma => "single-pass luma+chroma perceptual AQ",
    }
}

fn prompt_compression_settings(config: &mut InteractiveConfig) -> Result<()> {
    use crate::interactive_menu::{select_option, select_value, SelectOption};

    if config.reencode_media {
        println!(
            "\n{}Image Settings (BPG Format):{}",
            COLORS.info, COLORS.reset
        );
    }

    // Keep the legacy backend buildable, but present the same production
    // Fast/Best effort shape as the default bpg-rs backend.
    #[cfg(not(feature = "bpg-rs"))]
    if config.reencode_media {
        let bpg_options = [
            SelectOption::new("Best", bpg_effort_hint(BpgEffort::Best)),
            SelectOption::new("Fast", bpg_effort_hint(BpgEffort::Fast)),
        ];
        let default_idx = usize::from(config.bpg_effort == BpgEffort::Fast);
        let idx = select_option("BPG image preset:", &bpg_options, default_idx)?;
        config.bpg_effort = if idx == 1 {
            BpgEffort::Fast
        } else {
            BpgEffort::Best
        };
        // AQ is not user-selectable for the C backend; keep the production
        // default off.
        config.bpg_aq = BpgAq::Off;
    }

    #[cfg(feature = "bpg-rs")]
    if config.reencode_media {
        let bpg_options = [
            SelectOption::new("Best", bpg_effort_hint(BpgEffort::Best)),
            SelectOption::new("Fast", bpg_effort_hint(BpgEffort::Fast)),
        ];
        let default_bpg_idx = usize::from(config.bpg_effort == BpgEffort::Fast);
        let bpg_idx = select_option("BPG image preset:", &bpg_options, default_bpg_idx)?;
        config.bpg_effort = if bpg_idx == 1 {
            BpgEffort::Fast
        } else {
            BpgEffort::Best
        };
        if !config.bpg_effort.supports_two_pass_aq() && config.bpg_aq == BpgAq::TwoPass {
            config.bpg_aq = BpgAq::Off;
        }

        // AQ is off by default. Two-pass is the recommended opt-in, followed by
        // the two single-pass perceptual alternatives.
        let aq_choices: Vec<BpgAq> = if config.bpg_effort.supports_two_pass_aq() {
            vec![
                BpgAq::Off,
                BpgAq::TwoPass,
                BpgAq::Perceptual,
                BpgAq::PerceptualChroma,
            ]
        } else {
            vec![BpgAq::Off, BpgAq::Perceptual, BpgAq::PerceptualChroma]
        };
        let aq_options: Vec<SelectOption> = aq_choices
            .iter()
            .map(|aq| SelectOption::new(aq.as_str(), bpg_aq_hint(*aq)))
            .collect();
        let default_aq_idx = aq_choices
            .iter()
            .position(|aq| *aq == config.bpg_aq)
            .unwrap_or(0);
        let aq_idx = select_option("BPG adaptive quantization:", &aq_options, default_aq_idx)?;
        config.bpg_aq = aq_choices[aq_idx];
        config.bpg_aq.validate_for_effort(config.bpg_effort)?;
    }

    println!("\n{}Archive Settings:{}", COLORS.info, COLORS.reset);
    config.misc_compression_level = select_value(
        "Misc-file compression (LZMA2 level for non-media files):",
        1,
        9,
        config.misc_compression_level,
        1,
        2,
        |v| format!("level {v}"),
    )?;

    // File tracking toggle
    println!("\n{}File Tracking:{}", COLORS.info, COLORS.reset);
    println!("Tracks processed files across runs, detects duplicates.");
    print!("{}Enable tracking? (Y/n):{} ", COLORS.prompt, COLORS.reset);
    io::stdout().flush()?;
    config.enable_tracking = read_yes_no(true)?;
    if config.enable_tracking {
        println!("{}✓ File tracking enabled{}", COLORS.success, COLORS.reset);
    } else {
        println!("{}  File tracking disabled{}", COLORS.info, COLORS.reset);
    }

    Ok(())
}

fn prompt_output_location(config: &InteractiveConfig) -> Result<PathBuf> {
    let default_dir = std::env::current_dir()?;

    match config.mode {
        ProcessingMode::EncodeAndArchive => {
            let default_output = if config.output_folder_without_archive {
                println!("\n{}Output folder:{}", COLORS.info, COLORS.reset);
                default_dir.join("openarc_encoded")
            } else {
                println!(
                    "\n{}Archive output file (.oarc):{}",
                    COLORS.info, COLORS.reset
                );
                default_dir.join("openarc_archive.oarc")
            };
            println!("Default: {}", default_output.display());
            print!(
                "{}Path [{}]:{} ",
                COLORS.prompt,
                default_output.display(),
                COLORS.reset
            );
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let trimmed = input.trim();

            if trimmed.is_empty() {
                Ok(default_output)
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
    println!(
        "\n{}╔════════════════════════════════════════╗{}",
        COLORS.success, COLORS.reset
    );
    println!(
        "{}║          Processing Summary            ║{}",
        COLORS.success, COLORS.reset
    );
    println!(
        "{}╚════════════════════════════════════════╝{}",
        COLORS.success, COLORS.reset
    );

    println!(
        "\n{}Files to process:{} {}",
        COLORS.info,
        COLORS.reset,
        media_files.len()
    );

    // Count file classes for summary output
    let image_count = media_files.iter().filter(|p| is_image_file(p)).count();
    let video_count = media_files.iter().filter(|p| is_video_file(p)).count();
    let misc_count = media_files.len().saturating_sub(image_count + video_count);

    if config.reencode_media {
        if image_count > 0 {
            println!(
                "  • {} images → BPG (preset: {}, bit depth: adaptive, 8-bit \
                 sources stay 8-bit, high-depth sources up to 12-bit, AQ: {})",
                image_count, config.bpg_effort, config.bpg_aq
            );
        }
        if video_count > 0 {
            println!("  • {} videos staged for external encoding", video_count);
        }
        if misc_count > 0 {
            println!("  • {} other files archived as-is", misc_count);
        }
    } else {
        println!("  • Media files archived as-is (no image transcoding)");
        if misc_count > 0 {
            println!("  • Other files archived as-is: {}", misc_count);
        }
    }

    println!(
        "\n{}Mode:{} {}",
        COLORS.info,
        COLORS.reset,
        if config.output_folder_without_archive {
            "Encode to Folder (no final archive)"
        } else if config.reencode_media {
            "Archive with media re-encoding"
        } else {
            "Archive originals (no media re-encoding)"
        }
    );

    if config.mode == ProcessingMode::EncodeAndArchive {
        if config.output_folder_without_archive {
            println!(
                "{}Output folder:{} {}",
                COLORS.info,
                COLORS.reset,
                config.output_path.display()
            );
            println!(
                "  • Final archive container: disabled; misc LZMA2 level {}",
                config.misc_compression_level
            );
        } else {
            println!(
                "{}Archive:{} {}",
                COLORS.info,
                COLORS.reset,
                config.output_path.display()
            );
            println!(
                "  • Lossless container compression: Zstandard level {}",
                config.compression_level
            );
            println!(
                "  • Compressible misc/RAW bundles: LZMA2 level {} (already-compressed media is stored directly)",
                config.misc_compression_level
            );
        }
        println!(
            "  • Catalog: {}",
            if config.enable_catalog {
                "enabled"
            } else {
                "disabled"
            }
        );
        if let Some(ref catalog_path) = config.catalog_db_path {
            println!("  • Catalog DB: {}", catalog_path.display());
        } else if config.enable_catalog || config.enable_tracking {
            println!(
                "  • History DB: {}",
                crate::file_tracker::openarc_data_dir()
                    .join("tracking.db")
                    .display()
            );
        }
        println!(
            "  • Duplicate detection: {}",
            if config.enable_dedup {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!(
            "  • File tracking: {}",
            if config.enable_tracking {
                "enabled"
            } else {
                "disabled"
            }
        );
    } else {
        println!(
            "{}Output folder:{} {}",
            COLORS.info,
            COLORS.reset,
            config.output_path.display()
        );
    }

    Ok(())
}

fn process_files(config: &InteractiveConfig, _media_files: Vec<PathBuf>) -> Result<()> {
    println!(
        "\n{}Starting processing...{}",
        COLORS.processing, COLORS.reset
    );

    match config.mode {
        ProcessingMode::EncodeAndArchive => {
            // Use existing archive creation
            let settings = OrchestratorSettings {
                bpg_quality: config
                    .bpg_quality_override
                    .unwrap_or_else(|| config.bpg_effort.default_quality() as i32),
                bpg_effort: config.bpg_effort,
                bpg_aq: config.bpg_aq,
                bpg_bit_depth: config.bpg_bit_depth as i32,
                bpg_chroma_format: 1,
                bpg_encoder_type: config.bpg_effort.encoder_type() as i32,
                bpg_compression_level: config.bpg_effort.compression_level() as i32,
                compression_level: config.compression_level,
                misc_compression_level: config.misc_compression_level,
                enable_catalog: config.enable_catalog,
                catalog_db_path: config.catalog_db_path.clone(),
                enable_dedup: config.enable_dedup,
                staging_dir: None,
                heic_quality: 90,
                jpeg_quality: 92,
                enable_tracking: config.enable_tracking,
                reencode_media: config.reencode_media,
                output_folder_without_archive: config.output_folder_without_archive,
            };

            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{elapsed_precise}] {msg}")
                    .unwrap(),
            );
            pb.set_message("Starting...");
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            let pb_clone = pb.clone();
            let bar_style = Arc::new(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            let progress_fn = Arc::new(move |current: usize, total: usize, msg: &str| {
                if total > 0 {
                    pb_clone.set_style((*bar_style).clone());
                    pb_clone.set_length(total as u64);
                    pb_clone.set_position((current as u64).min(total as u64));
                }
                pb_clone.set_message(msg.to_string());
            });

            let result = create_archive_from_discovered(
                &config.input_paths,
                _media_files,
                &config.output_path,
                settings,
                Some(progress_fn),
            )?;

            pb.finish_with_message("Complete!");

            if !config.output_folder_without_archive
                && !result.staged_uncompressed_videos.is_empty()
            {
                let stage_root = result.video_staging_dir.as_deref().ok_or_else(|| {
                    anyhow!("videos were staged but the staging root was not returned")
                })?;
                let partial_output = partial_archive_path(&config.output_path);
                fs::rename(&config.output_path, &partial_output).with_context(|| {
                    format!(
                        "Failed to move incomplete archive {} to {}",
                        config.output_path.display(),
                        partial_output.display()
                    )
                })?;
                let merged = wait_for_external_video_encoding(
                    &partial_output,
                    stage_root,
                    result.staged_uncompressed_videos.len(),
                    config.compression_level,
                )?;
                fs::rename(&partial_output, &config.output_path).with_context(|| {
                    format!(
                        "Videos were merged, but the finalized archive could not be moved to {}. It remains at {}",
                        config.output_path.display(),
                        partial_output.display()
                    )
                })?;
                let _ = fs::remove_dir_all(stage_root);
                println!("  • Merged externally encoded videos: {}", merged);
            }

            println!(
                "\n{}╔════════════════════════════════════════╗{}",
                COLORS.success, COLORS.reset
            );
            println!(
                "{}║         Processing Complete!           ║{}",
                COLORS.success, COLORS.reset
            );
            println!(
                "{}╚════════════════════════════════════════╝{}",
                COLORS.success, COLORS.reset
            );

            println!("\n{}Statistics:{}", COLORS.info, COLORS.reset);
            println!("  • Processed: {} files", result.processed.len());
            println!("  • Skipped: {} files", result.skipped_by_catalog.len());
            if result.dedup_groups > 0 {
                println!(
                    "  • Duplicate content detected: {} groups (all paths preserved)",
                    result.dedup_groups
                );
            }

            let total_original: u64 = result.processed.iter().map(|p| p.original_size).sum();
            let total_compressed = artifact_size(&config.output_path)
                .unwrap_or_else(|| result.processed.iter().map(|p| p.output_size).sum());
            let ratio = if total_original > 0 {
                (total_compressed as f64 / total_original as f64) * 100.0
            } else {
                100.0
            };

            println!("\n{}Compression:{}", COLORS.info, COLORS.reset);
            println!("  • Original: {} MB", total_original / 1_000_000);
            println!("  • Compressed: {} MB", total_compressed / 1_000_000);
            println!(
                "  • Ratio: {}{:.1}%{} of original size",
                if ratio < 50.0 {
                    COLORS.success
                } else {
                    COLORS.info
                },
                ratio,
                COLORS.reset
            );
            if result.jpeg2000_fallback.replaced_files > 0 {
                println!(
                    "  • JPEG 2000 fallback: {} files encoded to JP2 q85 after BPG bitrate criteria flagged them and JP2 was smaller",
                    result.jpeg2000_fallback.replaced_files
                );
                println!(
                    "  • JPEG 2000 average savings: {} KB/file ({:.2}% average across replaced files)",
                    result.jpeg2000_fallback.average_saved_bytes() / 1_000,
                    result.jpeg2000_fallback.average_saved_percent()
                );
            } else if result.jpeg2000_fallback.flagged_files > 0 {
                println!(
                    "  • JPEG 2000 fallback: {} files flagged by BPG bitrate criteria, but BPG remained smaller",
                    result.jpeg2000_fallback.flagged_files
                );
            }

            println!(
                "\n{}Output:{} {}",
                COLORS.highlight,
                COLORS.reset,
                config.output_path.display()
            );

            if config.output_folder_without_archive && !result.staged_uncompressed_videos.is_empty()
            {
                println!(
                    "{}Staged uncompressed videos:{} {}",
                    COLORS.info,
                    COLORS.reset,
                    result.staged_uncompressed_videos.len()
                );
                if let Some(stage_root) = result.video_staging_dir.as_deref() {
                    println!("  • Stage folder: {}", stage_root.display());
                }
                println!(
                    "  • Folder mode does not finalize an archive; encode these videos externally and place the results in the output media layout."
                );
            }

            if result.tracking_report.is_some() {
                println!("{}  • File tracking: recorded{}", COLORS.info, COLORS.reset);
            }
        }
    }

    Ok(())
}

fn is_video_file(path: &PathBuf) -> bool {
    const VIDEO_EXTS: &[&str] = &[
        "mp4", "mov", "avi", "mkv", "webm", "m4v", "3gp", "flv", "wmv", "mts", "m2ts",
    ];

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        VIDEO_EXTS.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}

fn artifact_size(path: &Path) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.is_file() {
        return Some(metadata.len());
    }
    Some(
        walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| entry.metadata().ok().map(|m| m.len()))
            .sum(),
    )
}

fn partial_archive_path(output: &Path) -> PathBuf {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("archive.oarc");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    parent.join(format!(
        ".{name}.openarc-partial-{}-{nonce}",
        std::process::id()
    ))
}

fn wait_for_external_video_encoding(
    archive_path: &Path,
    stage_root: &Path,
    expected_count: usize,
    compression_level: i32,
) -> Result<usize> {
    println!(
        "\n{}External video encoding is required before the archive can be finalized.{}",
        COLORS.info, COLORS.reset
    );
    println!("  • Staged originals: {}", stage_root.display());
    println!(
        "  • Encode all {} staged video(s) with HandBrake/ffmpeg/etc. into a clean output folder.",
        expected_count
    );

    loop {
        print!(
            "{}Encoded video output folder (OpenArc will keep waiting):{} ",
            COLORS.prompt, COLORS.reset
        );
        io::stdout().flush()?;
        let mut encoded_dir = String::new();
        let bytes_read = io::stdin().read_line(&mut encoded_dir)?;
        if bytes_read == 0 {
            return Err(anyhow!(
                "standard input closed while waiting for externally encoded videos; partial archive remains at {}",
                archive_path.display()
            ));
        }
        let encoded_dir = encoded_dir.trim().trim_matches('"');
        if encoded_dir.is_empty() {
            println!("No path entered; the archive is still waiting for video output.");
            continue;
        }
        let encoded_dir = PathBuf::from(encoded_dir);
        if !encoded_dir.is_dir() {
            println!("Not a directory: {}", encoded_dir.display());
            continue;
        }

        match append_external_video_bundle(
            archive_path,
            &encoded_dir,
            expected_count,
            compression_level,
        ) {
            Ok(merged) => return Ok(merged),
            Err(err) => {
                println!("Could not use that output folder: {err:#}");
                println!("Fix the encoded outputs or enter a different folder.");
            }
        }
    }
}

fn is_image_file(path: &PathBuf) -> bool {
    const IMAGE_EXTS: &[&str] = &[
        "jpg", "jpeg", "png", "heic", "heif", "bpg", "tiff", "tif", "bmp", "webp", "dng", "cr2",
        "nef", "arw", "orf", "rw2", "raf", "jp2", "j2k",
    ];

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        IMAGE_EXTS.contains(&ext.to_lowercase().as_str())
    } else {
        false
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

    let value: i32 = trimmed
        .parse()
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
