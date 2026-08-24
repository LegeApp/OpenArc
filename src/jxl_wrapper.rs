//! The OpenArc-facing JPEG XL preset surface, and single-file conversion.
//!
//! This replaced `bpg_wrapper.rs`. The presets are deliberately few — `best`
//! (the default), `fast` and `lossless` — and the numeric knobs
//! underneath them live in [`codecs::jxl`] rather than being spelled out on the
//! command line.
//!
//! # What changed relative to the BPG surface
//!
//! * There is no chroma-format option, because JPEG XL has no chroma
//!   subsampling to select. The BPG path defaulted to 4:2:0 for photos; nothing
//!   here subsamples at any setting.
//! * There is no adaptive-quantization selector. AQ was a BPG/x265 knob; the
//!   JPEG XL rate controller owns that decision.
//! * There is no bit-depth flag. Depth is taken from the source
//!   ([`crate::image_source`]) instead of being configured, so a 16-bit
//!   original is encoded and declared at 16-bit without anyone asking.
//! * The quality dial is a **bitrate**, not a QP. See [`codecs::jxl`] for why.

use anyhow::{Context, Result};
use std::path::Path;

use crate::image_source;
use crate::orchestrator::OriginalImageFormat;

pub use codecs::jxl::{JxlConfig, JxlEffort, JxlMode};

/// Effort values the CLI accepts, in the order they are offered.
pub const EFFORT_CLI_VALUES: &[&str] = JxlEffort::VALID_VALUES;
/// The default effort when none is given.
pub const EFFORT_CLI_DEFAULT: &str = "best";

/// The file extension every encoded image gets.
pub const JXL_EXTENSION: &str = "jxl";

/// Builds an encoder config from the CLI-facing preset name and optional
/// bitrate override.
pub fn config_from_cli(effort: &str, bits_per_pixel: Option<f64>) -> Result<JxlConfig> {
    let effort = JxlEffort::parse(effort)?;
    if let Some(bpp) = bits_per_pixel {
        if !(bpp.is_finite() && bpp > 0.0) {
            anyhow::bail!("the bitrate target must be a positive number, got {bpp}");
        }
        if effort.is_lossless() {
            // Spelled without a flag prefix because `create` calls these
            // --jxl-effort/--jxl-bpp while `convert-jxl`/`batch-jxl` call them
            // --effort/--bpp; naming one spelling would be wrong for the other.
            anyhow::bail!(
                "a bitrate target has no meaning with the lossless preset: a \
                 lossless encode has no rate to hit"
            );
        }
    }
    Ok(JxlConfig {
        effort,
        bits_per_pixel,
        container: false,
    })
}

/// Classifies `path` well enough to pick a decoder, for callers outside the
/// archive pipeline (the `convert`/`batch` subcommands).
fn format_for(path: &Path) -> OriginalImageFormat {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => OriginalImageFormat::Jpeg,
        "png" => OriginalImageFormat::Png,
        "heic" | "heif" | "hif" => OriginalImageFormat::Heic,
        "tif" | "tiff" => OriginalImageFormat::Tiff,
        "bmp" => OriginalImageFormat::Bmp,
        "webp" => OriginalImageFormat::WebP,
        other if raw_autotune::files::is_supported_raw(Path::new(&format!("x.{other}"))) => {
            OriginalImageFormat::Raw
        }
        _ => OriginalImageFormat::Png,
    }
}

/// Encodes one image file to a `.jxl` file.
///
/// Returns the number of bytes written.
pub fn encode_image_to_jxl<P: AsRef<Path>>(
    input_path: P,
    output_path: P,
    config: &JxlConfig,
) -> Result<u64> {
    let input = input_path.as_ref();
    let output = output_path.as_ref();

    let prepared = image_source::load(input, format_for(input))
        .with_context(|| format!("Failed to load image: {}", input.display()))?;

    if prepared.has_transparency {
        anyhow::bail!(
            "{} has a transparency channel, which the JPEG XL encoder cannot yet \
             carry (extra channels are unimplemented). Encoding it would silently \
             discard the alpha, so it is refused here; the archive pipeline stores \
             such images unchanged instead.",
            input.display()
        );
    }

    let bytes = codecs::jxl::encode(&prepared.as_jxl_image(), config)
        .with_context(|| format!("Failed to encode {} to JPEG XL", input.display()))?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, &bytes)
        .with_context(|| format!("Failed to write JPEG XL file: {}", output.display()))?;
    Ok(bytes.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_public_effort_surface_leads_with_best() {
        assert_eq!(EFFORT_CLI_DEFAULT, "best");
        assert_eq!(EFFORT_CLI_VALUES, &["best", "fast", "lossless"]);
        assert!(JxlEffort::parse("placebo").is_err());
    }

    #[test]
    fn a_bitrate_override_is_rejected_for_a_lossless_encode() {
        // Silently ignoring it would let a caller believe they had asked for
        // something the encode did not do.
        assert!(config_from_cli("lossless", Some(2.0)).is_err());
        assert!(config_from_cli("lossless", None).is_ok());
        assert!(config_from_cli("best", Some(2.0)).is_ok());
    }

    #[test]
    fn a_nonsense_bitrate_is_rejected() {
        assert!(config_from_cli("best", Some(0.0)).is_err());
        assert!(config_from_cli("best", Some(-1.0)).is_err());
        assert!(config_from_cli("best", Some(f64::NAN)).is_err());
    }

    #[test]
    fn the_default_config_is_lossy_best() {
        let config = config_from_cli("best", None).expect("valid");
        assert_eq!(config.effort, JxlEffort::Best);
        match config.mode() {
            JxlMode::Lossy { bits_per_pixel } => assert!(bits_per_pixel > 0.0),
            JxlMode::Lossless => panic!("best is a lossy preset"),
        }
    }
}
