//! JPEG XL encoding and decoding for OpenArc, over the sibling JPXL crates.
//!
//! This is the module that replaced BPG as OpenArc's image codec. Its shape is
//! deliberately different from the BPG wrapper it succeeded, in three ways that
//! matter to an archiver:
//!
//! * **Nothing is subsampled.** The BPG path encoded photos as YCbCr 4:2:0,
//!   which throws away three quarters of the chroma before the encoder ever
//!   runs. A JPEG XL VarDCT frame is XYB throughout — there is no chroma
//!   subsampling to configure and none is applied, at any setting.
//! * **Source precision is carried, not clamped.** [`JxlSamples::Rgb16`] feeds
//!   the encoder 9..=16-bit samples at their own depth, and the declared
//!   `bit_depth` follows, so a 16-bit original decodes back as 16-bit instead
//!   of being crushed to 8-bit on the way out.
//! * **Lossless is a first-class mode**, not an afterthought:
//!   [`JxlEffort::Lossless`] takes the modular track, whose decoded samples are
//!   exactly the encoded samples.
//!
//! # The quality dial is a rate, not a distance
//!
//! JPXL's lossy rate loop targets a **size** — bits per pixel — because it has
//! no perceptual model to target a distance with. This is the one place where
//! the switch away from BPG is not a drop-in: BPG's QP asked for a quality and
//! got whatever size that implied, and this asks for a size and gets whatever
//! quality that implies. [`JxlEffort`] therefore names bitrates, and the
//! numbers it maps to are archival-leaning rather than web-leaning.

use anyhow::{anyhow, Context, Result};
use image::{DynamicImage, ImageBuffer, Luma, Rgb};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

/// The largest bit depth the JPEG XL path carries end to end.
pub const MAX_BITS_PER_SAMPLE: u32 = jpxl_encode::MAX_BITS_PER_SAMPLE;

/// Whether the encode is rate-targeted VarDCT or exact-sample modular.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JxlMode {
    /// Lossy VarDCT at a bits-per-pixel target.
    Lossy {
        /// Target bits per pixel. Higher is larger and closer to the source.
        bits_per_pixel: f64,
    },
    /// Lossless modular: the decoded samples are the encoded samples.
    Lossless,
}

/// The public effort/quality presets OpenArc exposes.
///
/// Each names a bitrate for the lossy path plus a modular search effort for the
/// lossless one. The bitrates are chosen for an archive rather than for a web
/// page: the cheapest preset here is still above where JPEG XL is usually run.
/// The lossy presets map only to JPXL's production controllers; its exhaustive
/// `Quality` reference controller is deliberately not exposed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JxlEffort {
    /// Fastest encode, lower bitrate of the two lossy tiers.
    Fast,
    /// Production archival tier and the default.
    Best,
    /// Exact-sample modular encoding. Ignores the bitrate entirely.
    Lossless,
}

impl JxlEffort {
    pub const VALID_VALUES: &'static [&'static str] = &["best", "fast", "lossless"];

    pub fn parse(value: &str) -> Result<Self> {
        value.parse()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Best => "best",
            Self::Lossless => "lossless",
        }
    }

    /// The default bits-per-pixel target for the lossy presets.
    ///
    /// These sit deliberately high. OpenArc is an archiver, so the failure that
    /// matters is an artifact preserved forever, not a megabyte spent once —
    /// and JPXL's own documentation is explicit that its density still trails
    /// libjxl, so borrowing libjxl's usual operating points would land at a
    /// visibly worse place than they do there. Override with `--jxl-bpp` when a
    /// corpus has been measured and a lower rate is known to be enough.
    pub fn default_bits_per_pixel(self) -> f64 {
        match self {
            Self::Fast => 1.5,
            Self::Best => 2.5,
            // Unused: the lossless preset never consults a rate.
            Self::Lossless => 0.0,
        }
    }

    /// Modular search effort (1..=9) for the lossless mode.
    ///
    /// Every level is exact-lossless, so this trades encode time against file
    /// size and never against pixels.
    pub fn lossless_effort(self) -> u8 {
        match self {
            Self::Fast => 1,
            Self::Best | Self::Lossless => 7,
        }
    }

    /// The rate-controller preset the lossy path runs.
    fn rate_preset(self) -> jpxl_encode_policy::RateSearchPreset {
        match self {
            Self::Fast => jpxl_encode_policy::RateSearchPreset::Fast,
            Self::Best | Self::Lossless => jpxl_encode_policy::RateSearchPreset::Balanced,
        }
    }

    pub fn is_lossless(self) -> bool {
        matches!(self, Self::Lossless)
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Fast => "fastest encode, smallest files",
            Self::Best => "production archival image quality (default)",
            Self::Lossless => "exact samples preserved; much larger files",
        }
    }
}

