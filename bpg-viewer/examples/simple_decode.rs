// Simple BPG decoding example
use bpg_viewer::decode_file;
use std::env;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <image.bpg>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];

    println!("Decoding BPG file: {}", path);

    let decoded = decode_file(path)?;

    println!("\nImage Information:");
    println!("  Width: {}", decoded.width);
    println!("  Height: {}", decoded.height);
    println!("  Format: {:?}", decoded.format);
    println!("  Data size: {} bytes", decoded.data.len());
    println!("  Bytes per pixel: {}", decoded.bytes_per_pixel());

    // Convert to RGBA if needed
    let rgba = decoded.to_rgba32()?;
    println!("  RGBA size: {} bytes", rgba.len());

    Ok(())
}
