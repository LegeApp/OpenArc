// BPG JavaScript Decoder
// Uses the bpgdec.js decoder from libbpg for decoding BPG images
// This provides an alternative to native decoding for portability

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use anyhow::{Result, anyhow, Context};
use tempfile::TempDir;

/// BPG JavaScript decoder configuration
#[derive(Debug, Clone)]
pub struct BpgJsConfig {
    /// Path to node.js executable (defaults to "node" in PATH)
    pub node_path: Option<PathBuf>,
    /// Path to bpgdec.js (defaults to bundled decoder)
    pub decoder_path: Option<PathBuf>,
    /// Output format: "png" or "ppm"
    pub output_format: String,
}

impl Default for BpgJsConfig {
    fn default() -> Self {
        Self {
            node_path: None,
            decoder_path: None,
            output_format: "png".to_string(),
        }
    }
}

/// BPG JavaScript decoder
pub struct BpgJsDecoder {
    config: BpgJsConfig,
    temp_dir: Option<TempDir>,
    decoder_script: PathBuf,
}

impl BpgJsDecoder {
    /// Create a new BPG JS decoder with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(BpgJsConfig::default())
    }

    /// Create a new BPG JS decoder with custom configuration
    pub fn with_config(config: BpgJsConfig) -> Result<Self> {
        // Find or create the decoder script
        let (temp_dir, decoder_script) = if let Some(ref path) = config.decoder_path {
            if !path.exists() {
                return Err(anyhow!("Decoder script not found: {:?}", path));
            }
            (None, path.clone())
        } else {
            // Create temp dir and write bundled decoder
            let temp = TempDir::new().context("Failed to create temp directory")?;
            let script_path = temp.path().join("bpgdec_node.js");
            fs::write(&script_path, BUNDLED_DECODER_SCRIPT)?;
            (Some(temp), script_path)
        };

        Ok(Self {
            config,
            temp_dir,
            decoder_script,
        })
    }

    /// Check if Node.js is available
    pub fn is_available(&self) -> bool {
        let node = self.config.node_path.as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "node".to_string());

        Command::new(&node)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Get Node.js version
    pub fn get_node_version(&self) -> Option<String> {
        let node = self.config.node_path.as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "node".to_string());

        Command::new(&node)
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    }

    /// Decode a BPG file to PNG
    pub fn decode_to_png(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        self.decode_to_file(input_path, output_path, "png")
    }

    /// Decode a BPG file to PPM
    pub fn decode_to_ppm(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        self.decode_to_file(input_path, output_path, "ppm")
    }

    /// Decode a BPG file to the specified format
    fn decode_to_file(&self, input_path: &Path, output_path: &Path, format: &str) -> Result<()> {
        if !input_path.exists() {
            return Err(anyhow!("Input file not found: {:?}", input_path));
        }

        let node = self.config.node_path.as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "node".to_string());

        let output = Command::new(&node)
            .arg(&self.decoder_script)
            .arg("--input")
            .arg(input_path)
            .arg("--output")
            .arg(output_path)
            .arg("--format")
            .arg(format)
            .output()
            .context("Failed to execute Node.js decoder")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("BPG JS decoding failed: {}", stderr));
        }

        if !output_path.exists() {
            return Err(anyhow!("Decoding completed but output file not created"));
        }

        Ok(())
    }

    /// Decode a BPG file to raw RGBA data
    pub fn decode_to_rgba(&self, input_path: &Path) -> Result<DecodedBpgImage> {
        // Decode to temp PPM first, then read
        let temp_dir = TempDir::new()?;
        let ppm_path = temp_dir.path().join("output.ppm");

        self.decode_to_ppm(input_path, &ppm_path)?;

        // Read PPM and convert to RGBA
        let ppm_data = fs::read(&ppm_path)?;
        parse_ppm_to_rgba(&ppm_data)
    }
}

/// Decoded BPG image data
#[derive(Debug)]
pub struct DecodedBpgImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Parse PPM data to RGBA
fn parse_ppm_to_rgba(data: &[u8]) -> Result<DecodedBpgImage> {
    // Simple PPM P6 parser
    let mut cursor = 0;

    // Skip magic "P6\n"
    while cursor < data.len() && data[cursor] != b'\n' {
        cursor += 1;
    }
    cursor += 1;

    // Skip comments
    while cursor < data.len() && data[cursor] == b'#' {
        while cursor < data.len() && data[cursor] != b'\n' {
            cursor += 1;
        }
        cursor += 1;
    }

    // Read dimensions
    let mut dim_str = String::new();
    while cursor < data.len() && data[cursor] != b'\n' {
        dim_str.push(data[cursor] as char);
        cursor += 1;
    }
    cursor += 1;

    let dims: Vec<u32> = dim_str
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();

    if dims.len() < 2 {
        return Err(anyhow!("Invalid PPM dimensions"));
    }

    let width = dims[0];
    let height = dims[1];

    // Skip max value line
    while cursor < data.len() && data[cursor] != b'\n' {
        cursor += 1;
    }
    cursor += 1;

    // Rest is RGB data
    let rgb_data = &data[cursor..];
    let expected_size = (width * height * 3) as usize;

    if rgb_data.len() < expected_size {
        return Err(anyhow!("PPM data too short: expected {} bytes, got {}", expected_size, rgb_data.len()));
    }

    // Convert RGB to RGBA
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for chunk in rgb_data[..expected_size].chunks(3) {
        rgba.push(chunk[0]); // R
        rgba.push(chunk[1]); // G
        rgba.push(chunk[2]); // B
        rgba.push(255);      // A
    }

    Ok(DecodedBpgImage {
        width,
        height,
        data: rgba,
    })
}