impl Default for JxlEffort {
    fn default() -> Self {
        Self::Best
    }
}

impl fmt::Display for JxlEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for JxlEffort {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fast" => Ok(Self::Fast),
            "best" => Ok(Self::Best),
            "lossless" => Ok(Self::Lossless),
            other => Err(anyhow!(
                "invalid JPEG XL effort '{other}', expected one of: {}",
                Self::VALID_VALUES.join(", ")
            )),
        }
    }
}

/// A complete JPEG XL encode request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JxlConfig {
    pub effort: JxlEffort,
    /// Overrides [`JxlEffort::default_bits_per_pixel`] when set.
    pub bits_per_pixel: Option<f64>,
    /// Wrap the codestream in a Part 2 container. OpenArc writes naked
    /// codestreams: the container buys nothing when the archive already
    /// carries the filename and the metadata sidecar.
    pub container: bool,
}

impl Default for JxlConfig {
    fn default() -> Self {
        Self {
            effort: JxlEffort::default(),
            bits_per_pixel: None,
            container: false,
        }
    }
}

impl JxlConfig {
    pub fn mode(&self) -> JxlMode {
        if self.effort.is_lossless() {
            JxlMode::Lossless
        } else {
            JxlMode::Lossy {
                bits_per_pixel: self
                    .bits_per_pixel
                    .unwrap_or_else(|| self.effort.default_bits_per_pixel()),
            }
        }
    }
}

/// Interleaved source samples, at whichever width the source actually has.
#[derive(Debug, Clone, Copy)]
pub enum JxlSamples<'a> {
    /// Interleaved 8-bit RGB.
    Rgb8(&'a [u8]),
    /// Interleaved RGB at 9..=16 bits per sample, one `u16` per sample.
    Rgb16(&'a [u16]),
    /// 8-bit greyscale, one sample per pixel.
    Gray8(&'a [u8]),
    /// Greyscale at 9..=16 bits per sample.
    Gray16(&'a [u16]),
}

impl JxlSamples<'_> {
    fn channels(&self) -> usize {
        match self {
            Self::Rgb8(_) | Self::Rgb16(_) => 3,
            Self::Gray8(_) | Self::Gray16(_) => 1,
        }
    }

    fn count(&self) -> usize {
        match self {
            Self::Rgb8(s) | Self::Gray8(s) => s.len(),
            Self::Rgb16(s) | Self::Gray16(s) => s.len(),
        }
    }
}

/// One image handed to [`encode`].
#[derive(Debug, Clone, Copy)]
pub struct JxlImage<'a> {
    pub width: u32,
    pub height: u32,
    /// The source's own depth, in `1..=16`. Declared into the codestream so a
    /// decoder reconstructs at this precision rather than at 8-bit.
    pub bits_per_sample: u32,
    pub samples: JxlSamples<'a>,
}

impl JxlImage<'_> {
    fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            return Err(anyhow!(
                "zero image dimension: {}x{}",
                self.width,
                self.height
            ));
        }
        if self.bits_per_sample == 0 || self.bits_per_sample > MAX_BITS_PER_SAMPLE {
            return Err(anyhow!(
                "bit depth {} is outside the 1..={} JPEG XL path",
                self.bits_per_sample,
                MAX_BITS_PER_SAMPLE
            ));
        }
        let expected = self.width as usize * self.height as usize * self.samples.channels();
        if self.samples.count() != expected {
            return Err(anyhow!(
                "sample count mismatch: {} samples for a {}x{} image with {} channel(s), expected {}",
                self.samples.count(),
                self.width,
                self.height,
                self.samples.channels(),
                expected
            ));
        }
        Ok(())
    }
}

