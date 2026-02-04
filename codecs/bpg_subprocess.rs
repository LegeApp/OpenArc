// BPG Encoding via native DLL (openarc_bpg.dll)
// Provides access to both x265 and JCTVC encoders directly without subprocess overhead
//
// Encoder types:
//   - x265: Fast encoder for standard mode (good speed, good compression)
//   - JCTVC: Reference encoder for slow mode (slower, ~25% better compression)

use std::path::{Path, PathBuf};
use std::process::Command;
use std::fmt;
use std::ffi::{CString, CStr};
use std::os::raw::{c_char, c_int, c_void};

#[derive(Debug)]
pub struct BpgError(String);

impl fmt::Display for BpgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BpgError {}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// BPG encoder type
/// 
/// The DLL supports both encoders when built with build_openarc_combined_dll.bat:
/// - x265 (encoder_type = 0): Fast, good compression - use for standard/default mode
/// - JCTVC (encoder_type = 1): Slower, best compression - use for slow/high-quality mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpgEncoderType {
    /// x265 encoder (fast, good compression) - for standard/default encoding mode
    X265,
    /// JCTVC encoder (slower, ~25% better compression) - for slow/high-quality mode
    Jctvc,
}

impl BpgEncoderType {
    /// Get the encoder name string for bpgenc command line
    pub fn as_str(&self) -> &'static str {
        match self {
            BpgEncoderType::X265 => "x265",
            BpgEncoderType::Jctvc => "jctvc",
        }
    }
    
    /// Get the encoder type integer for DLL API (0 = x265, 1 = JCTVC)
    pub fn as_int(&self) -> c_int {
        match self {
            BpgEncoderType::X265 => 0,
            BpgEncoderType::Jctvc => 1,
        }
    }
    
    /// Get recommended quality for this encoder type
    pub fn default_quality(&self) -> u8 {
        match self {
            BpgEncoderType::X265 => 28,    // Standard quality
            BpgEncoderType::Jctvc => 28,   // Same quality, but better compression
        }
    }
    
    /// Get recommended compression level
    pub fn default_compress_level(&self) -> u8 {
        match self {
            BpgEncoderType::X265 => 5,     // Fast compression
            BpgEncoderType::Jctvc => 8,    // Better compression (slower)
        }
    }
}

/// BPG encoder configuration
#[derive(Debug, Clone)]
pub struct BpgConfig {
    pub quality: u8,           // 0-51, lower is better quality (default: 28)
    pub encoder_type: BpgEncoderType,
    pub lossless: bool,
    pub compress_level: u8,    // 1-9 (default: 8)
}

impl Default for BpgConfig {
    fn default() -> Self {
        Self {
            quality: 28,
            encoder_type: BpgEncoderType::X265,  // Use x265 by default for good speed/quality balance
            lossless: false,
            compress_level: 5,
        }
    }
}

impl BpgConfig {
    /// Create config for fast encoding (lower compression, faster speed)
    pub fn fast() -> Self {
        Self {
            encoder_type: BpgEncoderType::X265,
            compress_level: BpgEncoderType::X265.default_compress_level(),
            quality: BpgEncoderType::X265.default_quality(),
            ..Default::default()
        }
    }
    
    /// Create config for best compression (JCTVC, slower)
    pub fn best_compression() -> Self {
        Self {
            encoder_type: BpgEncoderType::Jctvc,
            compress_level: BpgEncoderType::Jctvc.default_compress_level(),
            quality: BpgEncoderType::Jctvc.default_quality(),
            ..Default::default()
        }
    }
}

/// BPG Encoder using subprocess
pub struct BpgEncoder {
    encoder_path: PathBuf,
    config: BpgConfig,
}

