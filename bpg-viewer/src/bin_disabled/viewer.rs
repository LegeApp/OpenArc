// BPG Viewer CLI Application
use std::path::PathBuf;
use anyhow::Result;
use bpg_viewer::{decode_file, ffi};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <bpg-file>", args[0]);
        eprintln!("\nBPG Viewer - Display BPG images");
        eprintln!("Version: {}", ffi::version_string());
        eprintln!("\nOptions:");
        eprintln!("  -i, --info    Show image information only");
        eprintln!("  -h, --help    Show this help message");
        std::process::exit(1);
    }

    let mut info_only = false;
    let mut file_path: Option<PathBuf> = None;

    for arg in &args[1..] {
        match arg.as_str() {
            "-i" | "--info" => info_only = true,
            "-h" | "--help" => {
                println!("BPG Viewer - Display BPG images");
                println!("Version: {}", ffi::version_string());
                println!("\nUsage: {} [OPTIONS] <file.bpg>", args[0]);
                println!("\nOptions:");
                println!("  -i, --info    Show image information only");
                println!("  -h, --help    Show this help message");
                return Ok(());
            }
            _ => {
                if !arg.starts_with('-') {
                    file_path = Some(PathBuf::from(arg));
                }
            }
        }
    }

    let file_path = file_path.ok_or_else(|| anyhow::anyhow!("No input file specified"))?;

    if !file_path.exists() {
        eprintln!("Error: File not found: {}", file_path.display());
        std::process::exit(1);
    }

    // Decode the BPG file
    println!("Decoding BPG file: {}", file_path.display());
    let decoded = decode_file(file_path.to_str().unwrap())?;

    // Display information
    println!("\nImage Information:");
    println!("  Dimensions: {}x{}", decoded.width, decoded.height);
    println!("  Format: {:?}", decoded.format);
    println!("  Data size: {} bytes", decoded.data.len());
    println!("  Bytes per pixel: {}", decoded.bytes_per_pixel());

    if info_only {
        return Ok(());
    }

    // For now, just save as PNG for viewing
    let output_path = file_path.with_extension("png");
    println!("\nConverting to PNG for viewing: {}", output_path.display());

    let rgba_data = decoded.to_rgba32()?;
    let img = image::ImageBuffer::from_raw(decoded.width, decoded.height, rgba_data)
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

    img.save(&output_path)?;
    println!("Saved to: {}", output_path.display());

    Ok(())
}