/// Encodes `image` to a JPEG XL codestream.
///
/// Greyscale takes the modular track whatever `config` asks for: the lossy
/// VarDCT path is defined over three colour channels, and replicating a grey
/// plane into three to reach it would spend bits coding two channels that are
/// known to be redundant. Modular is lossless there, which is the better
/// outcome anyway — greyscale sources are rare and small in a photo archive.
pub fn encode(image: &JxlImage<'_>, config: &JxlConfig) -> Result<Vec<u8>> {
    image.validate()?;

    match (config.mode(), image.samples) {
        (JxlMode::Lossy { bits_per_pixel }, JxlSamples::Rgb8(rgb)) => {
            encode_lossy_rgb8(image, rgb, bits_per_pixel, config)
        }
        (JxlMode::Lossy { bits_per_pixel }, JxlSamples::Rgb16(rgb)) => {
            encode_lossy_rgb16(image, rgb, bits_per_pixel, config)
        }
        // Greyscale, and every lossless request, go through modular.
        _ => encode_lossless(image, config),
    }
}

fn rate_request(
    effort: JxlEffort,
    target: jpxl_encode_policy::RateTarget,
) -> jpxl_encode_policy::EncodeRequest {
    let mut request = jpxl_encode_policy::EncodeRequest::for_target(target);
    request.rate_preset = effort.rate_preset();
    request
}

fn encode_lossy_rgb8(
    image: &JxlImage<'_>,
    rgb: &[u8],
    bits_per_pixel: f64,
    config: &JxlConfig,
) -> Result<Vec<u8>> {
    let target = jpxl_encode_policy::RateTarget::BitsPerPixel(bits_per_pixel);
    let request = rate_request(config.effort, target);
    let outcome =
        jpxl_encode_policy::encode_srgb8_to_target(image.width, image.height, rgb, &request, target)
            .map_err(|e| anyhow!("JPEG XL lossy encode failed: {e}"))?;
    Ok(outcome.codestream)
}

fn encode_lossy_rgb16(
    image: &JxlImage<'_>,
    rgb: &[u16],
    bits_per_pixel: f64,
    config: &JxlConfig,
) -> Result<Vec<u8>> {
    let target = jpxl_encode_policy::RateTarget::BitsPerPixel(bits_per_pixel);
    let request = rate_request(config.effort, target);
    let outcome = jpxl_encode_policy::encode_srgb16_to_target(
        image.width,
        image.height,
        rgb,
        image.bits_per_sample,
        &request,
        target,
    )
    .map_err(|e| anyhow!("JPEG XL lossy encode failed: {e}"))?;
    Ok(outcome.codestream)
}

fn encode_lossless(image: &JxlImage<'_>, config: &JxlConfig) -> Result<Vec<u8>> {
    let planes: Vec<Vec<i32>> = match image.samples {
        JxlSamples::Rgb8(rgb) => deinterleave(rgb.iter().map(|&s| i32::from(s)), 3),
        JxlSamples::Rgb16(rgb) => deinterleave(rgb.iter().map(|&s| i32::from(s)), 3),
        JxlSamples::Gray8(g) => vec![g.iter().map(|&s| i32::from(s)).collect()],
        JxlSamples::Gray16(g) => vec![g.iter().map(|&s| i32::from(s)).collect()],
    };

    let inner = jpxl_encode::Image::new(image.width, image.height, image.bits_per_sample, planes)
        .map_err(|e| anyhow!("JPEG XL lossless encode rejected the image: {e}"))?;
    let options = jpxl_encode::EncodeOptions {
        container: config.container,
        effort: jpxl_encode::Effort::new(config.effort.lossless_effort())
            .unwrap_or(jpxl_encode::Effort::DEFAULT),
        ..jpxl_encode::EncodeOptions::default()
    };
    jpxl_encode::encode(&inner, &options)
        .map_err(|e| anyhow!("JPEG XL lossless encode failed: {e}"))
}

fn deinterleave(samples: impl Iterator<Item = i32>, channels: usize) -> Vec<Vec<i32>> {
    let mut planes = vec![Vec::new(); channels];
    for (index, sample) in samples.enumerate() {
        if let Some(plane) = planes.get_mut(index % channels) {
            plane.push(sample);
        }
    }
    planes
}

