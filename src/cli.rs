//! Command-line interface for OpenArc

use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
        
        /// BPG effort preset (balanced, good, best)
        #[arg(long, default_value = "good", value_parser = ["balanced", "good", "best"])]
        bpg_effort: String,

        /// Advanced/testing override for BPG QP (0-51, lower = better quality).
        /// Normal use should prefer --bpg-effort.
        #[arg(long, hide = true)]
        bpg_quality: Option<i32>,

        /// Enable lossless BPG compression
        #[arg(long)]
        bpg_lossless: bool,

        /// ZSTD level (1-22) for the final archive container. The container
        /// wraps already-compressed BPG/video/LZMA2 data, so a low value
        /// (1-6) is recommended; higher levels mostly waste CPU.
        #[arg(long, default_value = "3")]
        compression_level: i32,

        /// LZMA2 level (1-9) for misc.arc, the bundle of small/likely
        /// uncompressible misc files (documents, configs, etc.)
        #[arg(long, default_value = "6")]
        misc_compression_level: i32,

        /// Disable catalog (incremental backup tracking)
        #[arg(long)]
        no_catalog: bool,
        
        /// Disable deduplication
        #[arg(long)]
        no_dedup: bool,
        
        /// Disable file-level tracking
        #[arg(long)]
        no_tracking: bool,

        /// Archive media as-is (disable image/video re-encoding)
        #[arg(long)]
        no_reencode: bool,

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
        
        /// BPG effort preset (balanced, good, best)
        #[arg(long, default_value = "good", value_parser = ["balanced", "good", "best"])]
        effort: String,

        /// Advanced/testing override for BPG QP (0-51, lower = better quality)
        #[arg(short, long, hide = true)]
        quality: Option<u8>,
        
        /// Enable lossless compression
        #[arg(long)]
        lossless: bool,
    },
    
    /// Batch convert images to BPG
    BatchBpg {
        /// Input directory
        input: PathBuf,
        
        /// Output directory
        #[arg(short, long)]
        output: PathBuf,
        
        /// BPG effort preset (balanced, good, best)
        #[arg(long, default_value = "good", value_parser = ["balanced", "good", "best"])]
        effort: String,

        /// Advanced/testing override for BPG QP (0-51, lower = better quality)
        #[arg(short, long, hide = true)]
        quality: Option<u8>,
        
        /// Enable lossless compression
        #[arg(long)]
        lossless: bool,
    },
    
    // === ArcMax Compression Commands ===

    /// Compress a file with LZMA2 or Zstd via arcmax
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

    /// Decompress a file compressed with arcmax
    ArcExtract {
        /// File to decompress
        #[arg(required = true)]
        archive: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Password (reserved, currently unused)
        #[arg(short, long)]
        password: Option<String>,
    },

    /// Test LZMA2/Zstd round-trip via arcmax
    ArcTest {
        /// Test data string
        #[arg(short, long, default_value = "Hello, World! This is a compression test.")]
        data: String,

        /// Compression method to test (lzma2, zstd, store)
        #[arg(short, long, default_value = "lzma2")]
        method: String,
    },

    /// Detect connected phone devices (MTP on Windows, mounted media on Linux)
    PhoneDetect,
}
