//! OpenArc - Media archiver for phone/camera files

use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use openarc_core::orchestrator::{create_archive, OrchestratorSettings};
use std::sync::Arc;

mod cli;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
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
        } => {
            println!("OpenArc - Creating archive: {}", output.display());
            println!("Input sources: {} items", inputs.len());
            println!();

            let settings = OrchestratorSettings {
                bpg_quality,
                bpg_lossless,
                bpg_bit_depth: 8,
                bpg_chroma_format: 1,
                bpg_encoder_type: 0,
                bpg_compression_level: 8,
                video_preset,
                video_crf,
                compression_level,
                enable_catalog: !no_catalog,
                enable_dedup: !no_dedup,
                skip_already_compressed_videos: !no_skip_compressed,
                staging_dir: None,
                heic_quality: 90,
                jpeg_quality: 92,
            };

            println!("Settings:");
            println!("  BPG quality: {} (lossless: {})", bpg_quality, bpg_lossless);
            println!("  Video preset: {} (CRF: {})", video_preset, video_crf);
            println!("  ZSTD level: {}", compression_level);
            println!("  Catalog: {}", !no_catalog);
            println!("  Deduplication: {}", !no_dedup);
            println!("  Skip compressed videos: {}", !no_skip_compressed);
            println!();

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

            println!("Processing files...");
            let result = create_archive(&inputs, &output, settings, Some(progress_fn))?;

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
            println!();
            println!("Output: {}", output.display());

            Ok(())
        }

        Commands::Extract { input, output } => {
            println!("Extracting archive: {} to {}", input.display(), output.display());
            println!("Note: Extraction not yet implemented in alpha version");
            Ok(())
        }

        Commands::List { archive } => {
            println!("Listing contents of: {}", archive.display());
            println!("Note: Listing not yet implemented in alpha version");
            Ok(())
        }

        Commands::ConvertBpg { .. } | Commands::BatchBpg { .. } | Commands::ConvertVideo { .. } => {
            println!("Note: Individual conversion commands are available for testing.");
            println!("For full archiving, use the 'create' command.");
            Ok(())
        }
    }
}
