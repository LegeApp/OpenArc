//! In-process JPEG 2000 decode via vendored `openjp2` / `openjp2-tools` (same pipeline as `opj_decompress`).

use std::path::Path;

use anyhow::{Context, Result};
use image::DynamicImage;

/// Decode a JP2/J2K/… file to a [`DynamicImage`].
pub fn decode_jpeg2000_file(path: &Path) -> Result<DynamicImage> {
  openjp2_tools::decode_jpeg2000_file_to_dynamic(path)
    .map_err(|e| anyhow::anyhow!("{}", e))
    .with_context(|| format!("JPEG 2000 ({})", path.display()))
}