impl BpgEncoder {
    /// Create encoder with path to bpgenc-jctvc.exe
    pub fn new<P: AsRef<Path>>(encoder_path: P, config: BpgConfig) -> Result<Self> {
        let encoder_path = encoder_path.as_ref().to_path_buf();
        
        if !encoder_path.exists() {
            return Err(Box::new(BpgError(format!(
                "BPG encoder not found: {}",
                encoder_path.display()
            ))));
        }
        
        Ok(Self {
            encoder_path,
            config,
        })
    }
    
    /// Create encoder with default configuration
    pub fn with_defaults<P: AsRef<Path>>(encoder_path: P) -> Result<Self> {
        Self::new(encoder_path, BpgConfig::default())
    }
    
    /// Encode image file to BPG
    pub fn encode_file<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input_path: P,
        output_path: Q,
    ) -> Result<()> {
        let input = input_path.as_ref();
        let output = output_path.as_ref();
        
        if !input.exists() {
            return Err(Box::new(BpgError(format!("Input file not found: {}", input.display()))));
        }
        
        let mut cmd = Command::new(&self.encoder_path);
        
        // Set encoder type
        cmd.arg("-e").arg(self.config.encoder_type.as_str());
        
        // Set quality
        cmd.arg("-q").arg(self.config.quality.to_string());
        
        // Set compression level
        cmd.arg("-m").arg(self.config.compress_level.to_string());
        
        // Lossless mode
        if self.config.lossless {
            cmd.arg("-lossless");
        }
        
        // Output file
        cmd.arg("-o").arg(output);
        
        // Input file
        cmd.arg(input);
        
        // Execute
        let output_result = cmd.output()
            .map_err(|e| Box::new(BpgError(format!("Failed to execute {}: {}", self.encoder_path.display(), e))) as Box<dyn std::error::Error>)?;
        
        if !output_result.status.success() {
            let stderr = String::from_utf8_lossy(&output_result.stderr);
            return Err(Box::new(BpgError(format!("BPG encoding failed: {}", stderr))));
        }
        
        Ok(())
    }
    
    /// Encode with custom quality (convenience method)
    pub fn encode_file_with_quality<P: AsRef<Path>, Q: AsRef<Path>>(
        encoder_path: P,
        input_path: Q,
        output_path: Q,
        quality: u8,
        encoder_type: BpgEncoderType,
    ) -> Result<()> {
        let config = BpgConfig {
            quality,
            encoder_type,
            ..Default::default()
        };
        
        let encoder = Self::new(encoder_path, config)?;
        encoder.encode_file(input_path, output_path)
    }
    
    /// Get supported encoders from bpgenc-jctvc.exe
    pub fn get_supported_encoders<P: AsRef<Path>>(encoder_path: P) -> Result<Vec<String>> {
        let output = Command::new(encoder_path.as_ref())
            .arg("-h")
            .output()
            .map_err(|e| Box::new(BpgError(format!("Failed to get encoder help: {}", e))) as Box<dyn std::error::Error>)?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);
        
        // Parse encoder line: "-e encoder           select the HEVC encoder (jctvc, default = jctvc)"
        let encoders: Vec<String> = combined
            .lines()
            .filter(|line| line.contains("-e encoder"))
            .flat_map(|line| {
                // Extract text in parentheses
                if let Some(start) = line.find('(') {
                    if let Some(end) = line.find(')') {
                        let encoder_text = &line[start+1..end];
                        return encoder_text
                            .split(',')
                            .map(|s| s.trim())
                            .filter(|s| !s.starts_with("default"))
                            .map(String::from)
                            .collect::<Vec<_>>();
                    }
                }
                Vec::new()
            })
            .collect();
        
        Ok(encoders)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = BpgConfig::default();
        assert_eq!(config.quality, 28);
        assert_eq!(config.encoder_type, BpgEncoderType::Jctvc);
        assert_eq!(config.lossless, false);
    }
    
    #[test]
    fn test_fast_config() {
        let config = BpgConfig::fast();
        assert_eq!(config.encoder_type, BpgEncoderType::X265);
        assert_eq!(config.compress_level, 5);
    }
}
