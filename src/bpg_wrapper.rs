// BPG encoding wrapper for FFI use.
//
// The public OpenArc-facing presets intentionally mirror the production
// bpg-rs shape: `best` (upstream `Effort::Slow`) and `fast`. Experimental
// Placebo and legacy libbpg numeric knobs are not exposed. Internally this
// module translates the names into the numeric compatibility fields used by
// `codecs::bpg`.

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
    /// Faster encode with cheaper search budgets. Maps to bpg-rs `Effort::Fast`.
    Fast,
    /// Production archival tier and the default. Maps to bpg-rs `Effort::Slow`
    /// (documented upstream as "the default archival-quality preset").
    Best,
}

impl BpgEffort {
    pub const VALID_VALUES: &'static [&'static str] = &["best", "fast"];

    pub fn parse(value: &str) -> Result<Self> {
        value.parse()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Best => "best",
        }
    }

    /// Conservative default visual-quality value for the named OpenArc preset.
    /// Lower BPG QP means higher quality/larger files.  The default archival
    /// presets intentionally use stock `bpgenc`'s default QP 29 so effort
    /// choices change encoder speed/search depth without silently increasing
    /// bitrate.
    pub fn default_quality(self) -> u8 {
        match self {
            Self::Fast => 32,
            Self::Best => 29,
        }
    }

    /// Internal bpg-rs effort/compression mapping.
    pub fn compression_level(self) -> u8 {
        match self {
            Self::Fast => 6,
            Self::Best => 8,
        }
    }

    /// Internal bpg-rs effort mapping: 0=Fast, 1=Slow (the production "Best"
    /// tier). The experimental Placebo tier (2) is intentionally not selectable.
    pub fn encoder_type(self) -> u8 {
        match self {
            Self::Fast => 0,
            Self::Best => 1,
        }
    }

    /// Two-pass measured AQ requires bpg-rs `Effort::Slow` or `Placebo`; only the
    /// `Best` tier maps onto that, so `Fast` cannot run it.
    pub fn supports_two_pass_aq(self) -> bool {
        matches!(self, Self::Best)
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Fast => "faster encode, slightly larger files",
            Self::Best => "production archival image quality (default)",
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
            "fast" => Ok(Self::Fast),
            "best" => Ok(Self::Best),
            other => Err(anyhow!(
                "invalid BPG effort '{other}', expected one of: {}",
                Self::VALID_VALUES.join(", ")
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BpgAq {
    Off,
    TwoPass,
    Perceptual,
    PerceptualChroma,
}

impl BpgAq {
    pub const VALID_VALUES: &'static [&'static str] =
        &["off", "two-pass", "perceptual", "perceptual-chroma"];

    pub fn parse(value: &str) -> Result<Self> {
        value.parse()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::TwoPass => "two-pass",
            Self::Perceptual => "perceptual",
            Self::PerceptualChroma => "perceptual-chroma",
        }
    }

    pub fn validate_for_effort(self, effort: BpgEffort) -> Result<()> {
        if self == Self::TwoPass && !effort.supports_two_pass_aq() {
            return Err(anyhow!("BPG two-pass AQ requires --bpg-effort best"));
        }
        Ok(())
    }
}

impl Default for BpgAq {
    fn default() -> Self {
        Self::Off
    }
}

impl fmt::Display for BpgAq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BpgAq {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "0" => Ok(Self::Off),
            "two-pass" | "twopass" | "2pass" | "2-pass" => Ok(Self::TwoPass),
            "perceptual" => Ok(Self::Perceptual),
            "perceptual-chroma" | "perceptual_chroma" => Ok(Self::PerceptualChroma),
            other => Err(anyhow!(
                "invalid BPG AQ preset '{other}', expected one of: {}",
                Self::VALID_VALUES.join(", ")
            )),
        }
    }
}

// ============================================================================
// Per-backend CLI surface
// ----------------------------------------------------------------------------
// OpenArc deliberately exposes only the production bpg-rs shape:
// - Best (upstream `Effort::Slow`) as the default
// - Fast as the other effort tier
// - AQ off by default, with two-pass as the recommended opt-in followed by the
//   two supported single-pass perceptual variants.
//
// Centralizing the allowed values here keeps the cfg logic in one place; cli.rs,
// main.rs and interactive.rs consume these.
// ============================================================================

