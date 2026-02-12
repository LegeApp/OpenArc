// Test HEIC to BPG color conversion
use std::path::Path;
use anyhow::Result;

#[cfg(feature = "heif")]
fn test_heic_bpg_color() -> Result<()> {
    use codecs::heic::HeicCodec;
    use codecs::bpg::{NativeBPGEncoder, BPGEncoderConfig};
    
    println!("Testing HEIC to BPG color conversion...");
    
    // Test with a sample HEIC file if available
    let test_heic = Path::new("test.heic");
    if !test_heic.exists() {
        println!("No test.heic file found, skipping test");
        return Ok(());
    }
    
    // Decode HEIC to YCbCr
    let mut codec = HeicCodec::new()?;
    let decoded = codec.decode_file_ycbcr420(test_heic)?;
    
    println!("Decoded HEIC: {}x{}", decoded.width, decoded.height);
    println!("Y plane size: {}", decoded.y_plane.len());
    println!("Cb plane size: {}", decoded.cb_plane.len());
    println!("Cr plane size: {}", decoded.cr_plane.len());
    
    // Sample some YCbCr values to check
    if !decoded.y_plane.is_empty() && !decoded.cb_plane.is_empty() && !decoded.cr_plane.is_empty() {
        println!("Sample YCbCr values:");
        println!("  Y[0]: {}", decoded.y_plane[0]);
        println!("  Cb[0]: {}", decoded.cb_plane[0]);
        println!("  Cr[0]: {}", decoded.cr_plane[0]);
        
        // Check if Cb/Cr ranges look reasonable (should be around 128 for neutral colors)
        let cb_avg: u32 = decoded.cb_plane.iter().take(100).map(|&v| v as u32).sum();
        let cr_avg: u32 = decoded.cr_plane.iter().take(100).map(|&v| v as u32).sum();
        let cb_avg = cb_avg / 100;
        let cr_avg = cr_avg / 100;
        
        println!("  Cb average (first 100): {}", cb_avg);
        println!("  Cr average (first 100): {}", cr_avg);
        
        if cb_avg < 100 || cr_avg < 100 {
            println!("  ⚠️  Cb/Cr values seem low, might indicate color space issue");
        }
        if cb_avg > 150 || cr_avg > 150 {
            println!("  ⚠️  Cb/Cr values seem high, might indicate color space issue");
        }
    }
    
    // Encode to BPG
    let encoder = NativeBPGEncoder::new()?;
    let mut config = BPGEncoderConfig::default();
    config.quality = 28;
    config.chroma_format = 0; // YCbCr 4:2:0
    
    let bpg_data = encoder.encode_from_ycbcr420_planar(
        &decoded.y_plane,
        &decoded.cb_plane,
        &decoded.cr_plane,
        decoded.width,
        decoded.height,
        decoded.y_stride,
        decoded.cb_stride,
        decoded.cr_stride,
    )?;
    
    println!("Encoded to BPG: {} bytes", bpg_data.len());
    
    // Decode back and check
    match codecs::bpg::decode_file("test_output.bpg") {
        Ok((rgba_data, width, height, format)) => {
            println!("Decoded BPG: {}x{}, format: {:?}", width, height, format);
            
            // Sample some RGBA values
            if rgba_data.len() >= 12 {
                println!("Sample RGBA values:");
                println!("  Pixel 0: R={}, G={}, B={}, A={}", rgba_data[0], rgba_data[1], rgba_data[2], rgba_data[3]);
                println!("  Pixel 1: R={}, G={}, B={}, A={}", rgba_data[4], rgba_data[5], rgba_data[6], rgba_data[7]);
                println!("  Pixel 2: R={}, G={}, B={}, A={}", rgba_data[8], rgba_data[9], rgba_data[10], rgba_data[11]);
                
                // Check for green tint (G significantly higher than R and B)
                let r_avg = rgba_data[0] as f32;
                let g_avg = rgba_data[1] as f32;
                let b_avg = rgba_data[2] as f32;
                
                if g_avg > r_avg * 1.2 && g_avg > b_avg * 1.2 {
                    println!("  ⚠️  Green tint detected! G={:.1}, R={:.1}, B={:.1}", g_avg, r_avg, b_avg);
                    println!("  This suggests a YCbCr to RGB conversion issue in BPG decoder");
                }
            }
        }
        Err(e) => println!("Failed to decode BPG: {}", e),
    }
    
    Ok(())
}

fn main() -> Result<()> {
    #[cfg(feature = "heif")]
    {
        test_heic_bpg_color()?;
    }
    #[cfg(not(feature = "heif"))]
    {
        println!("HEIC support not compiled in");
    }
    
    Ok(())
}
