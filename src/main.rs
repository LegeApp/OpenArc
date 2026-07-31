//! OpenArc - Media archiver for phone/camera files

use anyhow::{anyhow, Context, Result};

#[cfg(windows)]
fn enable_ansi_support() {
    unsafe extern "system" {
        fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
        fn GetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, lpMode: *mut u32) -> i32;
        fn SetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, dwMode: u32) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5_u32;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) != 0 {
            SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}
use arcmax::codec::lzma::LzmaOptions;
use arcmax::{compress_with, decompress, CompressionOptions, Method};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use openarc::bpg_wrapper::{BpgAq, BpgConfig, BpgEffort};
use openarc::cli::{Cli, Commands};
use openarc::interactive;
use openarc::orchestrator::{
    create_archive, extract_archive_with_decoding, list_archive_contents, ExtractionSettings,
    OrchestratorSettings,
};
use openarc::phone_backup;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

const CODEC_THREAD_STACK_SIZE: usize = 16 * 1024 * 1024;

fn run_with_codec_stack<F, T>(name: &'static str, f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    let handle = std::thread::Builder::new()
        .name(name.to_string())
        .stack_size(CODEC_THREAD_STACK_SIZE)
        .spawn(f)
        .with_context(|| format!("Failed to start {name} thread"))?;

    match handle.join() {
        Ok(result) => result,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("unknown panic payload");
            Err(anyhow!("{name} thread panicked: {message}"))
        }
    }
}