/// Decodes a JPEG XL codestream (or container) into a `DynamicImage`.
///
/// The result keeps the stream's own precision: a 16-bit stream becomes a
/// 16-bit buffer, so a decode/re-encode round trip through this module does not
/// quietly narrow the image.
pub fn decode(data: &[u8]) -> Result<DynamicImage> {
    let limits = jpxl_core::limits::Limits::default();
    let decoded =
        jpxl_decode::decode(data, &limits).map_err(|e| anyhow!("JPEG XL decode failed: {e}"))?;

    let bits = decoded.colour_bits_per_sample();
    let (width, height) = (decoded.width, decoded.height);
    let channels = decoded.num_colour_channels;
    let interleaved = decoded.interleaved_colour();

    // A stream deeper than 8 bits is widened to the 16-bit `image` types;
    // anything at 8 or below is scaled into the 8-bit ones. Both keep every
    // code point the stream carried.
    if bits > 8 {
        let shift = 16u32.saturating_sub(bits);
        let scaled: Vec<u16> = interleaved.iter().map(|&s| s << shift).collect();
        return match channels {
            1 => ImageBuffer::<Luma<u16>, _>::from_raw(width, height, scaled)
                .map(DynamicImage::ImageLuma16)
                .ok_or_else(|| anyhow!("decoded greyscale buffer did not fit {width}x{height}")),
            3 => ImageBuffer::<Rgb<u16>, _>::from_raw(width, height, scaled)
                .map(DynamicImage::ImageRgb16)
                .ok_or_else(|| anyhow!("decoded RGB buffer did not fit {width}x{height}")),
            other => Err(anyhow!("unsupported decoded channel count: {other}")),
        };
    }

    let max = ((1u32 << bits) - 1).max(1);
    let narrowed: Vec<u8> = interleaved
        .iter()
        .map(|&s| u8::try_from(u32::from(s) * 255 / max).unwrap_or(u8::MAX))
        .collect();
    match channels {
        1 => ImageBuffer::<Luma<u8>, _>::from_raw(width, height, narrowed)
            .map(DynamicImage::ImageLuma8)
            .ok_or_else(|| anyhow!("decoded greyscale buffer did not fit {width}x{height}")),
        3 => ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, narrowed)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| anyhow!("decoded RGB buffer did not fit {width}x{height}")),
        other => Err(anyhow!("unsupported decoded channel count: {other}")),
    }
}

/// Decodes a `.jxl` file.
pub fn decode_file(path: &Path) -> Result<DynamicImage> {
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read JPEG XL file: {}", path.display()))?;
    decode(&data).with_context(|| format!("Failed to decode {}", path.display()))
}

/// Whether `data` starts with a JPEG XL signature — either the naked
/// codestream `FF 0A` or a Part 2 container's `JXL ` signature box.
pub fn is_jxl(data: &[u8]) -> bool {
    if jpxl_decode::starts_with_signature(data) {
        return true;
    }
    data.len() >= 12 && data.get(4..8) == Some(&b"JXL "[..])
}

/// Estimated peak working-set bytes for one encode.
///
/// The VarDCT path holds three resident `f32` XYB planes plus the forward
/// transform's coefficient buffers and the candidate scratch the rate loop
/// prices against. That is the allocation the orchestrator's memory budget has
/// to reserve against, and it scales with pixels, not with the source's byte
/// width — a 16-bit source and an 8-bit one converge to the same float planes.
pub fn estimate_encode_peak(width: u32, height: u32, lossless: bool) -> u64 {
    let pixels = u64::from(width) * u64::from(height);
    if lossless {
        // Modular holds `i32` planes plus residual/scratch of the same order.
        pixels.saturating_mul(4 * 3 * 3).saturating_add(64 << 20)
    } else {
        // Three f32 XYB planes (12 B/px) held resident, and roughly as much
        // again across coefficients, the analysis atlas and rate-loop scratch.
        pixels.saturating_mul(12 * 3).saturating_add(96 << 20)
    }
}

