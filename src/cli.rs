//! Command-line interface for OpenArc

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Value parser for the BPG effort/compression option. The accepted values are
/// backend-specific (see `bpg_wrapper::EFFORT_CLI_VALUES`).
fn bpg_effort_parser() -> clap::builder::PossibleValuesParser {
    clap::builder::PossibleValuesParser::new(crate::bpg_wrapper::EFFORT_CLI_VALUES.iter().copied())
}

/// Value parser for the BPG adaptive-quantization option. Only compiled for the
/// pure-Rust backend; the C/x265 backend forces x265's default AQ and does not
/// expose a selector.
#[cfg(feature = "bpg-rs")]
fn bpg_aq_parser() -> clap::builder::PossibleValuesParser {
    clap::builder::PossibleValuesParser::new(crate::bpg_wrapper::AQ_CLI_VALUES.iter().copied())
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

        /// BPG compression effort: best (production default) or fast.
        #[arg(long, default_value = crate::bpg_wrapper::EFFORT_CLI_DEFAULT, value_parser = bpg_effort_parser())]
        bpg_effort: String,

        /// BPG adaptive quantization (Rust backend only). AQ is off by default.
        /// `--bpg-aq` enables the recommended two-pass mode; use
        /// `--bpg-aq=<mode>` to select another mode.
        #[cfg(feature = "bpg-rs")]
        #[arg(
            long,
            default_value = crate::bpg_wrapper::AQ_CLI_DEFAULT,
            default_missing_value = crate::bpg_wrapper::AQ_CLI_DEFAULT_WHEN_ENABLED,
            num_args = 0..=1,
            require_equals = true,
            value_parser = bpg_aq_parser()
        )]
        bpg_aq: String,

        /// Advanced/testing override for BPG QP (0-51, lower = better quality).
        /// Normal use should prefer --bpg-effort.
        #[arg(long, hide = true)]
        bpg_quality: Option<i32>,

        /// ZSTD level (1-22) for the final archive container. The container
        /// wraps already-compressed BPG/video/LZMA2 data, so a low value
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

    /// Convert single image to BPG
    ConvertBpg {
        /// Input image file
        input: PathBuf,

        /// Output BPG file
        #[arg(short, long)]
        output: PathBuf,

        /// BPG compression effort: best (production default) or fast.
        #[arg(long, default_value = crate::bpg_wrapper::EFFORT_CLI_DEFAULT, value_parser = bpg_effort_parser())]
        effort: String,

        /// BPG adaptive quantization (Rust backend only). AQ is off by default.
        /// `--aq` enables the recommended two-pass mode; use `--aq=<mode>` to
        /// select another mode.
        #[cfg(feature = "bpg-rs")]
        #[arg(
            long,
            default_value = crate::bpg_wrapper::AQ_CLI_DEFAULT,
            default_missing_value = crate::bpg_wrapper::AQ_CLI_DEFAULT_WHEN_ENABLED,
            num_args = 0..=1,
            require_equals = true,
            value_parser = bpg_aq_parser()
        )]
        aq: String,

        /// Advanced/testing override for BPG QP (0-51, lower = better quality)
        #[arg(short, long, hide = true)]
        quality: Option<u8>,
    },

    /// Batch convert images to BPG
    BatchBpg {
        /// Input directory
        input: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: PathBuf,

        /// BPG compression effort: best (production default) or fast.
        #[arg(long, default_value = crate::bpg_wrapper::EFFORT_CLI_DEFAULT, value_parser = bpg_effort_parser())]
        effort: String,

        /// BPG adaptive quantization (Rust backend only). AQ is off by default.
        /// `--aq` enables the recommended two-pass mode; use `--aq=<mode>` to
        /// select another mode.
        #[cfg(feature = "bpg-rs")]
        #[arg(
            long,
            default_value = crate::bpg_wrapper::AQ_CLI_DEFAULT,
            default_missing_value = crate::bpg_wrapper::AQ_CLI_DEFAULT_WHEN_ENABLED,
            num_args = 0..=1,
            require_equals = true,
            value_parser = bpg_aq_parser()
        )]
        aq: String,

        /// Advanced/testing override for BPG QP (0-51, lower = better quality)
        #[arg(short, long, hide = true)]
        quality: Option<u8>,
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

#[cfg(all(test, feature = "bpg-rs"))]
mod tests {
    use super::*;

    #[test]
    fn create_defaults_to_best_with_aq_off() {
        let cli = Cli::try_parse_from(["openarc", "create", "-o", "out.oarc", "input"])
            .expect("CLI should parse");
        match cli.command {
            Commands::Create {
                bpg_effort, bpg_aq, ..
            } => {
                assert_eq!(bpg_effort, "best");
                assert_eq!(bpg_aq, "off");
            }
            _ => panic!("expected create command"),
        }
    }

    #[test]
    fn bare_aq_flag_enables_two_pass() {
        let cli = Cli::try_parse_from(["openarc", "create", "-o", "out.oarc", "input", "--bpg-aq"])
            .expect("CLI should parse");
        match cli.command {
            Commands::Create { bpg_aq, .. } => assert_eq!(bpg_aq, "two-pass"),
            _ => panic!("expected create command"),
        }
    }

    #[test]
    fn placebo_is_not_a_public_cli_effort() {
        assert!(Cli::try_parse_from([
            "openarc",
            "create",
            "-o",
            "out.oarc",
            "input",
            "--bpg-effort",
            "placebo",
        ])
        .is_err());
    }
}
