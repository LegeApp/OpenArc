//! OpenArc - Media archiver for phone/camera files

use anyhow::{anyhow, Result, Context};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use openarc::orchestrator::{
    create_archive, extract_archive_with_decoding, list_archive_contents, ExtractionSettings,
    OrchestratorSettings,
};
use openarc::cli::{Cli, Commands};
use openarc::interactive;
use openarc::phone_backup;
use std::path::PathBuf;
use std::sync::Arc;
use openarc::orchestrator::FileClass;

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

fn main() -> Result<()> {
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
            bpg_quality,
            bpg_lossless,
            video_preset,
            video_crf,
            compression_level,
            no_catalog,
            no_dedup,
            no_skip_compressed,
            no_tracking,
            no_reencode,
            bpg_compress_level,
        } => {
            println!("OpenArc - Creating archive: {}", output.display());
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

            let settings = OrchestratorSettings {
                bpg_quality,
                bpg_lossless,
                bpg_bit_depth: 8,
                bpg_chroma_format: 1,
                bpg_encoder_type: 0,
                bpg_compression_level: bpg_compress_level,
                video_preset,
                video_crf,
                compression_level,
                enable_catalog: !no_catalog,
                catalog_db_path: None,
                enable_dedup: !no_dedup,
                skip_already_compressed_videos: !no_skip_compressed,
                staging_dir: None,
                heic_quality: 90,
                jpeg_quality: 92,
                enable_tracking: !no_tracking,
                reencode_media: !no_reencode,
            };

            println!("Settings:");
            println!("  BPG quality: {} (lossless: {}, compress-level: {})", bpg_quality, bpg_lossless, bpg_compress_level);
            println!("  Video preset: {} (CRF: {})", video_preset, video_crf);
            println!("  ZSTD level: {}", compression_level);
            println!("  Catalog: {}", !no_catalog);
            println!("  Deduplication: {}", !no_dedup);
            println!("  Skip compressed videos: {}", !no_skip_compressed);
            println!("  File tracking: {}", !no_tracking);
            println!("  Re-encode media: {}", !no_reencode);
            println!();

            // Start with a spinner (length unknown) — switches to a real bar
            // once the orchestrator calls back with the actual file count.
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{elapsed_precise}] {msg}")
                    .unwrap(),
            );
            pb.set_message("Discovering files…");

            let pb_clone = pb.clone();
            let bar_style = ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-");
            let bar_style = Arc::new(bar_style);
            let progress_fn = Arc::new(move |current: usize, total: usize, msg: &str| {
                if total > 0 {
                    pb_clone.set_style((*bar_style).clone());
                    pb_clone.set_length(total as u64);
                }
                pb_clone.set_position(current as u64);
                pb_clone.set_message(msg.to_string());
            });

            println!("Processing files...");
            let archive_inputs = inputs.clone();
            let archive_output = output.clone();
            let result = run_with_codec_stack("openarc-create", move || {
                create_archive(&archive_inputs, &archive_output, settings, Some(progress_fn))
            })?;

            pb.finish_with_message("Complete");
            println!();
            println!("Archive creation complete!");
            println!("  Discovered: {} files", result.discovered_files.len());
            println!("  Processed: {} files", result.processed.len());
            println!("  Skipped (catalog): {} files", result.skipped_by_catalog.len());
            if result.dedup_groups > 0 {
                println!("  Dedup groups: {}", result.dedup_groups);
            }

            let total_original: u64 = result.processed.iter().map(|p| p.original_size).sum();
            let total_compressed: u64 = result.processed.iter().map(|p| p.output_size).sum();
            let raw_count = result.processed.iter().filter(|p| p.class == FileClass::Raw).count();
            let raw_total: u64 = result.processed
                .iter()
                .filter(|p| p.class == FileClass::Raw)
                .map(|p| p.original_size)
                .sum();
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
            if raw_count > 0 {
                println!(
                    "  RAW preserved separately: {} files, {} MB total (stored losslessly in raw.arc with FreeArc max level)",
                    raw_count,
                    raw_total / 1_000_000
                );
            }
            println!();
            println!("Output: {}", output.display());

            Ok(())
        }

        Commands::Extract { input, output, no_reencode } => {
            println!("Extracting archive: {} to {}", input.display(), output.display());

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

            let extract_input = input.clone();
            let extract_output = output.clone();
            let extraction = run_with_codec_stack("openarc-extract", move || {
                extract_archive_with_decoding(
                    &extract_input,
                    &extract_output,
                    OrchestratorSettings::default().compression_level,
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

        Commands::ConvertBpg { .. } | Commands::BatchBpg { .. } | Commands::ConvertVideo { .. } => {
            println!("Note: Individual conversion commands are available for testing.");
            println!("For full archiving, use the 'create' command.");
            Ok(())
        }

        // === ArcMax Commands ===
        Commands::ArcCompress { input, output, method, level, dict_size } => {
            use arcmax::{compress, CompressionMethod};
            use std::io::{Read, Write};

            println!("ArcMax: Compressing files with FreeARC");
            println!("  Input: {:?}", input);
            println!("  Output: {}", output.display());
            println!("  Method: {} (level: {}, dict: {} bytes)", method, level, dict_size);
            println!();

            // Read input file
            let mut input_data = Vec::new();
            let mut input_file = std::fs::File::open(&input[0])
                .with_context(|| format!("Failed to open input file: {}", input[0].display()))?;
            input_file.read_to_end(&mut input_data)?;

            // Parse compression method
            let compression_method = match method.as_str() {
                "store" => CompressionMethod::Store,
                "lzma2" => CompressionMethod::Lzma2 { level, dict_size },
                _ => anyhow::bail!("Unknown compression method: {}", method),
            };

            // Compress
            let compressed = compress(&input_data, compression_method)
                .context("Compression failed")?;

            // Write output
            let mut output_file = std::fs::File::create(&output)
                .with_context(|| format!("Failed to create output file: {}", output.display()))?;
            output_file.write_all(&compressed)?;

            let ratio = if input_data.len() > 0 {
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

        Commands::ArcExtract { archive, output, password } => {
            use arcmax::decompress;
            use std::io::{Read, Write};

            println!("ArcMax: Extracting FreeARC archive");
            println!("  Archive: {}", archive.display());
            println!("  Output: {}", output.as_ref().unwrap_or(&PathBuf::from(".")).display());
            if password.is_some() {
                println!("  Password: ***");
            }
            println!();

            // Read input file
            let mut input_data = Vec::new();
            let mut input_file = std::fs::File::open(&archive)
                .with_context(|| format!("Failed to open archive: {}", archive.display()))?;
            input_file.read_to_end(&mut input_data)?;

            // Decompress
            let decompressed = decompress(&input_data)
                .context("Decompression failed")?;

            // Write output
            let output_path = output.unwrap_or_else(|| PathBuf::from("output.bin"));
            let mut output_file = std::fs::File::create(&output_path)
                .with_context(|| format!("Failed to create output file: {}", output_path.display()))?;
            output_file.write_all(&decompressed)?;

            println!("Extraction complete!");
            println!("  Decompressed: {} bytes", decompressed.len());
            println!("  Output: {}", output_path.display());

            Ok(())
        }

        Commands::ArcTest { data, method } => {
            use arcmax::{compress, decompress, CompressionMethod, compression_ratio};

            println!("ArcMax: Testing FreeARC compression");
            println!("  Data: {:?}", data);
            println!("  Method: {}", method);
            println!();

            let test_data = data.as_bytes();

            // Parse compression method
            let compression_method = match method.as_str() {
                "store" => CompressionMethod::Store,
                "lzma2" => CompressionMethod::Lzma2 { level: 5, dict_size: 33554432 },
                _ => anyhow::bail!("Unknown compression method: {}", method),
            };

            // Test compression
            let compressed = compress(test_data, compression_method)
                .context("Compression test failed")?;

            println!("Compression test:");
            println!("  Original: {} bytes", test_data.len());
            println!("  Compressed: {} bytes", compressed.len());
            println!("  Ratio: {:.2}%", compression_ratio(test_data.len(), compressed.len()) * 100.0);

            // Test decompression
            let decompressed = decompress(&compressed)
                .context("Decompression test failed")?;

            if test_data == &decompressed {
                println!("  Round-trip: ✓ Successful!");
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