/// How many encodes of this size fit in `ram_budget`, at least one.
pub fn safe_encode_concurrency(width: u32, height: u32, lossless: bool, ram_budget: u64) -> usize {
    let per_encode = estimate_encode_peak(width, height, lossless).max(1);
    usize::try_from(ram_budget / per_encode).unwrap_or(1).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp_rgb8(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                let luma = ((x * 200 / width.max(1)) + (y * 40 / height.max(1))).min(255) as u8;
                out.extend_from_slice(&[luma, luma.saturating_sub(20), luma.saturating_add(30)]);
            }
        }
        out
    }

    #[test]
    fn effort_parsing_covers_the_public_surface() {
        for name in JxlEffort::VALID_VALUES {
            assert_eq!(JxlEffort::parse(name).expect("valid").as_str(), *name);
        }
        assert!(JxlEffort::parse("placebo").is_err());
        assert!(JxlEffort::parse("balanced").is_err());
        assert!(!JxlEffort::default().is_lossless());
    }

    #[test]
    fn public_lossy_presets_use_only_production_rate_controllers() {
        assert_eq!(
            JxlEffort::Best.rate_preset(),
            jpxl_encode_policy::RateSearchPreset::Balanced
        );
        assert_eq!(
            JxlEffort::Fast.rate_preset(),
            jpxl_encode_policy::RateSearchPreset::Fast
        );
        assert_ne!(
            JxlEffort::Best.rate_preset(),
            jpxl_encode_policy::RateSearchPreset::Quality
        );
    }

    #[test]
    fn lossless_round_trips_8_bit_rgb_exactly() {
        let (width, height) = (48u32, 32u32);
        let rgb = ramp_rgb8(width, height);
        let config = JxlConfig {
            effort: JxlEffort::Lossless,
            ..JxlConfig::default()
        };
        let bytes = encode(
            &JxlImage {
                width,
                height,
                bits_per_sample: 8,
                samples: JxlSamples::Rgb8(&rgb),
            },
            &config,
        )
        .expect("lossless encode");

        let decoded = decode(&bytes).expect("decodes");
        assert_eq!(decoded.width(), width);
        assert_eq!(decoded.height(), height);
        assert_eq!(
            decoded.to_rgb8().into_raw(),
            rgb,
            "lossless must return the exact samples"
        );
    }

    #[test]
    fn lossless_round_trips_16_bit_rgb_exactly() {
        let (width, height) = (32u32, 24u32);
        let rgb: Vec<u16> = (0..(width * height * 3))
            .map(|i| (i * 7 % 65_536) as u16)
            .collect();
        let config = JxlConfig {
            effort: JxlEffort::Lossless,
            ..JxlConfig::default()
        };
        let bytes = encode(
            &JxlImage {
                width,
                height,
                bits_per_sample: 16,
                samples: JxlSamples::Rgb16(&rgb),
            },
            &config,
        )
        .expect("lossless 16-bit encode");

        let decoded = decode(&bytes).expect("decodes");
        assert_eq!(decoded.to_rgb16().into_raw(), rgb);
    }

    #[test]
    fn a_lossy_encode_declares_the_source_depth() {
        let (width, height) = (64u32, 48u32);
        let rgb: Vec<u16> = ramp_rgb8(width, height)
            .iter()
            .map(|&s| u16::from(s) << 8)
            .collect();
        let bytes = encode(
            &JxlImage {
                width,
                height,
                bits_per_sample: 16,
                samples: JxlSamples::Rgb16(&rgb),
            },
            &JxlConfig::default(),
        )
        .expect("lossy 16-bit encode");

        assert!(is_jxl(&bytes), "output must be a JPEG XL stream");
        let decoded = decode(&bytes).expect("decodes");
        // The point of the wide path: a 16-bit source comes back 16-bit.
        assert!(matches!(decoded, DynamicImage::ImageRgb16(_)));
        assert_eq!((decoded.width(), decoded.height()), (width, height));
    }

    #[test]
    fn a_mismatched_sample_count_is_rejected_before_the_encoder_sees_it() {
        let rgb = vec![0u8; 10];
        let err = encode(
            &JxlImage {
                width: 8,
                height: 8,
                bits_per_sample: 8,
                samples: JxlSamples::Rgb8(&rgb),
            },
            &JxlConfig::default(),
        )
        .expect_err("must reject");
        assert!(err.to_string().contains("sample count mismatch"));
    }
}
