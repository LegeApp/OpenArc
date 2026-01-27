//! Video compression analysis
//! 
//! Detects whether a video file is already efficiently compressed (e.g., by ffmpeg)
//! or is raw/lightly-compressed phone footage that would benefit from recompression.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Analysis result for a video file
#[derive(Debug, Clone)]
pub struct VideoAnalysis {
    /// Bitrate in kbps
    pub bitrate_kbps: f64,
    /// Video codec name (e.g., "h264", "hevc")
    pub codec: String,
    /// Duration in seconds
    pub duration_secs: f64,
    /// Resolution (width x height)
    pub resolution: (u32, u32),
    /// File size in bytes
    pub file_size: u64,
    /// Whether this appears to be already-compressed (true) or phone-raw (false)
    pub is_efficiently_compressed: bool,
    /// Reason for the compression assessment
    pub compression_reason: String,
}

impl VideoAnalysis {
    /// Determine if recompression would be beneficial
    pub fn should_recompress(&self) -> bool {
        !self.is_efficiently_compressed
    }

    /// Estimate potential size reduction if recompressed (percentage)
    pub fn estimated_reduction_percent(&self) -> f64 {
        if self.is_efficiently_compressed {
            0.0
        } else {
            // Phone videos typically compress to 15-30% of original size
            70.0
        }
    }
}

/// Analyze a video file to determine if it needs recompression
pub fn analyze_video_compression(path: impl AsRef<Path>) -> Result<VideoAnalysis> {
    let path = path.as_ref();
    
    // Get file metadata
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    let file_size = metadata.len();

    // Use ffprobe to extract video information
    let probe_output = Command::new("ffprobe")
        .args(&[
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=codec_name,bit_rate,width,height,duration",
            "-show_entries", "format=duration,bit_rate",
            "-of", "default=noprint_wrappers=1",
            path.to_str().unwrap(),
        ])
        .output()
        .context("Failed to execute ffprobe - ensure ffmpeg is installed")?;

    if !probe_output.status.success() {
        let stderr = String::from_utf8_lossy(&probe_output.stderr);
        anyhow::bail!("ffprobe failed: {}", stderr);
    }

    let output_str = String::from_utf8_lossy(&probe_output.stdout);
    
    // Parse ffprobe output
    let mut codec = String::new();
    let mut bitrate_kbps = 0.0;
    let mut duration_secs = 0.0;
    let mut width = 0u32;
    let mut height = 0u32;

    for line in output_str.lines() {
        if let Some(val) = line.strip_prefix("codec_name=") {
            codec = val.to_string();
        } else if let Some(val) = line.strip_prefix("bit_rate=") {
            if let Ok(br) = val.parse::<f64>() {
                bitrate_kbps = br / 1000.0; // Convert to kbps
            }
        } else if let Some(val) = line.strip_prefix("duration=") {
            if let Ok(dur) = val.parse::<f64>() {
                duration_secs = dur;
            }
        } else if let Some(val) = line.strip_prefix("width=") {
            width = val.parse().unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("height=") {
            height = val.parse().unwrap_or(0);
        }
    }

    // If stream bitrate not found, calculate from file size and duration
    if bitrate_kbps == 0.0 && duration_secs > 0.0 {
        bitrate_kbps = (file_size as f64 * 8.0) / (duration_secs * 1000.0);
    }

    // Determine if video is efficiently compressed
    let (is_efficiently_compressed, compression_reason) = 
        assess_compression_efficiency(&codec, bitrate_kbps, width, height, file_size);

    Ok(VideoAnalysis {
        bitrate_kbps,
        codec,
        duration_secs,
        resolution: (width, height),
        file_size,
        is_efficiently_compressed,
        compression_reason,
    })
}

/// Assess whether a video is efficiently compressed based on heuristics
fn assess_compression_efficiency(
    codec: &str,
    bitrate_kbps: f64,
    width: u32,
    height: u32,
    file_size: u64,
) -> (bool, String) {
    let pixels = width as f64 * height as f64;
    
    // Calculate bits per pixel per frame (assuming 30fps)
    let bpp = if pixels > 0.0 && bitrate_kbps > 0.0 {
        (bitrate_kbps * 1000.0) / (pixels * 30.0)
    } else {
        0.0
    };

    // Heuristics for detecting phone-raw vs already-compressed video:
    
    // 1. Very high bitrate suggests inefficient encoding
    //    Phone videos: typically 15-50 Mbps for 1080p
    //    Compressed videos: typically 2-8 Mbps for 1080p
    if bitrate_kbps > 12000.0 {
        return (false, format!(
            "Very high bitrate ({:.1} Mbps) suggests unoptimized encoding",
            bitrate_kbps / 1000.0
        ));
    }

    // 2. High bits-per-pixel suggests wasteful encoding
    //    Phone videos: 0.15-0.25 bpp
    //    Compressed videos: 0.03-0.08 bpp
    if bpp > 0.12 {
        return (false, format!(
            "High bits-per-pixel ({:.3}) indicates inefficient compression",
            bpp
        ));
    }

    // 3. Check for very large file sizes relative to resolution/duration
    //    1080p phone video: ~100-200 MB/minute
    //    1080p compressed: ~15-40 MB/minute
    let resolution_factor = pixels / (1920.0 * 1080.0); // Normalize to 1080p
    let size_mb = file_size as f64 / (1024.0 * 1024.0);
    
    if size_mb > 150.0 * resolution_factor {
        return (false, format!(
            "Large file size ({:.1} MB) for resolution suggests phone encoding",
            size_mb
        ));
    }

    // 4. Low bitrate or reasonable bpp suggests already compressed
    if bitrate_kbps < 8000.0 && bpp < 0.10 {
        return (true, format!(
            "Moderate bitrate ({:.1} Mbps) and bpp ({:.3}) indicate efficient compression",
            bitrate_kbps / 1000.0, bpp
        ));
    }

    // 5. HEVC codec with reasonable bitrate is likely already optimized
    if codec == "hevc" && bitrate_kbps < 10000.0 {
        return (true, "HEVC codec with moderate bitrate suggests prior optimization".to_string());
    }

    // Default: assume moderately compressed
    (true, format!(
        "Bitrate {:.1} Mbps appears reasonably compressed",
        bitrate_kbps / 1000.0
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_assessment() {
        // Phone video: high bitrate, high bpp
        let (compressed, reason) = assess_compression_efficiency("h264", 20000.0, 1920, 1080, 200_000_000);
        assert!(!compressed, "Should detect phone video as needing compression: {}", reason);

        // Already compressed: low bitrate, low bpp
        let (compressed, reason) = assess_compression_efficiency("h264", 3000.0, 1920, 1080, 30_000_000);
        assert!(compressed, "Should detect compressed video: {}", reason);

        // HEVC optimized
        let (compressed, reason) = assess_compression_efficiency("hevc", 5000.0, 1920, 1080, 50_000_000);
        assert!(compressed, "Should detect HEVC as optimized: {}", reason);
    }
}
