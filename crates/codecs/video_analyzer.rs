//! Video compression analysis via ffprobe.
//!
//! This module intentionally avoids linking FFmpeg libraries. It shells out to
//! `ffprobe` and parses stream/format metadata.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct VideoAnalysis {
    pub bitrate_kbps: f64,
    pub codec: String,
    pub duration_secs: f64,
    pub resolution: (u32, u32),
    pub fps: f64,
    pub file_size: u64,
    pub is_efficiently_compressed: bool,
    pub compression_reason: String,
}

impl VideoAnalysis {
    pub fn should_recompress(&self) -> bool {
        !self.is_efficiently_compressed
    }

    pub fn estimated_reduction_percent(&self) -> f64 {
        if self.is_efficiently_compressed {
            0.0
        } else {
            70.0
        }
    }
}

pub fn analyze_video_compression(path: impl AsRef<Path>) -> Result<VideoAnalysis> {
    let path = path.as_ref();
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    let file_size = metadata.len();

    let probe_output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,bit_rate,width,height,duration,avg_frame_rate,r_frame_rate",
            "-show_entries",
            "format=duration,bit_rate",
            "-of",
            "default=noprint_wrappers=1",
            path.to_string_lossy().as_ref(),
        ])
        .output()
        .context("Failed to execute ffprobe")?;

    if !probe_output.status.success() {
        let stderr = String::from_utf8_lossy(&probe_output.stderr);
        anyhow::bail!("ffprobe failed: {}", stderr);
    }

    let output_str = String::from_utf8_lossy(&probe_output.stdout);

    let mut codec = String::new();
    let mut bitrate_kbps = 0.0;
    let mut duration_secs = 0.0;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut fps = 0.0;

    for line in output_str.lines() {
        if let Some(val) = line.strip_prefix("codec_name=") {
            codec = val.to_string();
        } else if let Some(val) = line.strip_prefix("bit_rate=") {
            if let Ok(br) = val.parse::<f64>() {
                bitrate_kbps = br / 1000.0;
            }
        } else if let Some(val) = line.strip_prefix("duration=") {
            if let Ok(dur) = val.parse::<f64>() {
                duration_secs = dur;
            }
        } else if let Some(val) = line.strip_prefix("width=") {
            width = val.parse().unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("height=") {
            height = val.parse().unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("avg_frame_rate=") {
            fps = parse_ratio_fps(val).unwrap_or(fps);
        } else if let Some(val) = line.strip_prefix("r_frame_rate=") {
            if fps <= 0.0 {
                fps = parse_ratio_fps(val).unwrap_or(fps);
            }
        }
    }

    if bitrate_kbps == 0.0 && duration_secs > 0.0 {
        bitrate_kbps = (file_size as f64 * 8.0) / (duration_secs * 1000.0);
    }
    if fps <= 0.0 {
        fps = 30.0;
    }

    let (is_efficiently_compressed, compression_reason) = assess_compression_efficiency(
        &codec,
        bitrate_kbps,
        duration_secs,
        width,
        height,
        fps,
        file_size,
    );

    Ok(VideoAnalysis {
        bitrate_kbps,
        codec,
        duration_secs,
        resolution: (width, height),
        fps,
        file_size,
        is_efficiently_compressed,
        compression_reason,
    })
}

fn parse_ratio_fps(s: &str) -> Option<f64> {
    let (n, d) = s.split_once('/')?;
    let num = n.parse::<f64>().ok()?;
    let den = d.parse::<f64>().ok()?;
    if den <= 0.0 {
        return None;
    }
    Some(num / den)
}

fn assess_compression_efficiency(
    codec: &str,
    bitrate_kbps: f64,
    duration_secs: f64,
    width: u32,
    height: u32,
    fps: f64,
    file_size: u64,
) -> (bool, String) {
    let pixels = width as f64 * height as f64;
    let bpppf = if pixels > 0.0 && bitrate_kbps > 0.0 && fps > 0.0 {
        (bitrate_kbps * 1000.0) / (pixels * fps)
    } else {
        0.0
    };

    if bitrate_kbps > 12_000.0 {
        return (
            false,
            format!(
                "Very high bitrate ({:.1} Mbps) suggests unoptimized encoding",
                bitrate_kbps / 1000.0
            ),
        );
    }

    if bpppf > 0.12 {
        return (
            false,
            format!(
                "High bits-per-pixel-per-frame ({:.3}) indicates inefficient compression",
                bpppf
            ),
        );
    }

    let resolution_factor = pixels / (1920.0 * 1080.0);
    let size_mb = file_size as f64 / (1024.0 * 1024.0);
    let duration_minutes = (duration_secs / 60.0).max(0.01);
    let mb_per_minute = size_mb / duration_minutes;

    if mb_per_minute > 150.0 * resolution_factor {
        return (
            false,
            format!(
                "Large file rate ({:.1} MB/min) for resolution suggests phone/camera source",
                mb_per_minute
            ),
        );
    }

    if bitrate_kbps < 8_000.0 && bpppf < 0.10 {
        return (
            true,
            format!(
                "Moderate bitrate ({:.1} Mbps) and bpppf ({:.3}) indicate efficient compression",
                bitrate_kbps / 1000.0,
                bpppf
            ),
        );
    }

    if codec == "hevc" && bitrate_kbps < 10_000.0 {
        return (
            true,
            "HEVC codec with moderate bitrate suggests prior optimization".to_string(),
        );
    }

    (
        true,
        format!(
            "Bitrate {:.1} Mbps appears reasonably compressed",
            bitrate_kbps / 1000.0
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_assessment() {
        let (compressed, _) =
            assess_compression_efficiency("h264", 20_000.0, 120.0, 1920, 1080, 30.0, 200_000_000);
        assert!(!compressed);

        let (compressed, _) =
            assess_compression_efficiency("h264", 3_000.0, 120.0, 1920, 1080, 30.0, 30_000_000);
        assert!(compressed);

        let (compressed, _) =
            assess_compression_efficiency("hevc", 5_000.0, 120.0, 1920, 1080, 30.0, 50_000_000);
        assert!(compressed);
    }
}
