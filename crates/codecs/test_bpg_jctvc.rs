// Test BPG subprocess encoder

use std::path::PathBuf;

mod bpg_subprocess;
use bpg_subprocess::{BpgEncoder, BpgConfig, BpgEncoderType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("BPG JCTVC Encoder Test");
    println!("======================\n");
    
    // Path to encoder
    let encoder_path = PathBuf::from(r"D:\misc\arc\openarc\codecs\bpg\bpgenc-jctvc.exe");
    
    if !encoder_path.exists() {
        eprintln!("❌ Encoder not found: {}", encoder_path.display());
        eprintln!("Please run the build script first to copy the encoder.");
        return Ok(());
    }
    
    println!("✅ Found encoder: {}", encoder_path.display());
    
    // Check supported encoders
    println!("\nChecking supported encoders...");
    match BpgEncoder::get_supported_encoders(&encoder_path) {
        Ok(encoders) => {
            println!("✅ Supported encoders: {:?}", encoders);
        }
        Err(e) => {
            println!("⚠️  Could not detect encoders: {}", e);
        }
    }
    
    // Test encoding (if test image exists)
    let test_image = PathBuf::from(r"D:\misc\arc\openarc\codecs\test.jpg");
    if test_image.exists() {
        println!("\n✅ Found test image: {}", test_image.display());
        
        // Test x265 encoding
        println!("\nTesting x265 encoder...");
        let output_x265 = test_image.with_file_name("test_x265.bpg");
        let config_x265 = BpgConfig {
            quality: 28,
            encoder_type: BpgEncoderType::X265,
            ..Default::default()
        };
        
        let encoder_x265 = BpgEncoder::new(&encoder_path, config_x265)?;
        match encoder_x265.encode_file(&test_image, &output_x265) {
            Ok(_) => {
                let size = std::fs::metadata(&output_x265)?.len();
                println!("✅ x265 encoded: {} ({} bytes)", output_x265.display(), size);
            }
            Err(e) => {
                println!("❌ x265 encoding failed: {}", e);
            }
        }
        
        // Test JCTVC encoding
        println!("\nTesting JCTVC encoder...");
        let output_jctvc = test_image.with_file_name("test_jctvc.bpg");
        let config_jctvc = BpgConfig {
            quality: 28,
            encoder_type: BpgEncoderType::Jctvc,
            ..Default::default()
        };
        
        let encoder_jctvc = BpgEncoder::new(&encoder_path, config_jctvc)?;
        match encoder_jctvc.encode_file(&test_image, &output_jctvc) {
            Ok(_) => {
                let size = std::fs::metadata(&output_jctvc)?.len();
                println!("✅ JCTVC encoded: {} ({} bytes)", output_jctvc.display(), size);
            }
            Err(e) => {
                println!("❌ JCTVC encoding failed: {}", e);
            }
        }
        
        // Compare sizes
        if output_x265.exists() && output_jctvc.exists() {
            let size_x265 = std::fs::metadata(&output_x265)?.len();
            let size_jctvc = std::fs::metadata(&output_jctvc)?.len();
            let improvement = ((size_x265 as f64 - size_jctvc as f64) / size_x265 as f64) * 100.0;
            println!("\n📊 Compression comparison:");
            println!("   x265:  {} bytes", size_x265);
            println!("   JCTVC: {} bytes", size_jctvc);
            println!("   Improvement: {:.1}%", improvement);
        }
    } else {
        println!("\n⚠️  No test image found at: {}", test_image.display());
        println!("Skipping encoding test.");
    }
    
    println!("\n✅ All tests completed!");
    Ok(())
}
