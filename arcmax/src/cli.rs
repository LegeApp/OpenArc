//! Simple CLI interface for FreeARC compression library

use std::io::{Read, Write};
use std::path::PathBuf;
use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};

use arcmax::{compress, decompress, CompressionMethod, compression_ratio};

#[derive(Parser, Debug)]
#[command(name = "arcmax")]
#[command(about = "FreeARC Compression Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Compress files
    Compress(CompressArgs),
    /// Extract archive
    Extract(ExtractArgs),
    /// Test compression
    Test(TestArgs),
}

#[derive(Parser, Debug)]
pub struct CompressArgs {
    /// Input files to compress
    #[arg(required = true)]
    input: Vec<PathBuf>,
    
    /// Output archive file
    #[arg(short, long)]
    output: PathBuf,
    
    /// Compression method
    #[arg(short, long, default_value = "store")]
    method: String,
    
    /// Compression level (1-9)
    #[arg(short, long, default_value = "5")]
    level: i32,
    
    /// Dictionary size in bytes
    #[arg(short, long, default_value = "33554432")]
    dict_size: u32,
}

#[derive(Parser, Debug)]
pub struct ExtractArgs {
    /// Archive file to extract
    #[arg(required = true)]
    archive: PathBuf,
    
    /// Output directory
    #[arg(short, long)]
    output: Option<PathBuf>,
    
    /// Password for encrypted archives
    #[arg(short, long)]
    password: Option<String>,
}

#[derive(Parser, Debug)]
pub struct TestArgs {
    /// Test data to compress and decompress
    #[arg(short, long, default_value = "Hello, World! This is a test of FreeARC library.")]
    data: String,
}

pub fn dispatch() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compress(args) => cmd_compress(args),
        Commands::Extract(args) => cmd_extract(args),
        Commands::Test(args) => cmd_test(args),
    }
}

fn cmd_compress(args: CompressArgs) -> Result<()> {
    // Read input file
    let mut input_data = Vec::new();
    let mut input_file = std::fs::File::open(&args.input[0])?;
    input_file.read_to_end(&mut input_data)?;
    
    // Parse compression method
    let method = match args.method.as_str() {
        "store" => CompressionMethod::Store,
        "lzma2" => CompressionMethod::Lzma2 { level: args.level, dict_size: args.dict_size },
        _ => return Err(anyhow!("Unknown compression method: {}", args.method)),
    };
    
    // Compress
    println!("Compressing {} -> {}", args.input[0].display(), args.output.display());
    let compressed = compress(&input_data, method)?;
    
    // Write output
    let mut output_file = std::fs::File::create(&args.output)?;
    output_file.write_all(&compressed)?;
    
    println!("Compression complete!");
    Ok(())
}

fn cmd_extract(args: ExtractArgs) -> Result<()> {
    // Read input file
    let mut input_data = Vec::new();
    let mut input_file = std::fs::File::open(&args.archive)?;
    input_file.read_to_end(&mut input_data)?;
    
    // Decompress
    println!("Decompressing {} -> {}", args.archive.display(), 
        args.output.as_ref().unwrap_or(&std::path::PathBuf::from(".")).display());
    let decompressed = decompress(&input_data)?;
    
    // Write output
    let output_path = args.output.unwrap_or_else(|| std::path::PathBuf::from("output.txt"));
    let mut output_file = std::fs::File::create(&output_path)?;
    output_file.write_all(&decompressed)?;
    
    println!("Decompression complete!");
    Ok(())
}

fn cmd_test(args: TestArgs) -> Result<()> {
    let data = args.data.as_bytes();
    
    // Test compression
    let compressed = compress(data, CompressionMethod::Store)?;
    println!("Original: {} bytes", data.len());
    println!("Compressed: {} bytes", compressed.len());
    println!("Ratio: {:.2}%", compression_ratio(data.len(), compressed.len()) * 100.0);
    
    // Test decompression
    let decompressed = decompress(&compressed)?;
    assert_eq!(data, &decompressed);
    println!("Round-trip successful!");
    
    Ok(())
}