fn make_progress_callback(
    initial_message: &'static str,
) -> (ProgressBar, Arc<openarc::orchestrator::ProgressFn>) {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    pb.set_message(initial_message);
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let pb_clone = pb.clone();
    let bar_style = Arc::new(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    let progress_fn: Arc<openarc::orchestrator::ProgressFn> =
        Arc::new(move |current: usize, total: usize, msg: &str| {
            if total > 0 {
                pb_clone.set_style((*bar_style).clone());
                pb_clone.set_length(total as u64);
                pb_clone.set_position((current as u64).min(total as u64));
            }
            pb_clone.set_message(msg.to_string());
        });
    (pb, progress_fn)
}

fn is_supported_image(path: &std::path::Path) -> bool {
    const IMAGE_EXTS: &[&str] = &[
        "jpg", "jpeg", "png", "heic", "heif", "tiff", "tif", "bmp", "webp", "jp2", "j2k",
    ];
    raw_autotune::files::is_supported_raw(path)
        || path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
            .unwrap_or(false)
}

fn artifact_size(path: &std::path::Path) -> Option<u64> {
    let metadata = std::fs::metadata(path).ok()?;
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

fn partial_archive_path(output: &std::path::Path) -> PathBuf {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
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
    archive_path: &std::path::Path,
    stage_root: &std::path::Path,
    expected_count: usize,
    compression_level: i32,
) -> Result<usize> {
    println!();
    println!("External video encoding is required before the archive can be finalized.");
    println!("Staged originals: {}", stage_root.display());
    println!(
        "Encode all {} staged video(s) with HandBrake/ffmpeg/etc. into a clean output folder.",
        expected_count
    );

    loop {
        print!("Encoded video output folder (OpenArc will keep waiting): ");
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

        match openarc::orchestrator::append_external_video_bundle(
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

fn find_images(input_dir: &std::path::Path) -> Result<Vec<PathBuf>> {
    let mut images = Vec::new();
    for entry in walkdir::WalkDir::new(input_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path().to_path_buf();
        if is_supported_image(&path) {
            images.push(path);
        }
    }
    let raw_keys: std::collections::HashSet<(PathBuf, String)> = images
        .iter()
        .filter(|path| raw_autotune::files::is_supported_raw(path))
        .filter_map(|path| {
            Some((
                path.parent()
                    .unwrap_or_else(|| std::path::Path::new(""))
                    .to_path_buf(),
                path.file_stem()?.to_string_lossy().to_lowercase(),
            ))
        })
        .collect();
    images.retain(|path| {
        let is_jpeg = matches!(
            path.extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
            "jpg" | "jpeg"
        );
        !is_jpeg
            || !path
                .file_stem()
                .map(|stem| {
                    raw_keys.contains(&(
                        path.parent()
                            .unwrap_or_else(|| std::path::Path::new(""))
                            .to_path_buf(),
                        stem.to_string_lossy().to_lowercase(),
                    ))
                })
                .unwrap_or(false)
    });
    Ok(images)
}

fn main() -> Result<()> {
    #[cfg(windows)]
    enable_ansi_support();

    // If no arguments provided, launch interactive mode
    if std::env::args().len() == 1 {
        return run_with_codec_stack("openarc-interactive", interactive::run_interactive);
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::Interactive => {
            run_with_codec_stack("openarc-interactive", interactive::run_interactive)
        }
        Commands::Create {
            output,
            inputs,
            bpg_effort,
            #[cfg(feature = "bpg-rs")]
            bpg_aq,
            bpg_quality,
            compression_level,
            misc_compression_level,
            no_catalog,
            no_dedup,
            no_tracking,
            no_reencode,
            no_zip,
        } => {
            println!(
                "OpenArc - Creating {}: {}",
                if no_zip { "folder" } else { "archive" },
                output.display()
            );
            println!("Input sources: {} items", inputs.len());
            println!();

            // Validate that every input path actually exists before doing any work.
            // This catches the common mistake of forgetting to quote paths with spaces
            // (the shell splits "C:\My Folder" into ["C:\My", "Folder"]).
            let missing: Vec<&PathBuf> = inputs.iter().filter(|p| !p.exists()).collect();
            if !missing.is_empty() {
                eprintln!("error: the following input path(s) do not exist:");
                for p in &missing {
                    eprintln!("  {}", p.display());
                }
                eprintln!();
                eprintln!("Tip: if your path contains spaces, wrap it in quotes:");
                eprintln!("  openarc create \"C:\\path with spaces\" --output archive.oarc");
                return Err(anyhow!("one or more input paths not found"));
            }

            let bpg_effort = BpgEffort::parse(&bpg_effort)?;
            #[cfg(feature = "bpg-rs")]
            let bpg_aq = BpgAq::parse(&bpg_aq)?;
            // C/x265 backend: AQ is not user-selectable; keep the production
            // default off.
            #[cfg(not(feature = "bpg-rs"))]
            let bpg_aq: BpgAq = openarc::bpg_wrapper::AQ_CLI_DEFAULT;
            bpg_aq.validate_for_effort(bpg_effort)?;
            let bpg_quality = bpg_quality.unwrap_or_else(|| bpg_effort.default_quality() as i32);

            let settings = OrchestratorSettings {
                bpg_quality,
                bpg_effort,
                bpg_aq,
                bpg_bit_depth: 8,
                bpg_chroma_format: 1,
                bpg_encoder_type: bpg_effort.encoder_type() as i32,
                bpg_compression_level: bpg_effort.compression_level() as i32,
                compression_level,
                misc_compression_level,
                enable_catalog: !no_catalog,
                catalog_db_path: None,
                enable_dedup: !no_dedup,
                staging_dir: None,
                heic_quality: 90,
                jpeg_quality: 92,
                enable_tracking: !no_tracking,
                reencode_media: !no_reencode,
                output_folder_without_archive: no_zip,
            };

            println!("Settings:");
            if !no_reencode {
                println!(
                    "  BPG: {} (AQ: {}, internal QP: {})",
                    bpg_effort, bpg_aq, bpg_quality
                );
                println!(
                    "  BPG bit depth: adaptive (8-bit sources stay 8-bit, high-depth sources up to 12-bit)"
                );
            }
            println!("  ZSTD container level: {}", compression_level);
            println!("  Misc LZMA2 level: {}", misc_compression_level);
            println!("  Persistent history catalog: {}", !no_catalog);
            println!("  Duplicate detection: {}", !no_dedup);
            println!("  File tracking: {}", !no_tracking);
            if !no_catalog || !no_tracking {
                println!(
                    "  History DB: {}",
                    openarc::file_tracker::openarc_data_dir()
                        .join("tracking.db")
                        .display()
                );
            }
            println!("  Re-encode media: {}", !no_reencode);
            println!("  Final archive container: {}", !no_zip);
            println!();

            let (pb, progress_fn) = make_progress_callback("Discovering files...");

            println!("Processing files...");
            let archive_inputs = inputs.clone();
            let archive_output = output.clone();
            let result = run_with_codec_stack("openarc-create", move || {
                create_archive(
                    &archive_inputs,
                    &archive_output,
                    settings,
                    Some(progress_fn),
                )
            })?;

            pb.finish_with_message("Complete");

            if !no_zip && !result.staged_uncompressed_videos.is_empty() {
                let stage_root = result.video_staging_dir.as_deref().ok_or_else(|| {
                    anyhow!("videos were staged but the staging root was not returned")
                })?;
                let partial_output = partial_archive_path(&output);
                std::fs::rename(&output, &partial_output).with_context(|| {
                    format!(
                        "Failed to move incomplete archive {} to {}",
                        output.display(),
                        partial_output.display()
                    )
                })?;
                let merged = wait_for_external_video_encoding(
                    &partial_output,
                    stage_root,
                    result.staged_uncompressed_videos.len(),
                    compression_level,
                )?;
                std::fs::rename(&partial_output, &output).with_context(|| {
                    format!(
                        "Videos were merged, but the finalized archive could not be moved to {}. It remains at {}",
                        output.display(),
                        partial_output.display()
                    )
                })?;
                let _ = std::fs::remove_dir_all(stage_root);
                println!("Merged externally encoded videos: {}", merged);
            }

            println!();
            println!(
                "{} creation complete!",
                if no_zip { "Folder" } else { "Archive" }
            );
            println!("  Discovered: {} files", result.discovered_files.len());
            println!("  Processed: {} files", result.processed.len());
            println!("  Failed: {} files", result.failed.len());
            println!(
                "  Dropped JPEG+RAW companions: {} files",
                result.dropped_paired_jpegs.len()
            );
            println!(
                "  Skipped (catalog): {} files",
                result.skipped_by_catalog.len()
            );
            if result.dedup_groups > 0 {
                println!(
                    "  Duplicate content detected: {} groups (all paths preserved)",
                    result.dedup_groups
                );
            }
            if !result.failed.is_empty() {
                println!("  Failed file list:");
                for failed in &result.failed {
                    println!("    - {}", failed.original_path.display());
                }
            }

            let total_original: u64 = result.processed.iter().map(|p| p.original_size).sum();
            let total_compressed = artifact_size(&output)
                .unwrap_or_else(|| result.processed.iter().map(|p| p.output_size).sum());
            let ratio = if total_original > 0 {
                (total_compressed as f64 / total_original as f64) * 100.0
            } else {
                0.0
            };

            println!();
            println!("Compression statistics:");
            println!("  Original size: {} MB", total_original / 1_000_000);
            println!("  Compressed size: {} MB", total_compressed / 1_000_000);
            println!("  Ratio: {:.2}%", ratio);
            if result.jpeg2000_fallback.replaced_files > 0 {
                println!(
                    "  JPEG 2000 fallback: {} files encoded to JP2 q85 after BPG bitrate criteria flagged them and JP2 was smaller",
                    result.jpeg2000_fallback.replaced_files
                );
                println!(
                    "  JPEG 2000 average savings: {} KB/file ({:.2}% average across replaced files)",
                    result.jpeg2000_fallback.average_saved_bytes() / 1_000,
                    result.jpeg2000_fallback.average_saved_percent()
                );
            } else if result.jpeg2000_fallback.flagged_files > 0 {
                println!(
                    "  JPEG 2000 fallback: {} files flagged by BPG bitrate criteria, but BPG remained smaller",
                    result.jpeg2000_fallback.flagged_files
                );
            }

            println!();
            println!("Output: {}", output.display());
            if no_zip && !result.staged_uncompressed_videos.is_empty() {
                println!();
                println!(
                    "Staged uncompressed videos: {}",
                    result.staged_uncompressed_videos.len()
                );
                if let Some(stage_root) = result.video_staging_dir.as_deref() {
                    println!("Stage folder: {}", stage_root.display());
                }
                println!(
                    "Folder mode does not finalize an archive; encode these videos externally and place the results in the output media layout."
                );
            }

            Ok(())
        }

        Commands::Extract {
            input,
            output,
            no_reencode,
        } => {
            println!(
                "Extracting archive: {} to {}",
                input.display(),
                output.display()
            );

            let (pb, progress_fn) = make_progress_callback("Extracting archive...");

            let extract_input = input.clone();
            let extract_output = output.clone();
            let extraction = run_with_codec_stack("openarc-extract", move || {
                extract_archive_with_decoding(
                    &extract_input,
                    &extract_output,
                    ExtractionSettings {
                        decode_images: !no_reencode,
                        ..ExtractionSettings::default()
                    },
                    Some(progress_fn),
                )
            })?;
            pb.finish_with_message("Complete");

            println!();
            println!("Extraction complete!");
            println!("  Files extracted: {}", extraction.files_extracted);
            println!("  Total size: {} MB", extraction.total_size / 1_000_000);
            println!("  Decoded images: {}", extraction.decoded_files);
            println!("  Output: {}", output.display());
            Ok(())
        }

        Commands::List { archive } => {
            println!("Listing contents of: {}", archive.display());
            let files = list_archive_contents(&archive)?;
            if files.is_empty() {
                println!("Archive is empty.");
                return Ok(());
            }

            let mut total_original = 0u64;
            let mut total_compressed = 0u64;
            for f in &files {
                total_original += f.original_size;
                total_compressed += f.compressed_size;
            }

            println!("Entries: {}", files.len());
            println!(
                "{:<6} {:>10} {:>11}  {}",
                "Type", "Original", "Compressed", "Path"
            );
            for f in &files {
                let kind = match f.file_type {
                    1 => "IMG",
                    2 => "VID",
                    _ => "MISC",
                };
                println!(
                    "{:<6} {:>9}M {:>10}M  {}",
                    kind,
                    f.original_size / 1_000_000,
                    f.compressed_size / 1_000_000,
                    f.filename
                );
            }
            println!();
            println!(
                "Total: {}M -> {}M",
                total_original / 1_000_000,
                total_compressed / 1_000_000
            );
            Ok(())
        }

        Commands::ConvertBpg {
            input,
            output,
            effort,
            #[cfg(feature = "bpg-rs")]
            aq,
            quality,
        } => {
            let effort = BpgEffort::parse(&effort)?;
            #[cfg(feature = "bpg-rs")]
            let aq = BpgAq::parse(&aq)?;
            #[cfg(not(feature = "bpg-rs"))]
            let aq: BpgAq = openarc::bpg_wrapper::AQ_CLI_DEFAULT;
            aq.validate_for_effort(effort)?;
            let cfg = BpgConfig {
                effort,
                aq,
                bit_depth: 8,
                quality_override: quality,
            };
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{elapsed_precise}] {msg}")
                    .unwrap(),
            );
            pb.set_message(format!("BPG {}: {}", effort, input.display()));
            openarc::bpg_wrapper::encode_image_to_bpg(&input, &output, &cfg)?;
            pb.finish_with_message("Complete");
            println!("Output: {}", output.display());
            Ok(())
        }

        Commands::BatchBpg {
            input,
            output,
            effort,
            #[cfg(feature = "bpg-rs")]
            aq,
            quality,
        } => {
            let effort = BpgEffort::parse(&effort)?;
            #[cfg(feature = "bpg-rs")]
            let aq = BpgAq::parse(&aq)?;
            #[cfg(not(feature = "bpg-rs"))]
            let aq: BpgAq = openarc::bpg_wrapper::AQ_CLI_DEFAULT;
            aq.validate_for_effort(effort)?;
            std::fs::create_dir_all(&output).with_context(|| {
                format!("Failed to create output directory {}", output.display())
            })?;
            let images = find_images(&input)?;
            let pb = ProgressBar::new(images.len() as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            let cfg = BpgConfig {
                effort,
                aq,
                bit_depth: 8,
                quality_override: quality,
            };
            for (idx, path) in images.iter().enumerate() {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
                let out_path = output.join(format!("{}.bpg", stem));
                pb.set_position(idx as u64);
                pb.set_message(
                    path.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("image")
                        .to_string(),
                );
                openarc::bpg_wrapper::encode_image_to_bpg(path, &out_path, &cfg)
                    .with_context(|| format!("Failed to encode {}", path.display()))?;
            }
            pb.finish_with_message("Complete");
            println!("Encoded {} image(s) to {}", images.len(), output.display());
            Ok(())
        }

        // === Standalone LZMA2/Zstandard commands ===
        Commands::ArcCompress {
            input,
            output,
            method,
            level,
            dict_size,
        } => {
            use std::io::{Read, Write};

            println!("OpenArc: Compressing with LZMA2/Zstandard");
            println!("  Input: {:?}", input);
            println!("  Output: {}", output.display());
            println!(
                "  Method: {} (level: {}, dict: {} bytes)",
                method, level, dict_size
            );
            println!();

            let mut input_data = Vec::new();
            std::fs::File::open(&input[0])
                .with_context(|| format!("Failed to open input file: {}", input[0].display()))?
                .read_to_end(&mut input_data)?;

            let arc_method = match method.as_str() {
                "store" => Method::Store,
                "zstd" => Method::Zstd(arcmax::codec::zstd::ZstdOptions { level }),
                _ => Method::Lzma(LzmaOptions {
                    lzma2: true,
                    dict_size,
                    level: Some(level.clamp(1, 9) as u8),
                    ..Default::default()
                }),
            };

            let compressed = compress_with(
                &input_data,
                CompressionOptions::default().with_method(arc_method),
            )
            .context("Compression failed")?;

            std::fs::File::create(&output)
                .with_context(|| format!("Failed to create output file: {}", output.display()))?
                .write_all(&compressed)?;

            let ratio = if !input_data.is_empty() {
                (compressed.len() as f64 / input_data.len() as f64) * 100.0
            } else {
                0.0
            };

            println!("Compression complete!");
            println!("  Original: {} bytes", input_data.len());
            println!("  Compressed: {} bytes", compressed.len());
            println!("  Ratio: {:.2}%", ratio);

            Ok(())
        }

        Commands::ArcExtract { archive, output } => {
            use std::io::{Read, Write};

            println!("OpenArc: Decompressing file");
            println!("  Archive: {}", archive.display());
            println!(
                "  Output: {}",
                output.as_ref().unwrap_or(&PathBuf::from(".")).display()
            );
            println!();

            let mut input_data = Vec::new();
            std::fs::File::open(&archive)
                .with_context(|| format!("Failed to open archive: {}", archive.display()))?
                .read_to_end(&mut input_data)?;

            let decompressed = decompress(&input_data).context("Decompression failed")?;

            let output_path = output.unwrap_or_else(|| PathBuf::from("output.bin"));
            std::fs::File::create(&output_path)
                .with_context(|| {
                    format!("Failed to create output file: {}", output_path.display())
                })?
                .write_all(&decompressed)?;

            println!("Extraction complete!");
            println!("  Decompressed: {} bytes", decompressed.len());
            println!("  Output: {}", output_path.display());

            Ok(())
        }

        Commands::ArcTest { data, method } => {
            println!("OpenArc: Testing compression");
            println!("  Data: {:?}", data);
            println!("  Method: {}", method);
            println!();

            let test_data = data.as_bytes();

            let arc_method = match method.as_str() {
                "store" => Method::Store,
                "zstd" => Method::Zstd(arcmax::codec::zstd::ZstdOptions { level: 3 }),
                _ => Method::Lzma(LzmaOptions {
                    lzma2: true,
                    dict_size: 128 * 1024 * 1024,
                    level: Some(5),
                    ..Default::default()
                }),
            };

            let compressed = compress_with(
                test_data,
                CompressionOptions::default().with_method(arc_method),
            )
            .context("Compression test failed")?;

            println!("Compression test:");
            println!("  Original: {} bytes", test_data.len());
            println!("  Compressed: {} bytes", compressed.len());
            println!(
                "  Ratio: {:.2}%",
                (compressed.len() as f64 / test_data.len() as f64) * 100.0
            );

            let decompressed = decompress(&compressed).context("Decompression test failed")?;

            if test_data == decompressed.as_slice() {
                println!("  Round-trip: OK");
            } else {
                anyhow::bail!("Round-trip verification failed - data mismatch!");
            }

            Ok(())
        }

        Commands::PhoneDetect => {
            let phones = phone_backup::detect_phones()?;
            if phones.is_empty() {
                println!("No phone devices detected.");
                return Ok(());
            }

            println!("Detected {} phone device(s):", phones.len());
            for phone in phones {
                let source = match phone.source_kind {
                    phone_backup::PhoneSourceKind::Mtp => "MTP",
                    phone_backup::PhoneSourceKind::MountedFilesystem => "mounted filesystem",
                };
                println!("  - {} [{}] ({})", phone.display_name, source, phone.id);
            }
            Ok(())
        }
    }
}