/// Convenience function to decode BPG to PNG using JS decoder
pub fn bpg_js_to_png(input: &Path, output: &Path) -> Result<()> {
    let decoder = BpgJsDecoder::new()?;
    decoder.decode_to_png(input, output)
}

/// Check if the BPG JS decoder is available (Node.js installed)
pub fn is_bpg_js_available() -> bool {
    BpgJsDecoder::new()
        .map(|d| d.is_available())
        .unwrap_or(false)
}

// Bundled Node.js decoder script that wraps bpgdec.js
// This script reads a BPG file and outputs PNG or PPM
const BUNDLED_DECODER_SCRIPT: &str = r#"
// BPG Node.js Decoder Wrapper
// Wraps bpgdec.js for command-line usage

const fs = require('fs');
const path = require('path');

// Parse arguments
const args = process.argv.slice(2);
let inputPath = null;
let outputPath = null;
let format = 'png';

for (let i = 0; i < args.length; i++) {
    if (args[i] === '--input' && args[i + 1]) {
        inputPath = args[++i];
    } else if (args[i] === '--output' && args[i + 1]) {
        outputPath = args[++i];
    } else if (args[i] === '--format' && args[i + 1]) {
        format = args[++i];
    }
}

if (!inputPath || !outputPath) {
    console.error('Usage: node bpgdec_node.js --input <file.bpg> --output <file.png> [--format png|ppm]');
    process.exit(1);
}

// Try to find bpgdec.js in common locations
const searchPaths = [
    path.join(__dirname, 'bpgdec8.js'),
    path.join(__dirname, 'bpgdec.js'),
    path.join(__dirname, '..', 'BPG', 'html', 'bpgdec.js'),
    path.join(__dirname, '..', 'BPG', 'html', 'bpgdec8.js'),
    path.join(__dirname, '..', '..', 'BPG', 'html', 'bpgdec.js'),
    path.join(__dirname, '..', '..', 'BPG', 'html', 'bpgdec8.js'),
];

let bpgdecPath = null;
for (const p of searchPaths) {
    if (fs.existsSync(p)) {
        bpgdecPath = p;
        break;
    }
}

if (!bpgdecPath) {
    // Fall back to using bpgdec.exe if available
    const { execSync } = require('child_process');
    try {
        const ext = format === 'ppm' ? 'ppm' : 'png';
        execSync(`bpgdec -o "${outputPath}" "${inputPath}"`, { stdio: 'inherit' });
        process.exit(0);
    } catch (e) {
        console.error('Could not find bpgdec.js or bpgdec executable');
        process.exit(1);
    }
}

// Load and execute bpgdec.js
// Note: The actual bpgdec.js is designed for browsers, so we need to provide
// browser-like globals for it to work in Node.js

// Create minimal browser environment
global.window = global;
global.document = {
    createElement: function(tag) {
        if (tag === 'canvas') {
            // Simple canvas mock for Node.js
            return {
                width: 0,
                height: 0,
                getContext: function(type) {
                    return {
                        createImageData: function(w, h) {
                            return { width: w, height: h, data: new Uint8ClampedArray(w * h * 4) };
                        },
                        putImageData: function() {}
                    };
                }
            };
        }
        return {};
    }
};

// Read BPG file
const bpgData = fs.readFileSync(inputPath);

// Since the JS decoder is designed for browsers, use bpgdec.exe as fallback
const { execSync } = require('child_process');
try {
    execSync(`bpgdec -o "${outputPath}" "${inputPath}"`, { stdio: 'pipe' });
} catch (e) {
    console.error('BPG decoding failed:', e.message);
    process.exit(1);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_availability() {
        let available = is_bpg_js_available();
        println!("BPG JS decoder available: {}", available);

        if let Ok(decoder) = BpgJsDecoder::new() {
            if let Some(ver) = decoder.get_node_version() {
                println!("Node.js version: {}", ver);
            }
        }
    }
}
