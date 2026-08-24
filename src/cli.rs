//! Command-line interface for OpenArc

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Value parser for the JPEG XL effort/quality preset.
fn jxl_effort_parser() -> clap::builder::PossibleValuesParser {
    clap::builder::PossibleValuesParser::new(crate::jxl_wrapper::EFFORT_CLI_VALUES.iter().copied())
}

#[derive(Parser)]
#[command(name = "openarc")]
#[command(about = "OpenArc - Media archiver for phone/camera files", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Interactive wizard mode (drag-and-drop, easy setup)
    Interactive,

    /// Create a new archive from files or directories
    Create {
        /// Output archive file (.oarc or .tar.zst)
        #[arg(short, long)]
        output: PathBuf,

        /// Input files or directories (can be specified multiple times)
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// JPEG XL preset: best (production default), fast, or lossless.
        #[arg(long, default_value = crate::jxl_wrapper::EFFORT_CLI_DEFAULT, value_parser = jxl_effort_parser())]
        jxl_effort: String,

        /// Override the preset's bits-per-pixel target (higher = closer to the
        /// source and larger). JPEG XL rate control targets a size, not a
        /// perceptual distance, so this is the quality dial. Not valid with
        /// `--jxl-effort lossless`.
        #[arg(long)]
        jxl_bpp: Option<f64>,

        /// ZSTD level (1-22) for the final archive container. The container
        /// wraps already-compressed JPEG XL/video/LZMA2 data, so a low value
        /// (1-6) is recommended; higher levels mostly waste CPU.
        #[arg(long, default_value = "3")]
        compression_level: i32,

        /// LZMA2 level (1-9) for misc.arc, the bundle of small/compressible
        /// miscellaneous files (documents, configs, etc.)
        #[arg(long, default_value = "6")]
        misc_compression_level: i32,

        /// Disable the persistent archive-history catalog
        #[arg(long)]
        no_catalog: bool,

        /// Disable within-job duplicate-content detection (all paths are preserved)
        #[arg(long)]
        no_dedup: bool,

        /// Disable file-level tracking
        #[arg(long)]
        no_tracking: bool,

        /// Archive media as-is (disable image/video re-encoding)
        #[arg(long)]
        no_reencode: bool,

        /// Write the encoded OpenArc folder layout without creating the final archive
        #[arg(long)]
        no_zip: bool,
    },

    /// Extract an archive
    Extract {
        /// Input archive file
        #[arg(short, long)]
        input: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: PathBuf,

        /// Do not decode images back from archived formats on extraction
        #[arg(long)]
        no_reencode: bool,
    },

    /// List archive contents
    List {
        /// Archive file
        archive: PathBuf,
    },

    /// Convert a single image to JPEG XL
    #[command(name = "convert-jxl", alias = "convert-bpg")]
    ConvertJxl {
        /// Input image file
        input: PathBuf,

        /// Output .jxl file
        #[arg(short, long)]
        output: PathBuf,

        /// JPEG XL preset: best (production default), fast, or lossless.
        #[arg(long, default_value = crate::jxl_wrapper::EFFORT_CLI_DEFAULT, value_parser = jxl_effort_parser())]
        effort: String,

        /// Override the preset's bits-per-pixel target. Not valid with
        /// `--effort lossless`.
        #[arg(long)]
        bpp: Option<f64>,
    },

    /// Batch convert images to JPEG XL
    #[command(name = "batch-jxl", alias = "batch-bpg")]
    BatchJxl {
        /// Input directory
        input: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: PathBuf,

        /// JPEG XL preset: best (production default), fast, or lossless.
        #[arg(long, default_value = crate::jxl_wrapper::EFFORT_CLI_DEFAULT, value_parser = jxl_effort_parser())]
        effort: String,

        /// Override the preset's bits-per-pixel target. Not valid with
        /// `--effort lossless`.
        #[arg(long)]
        bpp: Option<f64>,
    },

    // === Standalone LZMA2/Zstandard Compression Commands ===
    /// Compress files with LZMA2 or Zstandard
    #[command(name = "compress", alias = "arc-compress")]
    ArcCompress {
        /// Input files to compress
        #[arg(required = true)]
        input: Vec<PathBuf>,

        /// Output file
        #[arg(short, long)]
        output: PathBuf,

        /// Compression method (lzma2, zstd, store)
        #[arg(short, long, default_value = "lzma2")]
        method: String,

        /// Compression level (1-9 for LZMA2, 1-22 for Zstd)
        #[arg(short, long, default_value = "5")]
        level: i32,

        /// Dictionary size in bytes (LZMA2 only)
        #[arg(short, long, default_value = "134217728")]
        dict_size: u32,
    },

    /// Decompress a file produced by `openarc compress`
    #[command(name = "decompress", alias = "arc-extract")]
    ArcExtract {
        /// File to decompress
        #[arg(required = true)]
        archive: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Test an LZMA2/Zstandard round trip
    #[command(name = "compression-test", alias = "arc-test")]
    ArcTest {
        /// Test data string
        #[arg(
            short,
            long,
            default_value = "Hello, World! This is a compression test."
        )]
        data: String,

        /// Compression method to test (lzma2, zstd, store)
        #[arg(short, long, default_value = "lzma2")]
        method: String,
    },

    /// Detect connected phone devices (MTP on Windows, mounted media on Linux)
    PhoneDetect,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_defaults_to_the_best_jpeg_xl_preset() {
        let cli = Cli::try_parse_from(["openarc", "create", "-o", "out.oarc", "input"])
            .expect("CLI should parse");
        match cli.command {
            Commands::Create {
                jxl_effort, jxl_bpp, ..
            } => {
                assert_eq!(jxl_effort, "best");
                assert_eq!(jxl_bpp, None, "the preset supplies the rate unless overridden");
            }
            _ => panic!("expected create command"),
        }
    }

    #[test]
    fn lossless_is_a_selectable_preset() {
        let cli = Cli::try_parse_from([
            "openarc", "create", "-o", "out.oarc", "input", "--jxl-effort", "lossless",
        ])
        .expect("CLI should parse");
        match cli.command {
            Commands::Create { jxl_effort, .. } => assert_eq!(jxl_effort, "lossless"),
            _ => panic!("expected create command"),
        }
    }

    #[test]
    fn an_unknown_preset_is_refused() {
        assert!(Cli::try_parse_from([
            "openarc",
            "create",
            "-o",
            "out.oarc",
            "input",
            "--jxl-effort",
            "placebo",
        ])
        .is_err());
    }

    #[test]
    fn the_internal_balanced_controller_is_not_a_public_preset() {
        assert!(Cli::try_parse_from([
            "openarc",
            "create",
            "-o",
            "out.oarc",
            "input",
            "--jxl-effort",
            "balanced",
        ])
        .is_err());
    }

    /// The old subcommand names still work, so existing scripts do not break
    /// on the rename.
    #[test]
    fn the_legacy_bpg_subcommand_names_are_still_accepted() {
        let cli = Cli::try_parse_from(["openarc", "convert-bpg", "in.png", "-o", "out.jxl"])
            .expect("the alias should parse");
        assert!(matches!(cli.command, Commands::ConvertJxl { .. }));

        let cli = Cli::try_parse_from(["openarc", "batch-bpg", "in", "-o", "out"])
            .expect("the alias should parse");
        assert!(matches!(cli.command, Commands::BatchJxl { .. }));
    }
}
