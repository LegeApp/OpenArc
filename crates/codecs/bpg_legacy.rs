//! Read-only BPG support, kept so archives written before the JPEG XL switch
//! stay extractable.
//!
//! OpenArc no longer *produces* BPG: every image output is JPEG XL (see
//! [`crate::jxl`]). But an archive is a promise about the past as much as the
//! present, and an `.oarc` created by an earlier build has `.bpg` entries under
//! `media/` that extraction still has to turn back into pictures. That is the
//! whole job of this module — decode, never encode.
//!
//! The encoder-side modules (`bpg_rs.rs`, `bpg_c.rs`, the vendored libbpg/x265
//! tree and the build script that compiled it) were retired to `unused/bpg/`
//! rather than deleted, so the history is recoverable if the decision is ever
//! revisited.

use anyhow::{anyhow, Context, Result};
use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
use std::path::Path;

/// Decodes a BPG codestream to an 8-bit RGB or RGBA image.
///
/// `bpg-decode` renders to 8-bit interleaved output, so a >8-bit BPG is
/// narrowed here. That is a real loss relative to the original — but it is the
/// loss the pre-existing extraction path already took, and these are legacy
/// archives being read, not new ones being written.
pub fn decode(data: &[u8]) -> Result<DynamicImage> {
    let info = bpg_decode::ImageInfo::from_bytes(data)
        .map_err(|e| anyhow!("not a readable BPG stream: {e}"))?;
    let layout = if info.has_alpha {
        bpg_decode::PixelLayout::Rgba8
    } else {
        bpg_decode::PixelLayout::Rgb8
    };

    let decoder = bpg_decode::DecoderConfig::new();
    let output = decoder
        .decode(data, layout)
        .map_err(|e| anyhow!("BPG decode failed: {e}"))?;

    let (width, height) = (output.width, output.height);
    match output.layout {
        bpg_decode::PixelLayout::Rgb8 => ImageBuffer::<Rgb<u8>, _>::from_raw(
            width,
            height,
            output.data,
        )
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| anyhow!("decoded BPG buffer did not fit {width}x{height}")),
        bpg_decode::PixelLayout::Rgba8 => ImageBuffer::<Rgba<u8>, _>::from_raw(
            width,
            height,
            output.data,
        )
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| anyhow!("decoded BPG buffer did not fit {width}x{height}")),
        other => Err(anyhow!("unexpected BPG pixel layout: {other:?}")),
    }
}

/// Decodes a `.bpg` file.
pub fn decode_file(path: &Path) -> Result<DynamicImage> {
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read BPG file: {}", path.display()))?;
    decode(&data).with_context(|| format!("Failed to decode {}", path.display()))
}

/// Whether `data` starts with the BPG magic (`B P G \xFB`).
pub fn is_bpg(data: &[u8]) -> bool {
    data.starts_with(&[0x42, 0x50, 0x47, 0xFB])
}
