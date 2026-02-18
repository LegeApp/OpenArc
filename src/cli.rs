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
        
        /// BPG quality (0-51, lower = better quality, higher compression)
        #[arg(long, default_value = "25")]
        bpg_quality: i32,
        
        /// Enable lossless BPG compression
        #[arg(long)]
        bpg_lossless: bool,
        
        /// Video preset: 0=H264/Medium, 1=H265/Medium, 2=H264/Fast, 3=H265/Slow
        #[arg(long, default_value = "0")]
        video_preset: i32,
        
        /// Video CRF quality (lower = better, typical: 18-28)
        #[arg(long, default_value = "23")]
        video_crf: i32,
        
        /// ZSTD compression level (1-22, higher = better compression)
        #[arg(long, default_value = "3")]
        compression_level: i32,
        
        /// Disable catalog (incremental backup tracking)
        #[arg(long)]
        no_catalog: bool,
        
        /// Disable deduplication
        #[arg(long)]
        no_dedup: bool,
        
        /// Don't skip already compressed videos
        #[arg(long)]
        no_skip_compressed: bool,

        /// Disable file-level tracking
        #[arg(long)]
        no_tracking: bool,
    },
    
    /// Extract an archive
    Extract {
        /// Input archive file
        #[arg(short, long)]
        input: PathBuf,
        
        /// Output directory
        #[arg(short, long)]
        output: PathBuf,
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
        
        /// Quality (0-51, lower = better)
        #[arg(short, long, default_value = "25")]
        quality: u8,
        
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
        
        /// Quality (0-51, lower = better)
        #[arg(short, long, default_value = "25")]
        quality: u8,
        
        /// Enable lossless compression
        #[arg(long)]
        lossless: bool,
    },
    
    /// Convert video to H.264/H.265
    ConvertVideo {
        /// Input video file
        input: PathBuf,
        
        /// Output video file
        #[arg(short, long)]
        output: PathBuf,
        
        /// Video codec (h264, h265)
        #[arg(long, default_value = "h264")]
        codec: String,
        
        /// Speed preset (fast, medium, slow)
        #[arg(long, default_value = "medium")]
        preset: String,
        
        /// Quality (CRF, lower = better, typical: 18-28)
        #[arg(long, default_value = "23")]
        quality: u8,
        
        /// Copy audio stream
        #[arg(long)]
        copy_audio: bool,
    },

    // === ArcMax Compression Commands ===
    // These commands provide direct access to FreeARC compression algorithms

    /// Compress files with FreeARC algorithms (arcmax)
    ArcCompress {
        /// Input files to compress
        #[arg(required = true)]
        input: Vec<PathBuf>,

        /// Output archive file
        #[arg(short, long)]
        output: PathBuf,

        /// Compression method (store, lzma2, ppmd, tornado, etc.)
        #[arg(short, long, default_value = "lzma2")]
        method: String,

        /// Compression level (1-9)
        #[arg(short, long, default_value = "5")]
        level: i32,

        /// Dictionary size in bytes
        #[arg(short, long, default_value = "33554432")]
        dict_size: u32,
    },

    /// Extract FreeARC archive (arcmax)
    ArcExtract {
        /// Archive file to extract
        #[arg(required = true)]
        archive: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Password for encrypted archives
        #[arg(short, long)]
        password: Option<String>,
    },

    /// Test FreeARC compression (arcmax)
    ArcTest {
        /// Test data to compress and decompress
        #[arg(short, long, default_value = "Hello, World! This is a test of FreeARC compression.")]
        data: String,

        /// Compression method to test
        #[arg(short, long, default_value = "lzma2")]
        method: String,
    },
}
