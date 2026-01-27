// BPG Thumbnail Generator CLI Application
use std::path::PathBuf;
use anyhow::Result;
use bpg_viewer::{ThumbnailGenerator, ThumbnailConfig, ffi};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    let mut config = ThumbnailConfig::default();
    let mut input_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "-w" | "--width" => {
                i += 1;
                if i < args.len() {
                    config.max_width = args[i].parse()?;
                }
            }
            "-h" | "--height" => {
                i += 1;
                if i < args.len() {
                    config.max_height = args[i].parse()?;
                }
            }
            "-q" | "--quality" => {
                i += 1;
                if i < args.len() {
                    config.quality = args[i].parse()?;
                }
            }
            "-o" | "--output" => {
                i += 1;
                if i < args.len() {
                    output_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--help" => {
                print_usage(&args[0]);
                return Ok(());
            }
            _ => {
                if !args[i].starts_with('-') && input_path.is_none() {
                    input_path = Some(PathBuf::from(&args[i]));
                }
            }
        }
        i += 1;
    }

    let input_path = input_path.ok_or_else(|| anyhow::anyhow!("No input file specified"))?;

    if !input_path.exists() {
        eprintln!("Error: File not found: {}", input_path.display());
        std::process::exit(1);
    }

    let output_path = output_path.unwrap_or_else(|| {
        let mut p = input_path.clone();
        p.set_extension("thumb.png");
        p
    });

    println!("BPG Thumbnail Generator");
    println!("Version: {}", ffi::version_string());
    println!("\nInput: {}", input_path.display());
    println!("Output: {}", output_path.display());
    println!("Max dimensions: {}x{}", config.max_width, config.max_height);
    println!("Quality: {}", config.quality);

    let generator = ThumbnailGenerator::with_config(config);

    println!("\nGenerating thumbnail...");
    generator.generate_thumbnail_to_png(&input_path, &output_path)?;

    println!("Thumbnail saved successfully!");

    Ok(())
}

fn print_usage(program: &str) {
    println!("BPG Thumbnail Generator");
    println!("Version: {}", ffi::version_string());
    println!("\nUsage: {} [OPTIONS] <input.bpg>", program);
    println!("\nOptions:");
    println!("  -w, --width <pixels>     Maximum width (default: 256)");
    println!("  -h, --height <pixels>    Maximum height (default: 256)");
    println!("  -q, --quality <0-51>     BPG quality (default: 28, lower is better)");
    println!("  -o, --output <file>      Output file path (default: input.thumb.png)");
    println!("  --help                   Show this help message");
    println!("\nExamples:");
    println!("  {} image.bpg", program);
    println!("  {} -w 512 -h 512 image.bpg -o thumb.png", program);
}
