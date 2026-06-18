// BPG encoding wrapper for FFI use.
//
// The public OpenArc-facing preset is intentionally named (`balanced`, `good`,
// `best`) because the current bpg-rs integration should not expose libbpg-era
// encoder/compression numeric knobs to the CLI. Internally this module still
// translates those named presets into the numeric fields expected by the lower
// level encoder config currently used by `codecs::bpg`.

use anyhow::{anyhow, Context, Result};
use codecs::bpg::{BPGEncoderConfig, BPGImageFormat, NativeBPGEncoder};
use codecs::heic::{matrix_coeffs_to_bpg_color_space, HeicChromaFormat, HeicCodec};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use crate::image_loader;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BpgEffort {
    Balanced,
    Good,
    Best,
}

impl BpgEffort {
    pub const VALID_VALUES: &'static [&'static str] = &["balanced", "good", "best"];

    pub fn parse(value: &str) -> Result<Self> {
        value.parse()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Good => "good",
            Self::Best => "best",
        }
    }

    /// Conservative default visual-quality value for the named OpenArc preset.
    /// Lower BPG QP means higher quality/larger files. This is kept internal so
    /// the CLI can present stable names instead of numeric BPG knobs.
    pub fn default_quality(self) -> u8 {
        match self {
            Self::Balanced => 32,
            Self::Good => 28,
            Self::Best => 24,
        }
    }

    /// Internal bpg-rs effort/compression mapping.
    pub fn compression_level(self) -> u8 {
        match self {
            Self::Balanced => 6,
            Self::Good => 8,
            Self::Best => 9,
        }
    }

    /// Internal bpg-rs encoder-path mapping. Keep this at the stable/default
    /// backend unless the lower layer grows a named public API for this too.
    pub fn encoder_type(self) -> u8 {
        0
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Balanced => "smaller/faster archival image setting",
            Self::Good => "default archival image setting",
            Self::Best => "larger/slower highest-quality image setting",
        }
    }
}

impl fmt::Display for BpgEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BpgEffort {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "balanced" | "balance" | "medium" => Ok(Self::Balanced),
            "good" | "default" => Ok(Self::Good),
            "best" | "max" | "maximum" => Ok(Self::Best),
            other => Err(anyhow!(
                "invalid BPG effort '{other}', expected one of: {}",
                Self::VALID_VALUES.join(", ")
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BpgConfig {
    pub effort: BpgEffort,
    pub lossless: bool,
    pub bit_depth: u8,
    /// Optional hidden/testing override. Normal CLI paths should leave this as
    /// `None` and use `effort`.
    pub quality_override: Option<u8>,
}

impl BpgConfig {
    pub fn effective_quality(&self) -> u8 {
        if self.lossless {
            0
        } else {
            self.quality_override
                .unwrap_or_else(|| self.effort.default_quality())
        }
    }

    pub fn to_encoder_config(&self, chroma_format: i32) -> BPGEncoderConfig {
        BPGEncoderConfig {
            quality: self.effective_quality() as i32,
            bit_depth: self.bit_depth as i32,
            // The current Rust backend does not expose a true lossless BPG path;
            // quality 0 is the practical near-lossless fallback.
            lossless: 0,
            chroma_format,
            encoder_type: self.effort.encoder_type() as i32,
            compress_level: self.effort.compression_level() as i32,
            color_space: 3, // YCbCr BT.709 (better for HEIC/HEIF sources)
            limited_range: 0,
        }
    }
}

fn heic_chroma_to_bpg_format(chroma_format: HeicChromaFormat) -> BPGImageFormat {
    match chroma_format {
        HeicChromaFormat::Monochrome => BPGImageFormat::Gray,
        HeicChromaFormat::YCbCr420 => BPGImageFormat::YCbCr420P,
        HeicChromaFormat::YCbCr422 => BPGImageFormat::YCbCr422P,
        HeicChromaFormat::YCbCr444 => BPGImageFormat::YCbCr444P,
    }
}

pub fn encode_image_to_bpg<P: AsRef<Path>>(
    input_path: P,
    output_path: P,
    config: &BpgConfig,
) -> Result<()> {
    let input_path_ref = input_path.as_ref();
    output_path
        .as_ref()
        .to_str()
        .context("Invalid output path")?;

    let extension = input_path_ref
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_heic = matches!(extension.as_str(), "heic" | "heif");

    // If HEIC/HEIF, decode to native YUV and preserve source bit depth/chroma.
    if is_heic {
        let mut decoder = HeicCodec::new().context("Failed to create HEIC decoder")?;
        let decoded = decoder
            .decode_file_yuv(input_path_ref)
            .context("Failed to decode HEIC file to YUV")?;

        let mut encoder = NativeBPGEncoder::new().context("Failed to create BPG encoder")?;
        let mut encoder_config =
            config.to_encoder_config(decoded.chroma_format.to_bpg_chroma_format());
        encoder_config.bit_depth = decoded.bit_depth as i32;
        encoder_config.color_space = matrix_coeffs_to_bpg_color_space(decoded.matrix_coeffs);
        encoder_config.limited_range = if decoded.full_range { 0 } else { 1 };
        encoder
            .set_config(&encoder_config)
            .context("Failed to set BPG config")?;

        let bpg_data = encoder
            .encode_from_planar_u16(
                &decoded.y_plane,
                &decoded.cb_plane,
                &decoded.cr_plane,
                decoded.width,
                decoded.height,
                decoded.y_stride,
                decoded.cb_stride,
                decoded.cr_stride,
                heic_chroma_to_bpg_format(decoded.chroma_format),
            )
            .context("Failed to encode HEIC to BPG via planar YUV")?;

        std::fs::write(output_path.as_ref(), bpg_data).context("Failed to write BPG file")?;
        return Ok(());
    }

    // For other formats (JPEG, PNG, WebP, TIFF, etc.), load into memory and encode as RGBA.
    let decoded = image_loader::load_image(input_path.as_ref()).context("Failed to load image file")?;

    let chroma_format = 0; // Default to 4:2:0 (most efficient for photos)

    let mut encoder = NativeBPGEncoder::new().context("Failed to create BPG encoder")?;
    let encoder_config = config.to_encoder_config(chroma_format);
    encoder
        .set_config(&encoder_config)
        .context("Failed to set BPG config")?;

    let (data, width, height) = decoded.to_rgba8();
    let stride = width * 4;

    let bpg_data = encoder
        .encode_from_memory(&data, width, height, stride, BPGImageFormat::RGBA32)
        .context("Failed to encode image to BPG")?;

    std::fs::write(output_path.as_ref(), bpg_data).context("Failed to write BPG file")?;

    Ok(())
}