#[cfg(not(feature = "bpg-rs"))]
mod backend_cli {
    use super::BpgAq;

    pub const EFFORT_CLI_VALUES: &[&str] = &["best", "fast"];
    pub const EFFORT_CLI_DEFAULT: &str = "best";

    /// The legacy C/x265 path does not expose the bpg-rs AQ selector.
    pub const AQ_CLI_DEFAULT: BpgAq = BpgAq::Off;
}

#[cfg(feature = "bpg-rs")]
mod backend_cli {
    /// Production effort presets exposed for the pure-Rust backend.
    pub const EFFORT_CLI_VALUES: &[&str] = &["best", "fast"];
    pub const EFFORT_CLI_DEFAULT: &str = "best";

    /// Order is intentional: AQ remains disabled unless requested; when the
    /// flag is supplied without a value, two-pass is the recommended mode.
    pub const AQ_CLI_VALUES: &[&str] = &["off", "two-pass", "perceptual", "perceptual-chroma"];
    pub const AQ_CLI_DEFAULT: &str = "off";
    pub const AQ_CLI_DEFAULT_WHEN_ENABLED: &str = "two-pass";
}

pub use backend_cli::*;

#[derive(Debug, Clone)]
pub struct BpgConfig {
    pub effort: BpgEffort,
    pub aq: BpgAq,
    pub bit_depth: u8,
    /// Optional hidden/testing override. Normal CLI paths should leave this as
    /// `None` and use `effort`.
    pub quality_override: Option<u8>,
}

impl BpgConfig {
    pub fn effective_quality(&self) -> u8 {
        self.quality_override
            .unwrap_or_else(|| self.effort.default_quality())
    }

    pub fn to_encoder_config(&self, chroma_format: i32) -> BPGEncoderConfig {
        let (aq_mode, aq_strength, aq_clamp) =
            codecs::bpg::resolve_aq_preset(self.aq.as_str()).unwrap_or((0, 0.0, 2));
        BPGEncoderConfig {
            quality: self.effective_quality() as i32,
            bit_depth: self.bit_depth as i32,
            lossless: 0,
            chroma_format,
            encoder_type: self.effort.encoder_type() as i32,
            compress_level: self.effort.compression_level() as i32,
            aq_mode,
            aq_strength,
            aq_clamp,
            two_pass_gate: true,
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
    let decoded =
        image_loader::load_image(input_path.as_ref()).context("Failed to load image file")?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_effort_surface_is_best_then_fast() {
        assert_eq!(EFFORT_CLI_VALUES, &["best", "fast"]);
        assert_eq!(EFFORT_CLI_DEFAULT, "best");
        assert_eq!(BpgEffort::Best.encoder_type(), 1);
        assert_eq!(BpgEffort::Fast.encoder_type(), 0);
        assert!(BpgEffort::parse("placebo").is_err());
        assert!(BpgEffort::parse("balanced").is_err());
    }

    #[cfg(feature = "bpg-rs")]
    #[test]
    fn public_aq_surface_is_off_then_recommended_two_pass() {
        assert_eq!(
            AQ_CLI_VALUES,
            &["off", "two-pass", "perceptual", "perceptual-chroma"]
        );
        assert_eq!(AQ_CLI_DEFAULT, "off");
        assert_eq!(AQ_CLI_DEFAULT_WHEN_ENABLED, "two-pass");

        let (mode, strength, clamp) =
            codecs::bpg::resolve_aq_preset("two-pass").expect("upstream AQ preset");
        assert_eq!(mode, 6);
        assert!((strength - 1.0).abs() < f32::EPSILON);
        assert_eq!(clamp, 4);
    }

    #[test]
    fn two_pass_is_only_valid_for_production_best() {
        assert!(BpgAq::TwoPass.validate_for_effort(BpgEffort::Best).is_ok());
        assert!(BpgAq::TwoPass.validate_for_effort(BpgEffort::Fast).is_err());
    }
}
