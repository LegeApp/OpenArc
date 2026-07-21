//! In-process JPEG 2000 encode/decode via `jp2lam`.

use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use image::{DynamicImage, GrayImage, RgbImage};
use jp2lam::{
    ColorSpace, Component, EncodeOptions, Image, ImageView, OutputFormat, ResourceLimits,
};

const JP2_MAX_WORKING_MEMORY: usize = 256 * 1024 * 1024;
const JP2_ENCODED_STORE_MEMORY: usize = 64 * 1024 * 1024;

/// Encode a [`DynamicImage`] to JP2 bytes using the current bounded-memory
/// `jp2lam` path. Prefer [`encode_dynamic_image_to_jpeg2000_file`] when the
/// caller is writing a file, because it avoids retaining the full output in RAM.
pub fn encode_dynamic_image_to_jpeg2000(image: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    encode_dynamic_image_to_writer(image, quality, None, &mut output)?;
    Ok(output)
}

/// Encode directly to a JP2 file using zero-copy source views where possible,
/// automatic tiling, one jp2lam worker (OpenArc already parallelizes images),
/// and spill-to-disk once encoded block payloads exceed 64 MiB.
pub fn encode_dynamic_image_to_jpeg2000_file(
    image: &DynamicImage,
    quality: u8,
    output_path: &Path,
) -> Result<u64> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(output_path)
        .with_context(|| format!("failed to create JPEG 2000 file: {}", output_path.display()))?;
    let mut writer = BufWriter::new(file);
    let spill_directory = output_path.parent().map(Path::to_path_buf);
    encode_dynamic_image_to_writer(image, quality, spill_directory, &mut writer)?;
    writer
        .flush()
        .with_context(|| format!("failed to flush JPEG 2000 file: {}", output_path.display()))?;
    drop(writer);
    Ok(std::fs::metadata(output_path)?.len())
}

fn encode_dynamic_image_to_writer<W: Write>(
    image: &DynamicImage,
    quality: u8,
    spill_directory: Option<std::path::PathBuf>,
    writer: &mut W,
) -> Result<()> {
    let mut options = EncodeOptions::photo(quality, OutputFormat::Jp2);
    options.resource_limits = ResourceLimits {
        max_working_memory: Some(JP2_MAX_WORKING_MEMORY),
        max_threads: Some(1),
        encoded_store_memory_limit: Some(JP2_ENCODED_STORE_MEMORY),
        spill_directory,
    };

    match image {
        DynamicImage::ImageLuma8(gray) => encode_view(
            ImageView::from_gray8(gray.width(), gray.height(), gray.as_raw())?,
            &options,
            writer,
        ),
        DynamicImage::ImageRgb8(rgb) => encode_view(
            ImageView::from_rgb8_interleaved(rgb.width(), rgb.height(), rgb.as_raw())?,
            &options,
            writer,
        ),
        DynamicImage::ImageLuma16(gray) => encode_view(
            ImageView::from_gray16(gray.width(), gray.height(), gray.as_raw(), 16)?,
            &options,
            writer,
        ),
        DynamicImage::ImageRgb16(rgb) => encode_view(
            ImageView::from_rgb16_interleaved(rgb.width(), rgb.height(), rgb.as_raw(), 16)?,
            &options,
            writer,
        ),
        _ => {
            let rgb = image.to_rgb8();
            encode_view(
                ImageView::from_rgb8_interleaved(rgb.width(), rgb.height(), rgb.as_raw())?,
                &options,
                writer,
            )
        }
    }
}

fn encode_view<W: Write>(
    view: ImageView<'_>,
    options: &EncodeOptions,
    writer: &mut W,
) -> Result<()> {
    jp2lam::encode_view_to_writer(view, options, writer).map_err(|err| anyhow!("{err}"))
}

/// Decode a JP2/J2K/… file to a [`DynamicImage`].
pub fn decode_jpeg2000_file(path: &Path) -> Result<DynamicImage> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("failed to open JPEG 2000 file: {}", path.display()))?;
    let image = jp2lam::decode_from_reader(&mut file)
        .map_err(|err| anyhow!("{err}"))
        .with_context(|| format!("JPEG 2000 ({})", path.display()))?;

    jp2lam_image_to_dynamic(image)
        .with_context(|| format!("JPEG 2000 image conversion ({})", path.display()))
}

fn jp2lam_image_to_dynamic(image: Image) -> Result<DynamicImage> {
    match image.colorspace {
        ColorSpace::Gray => {
            let component = only_component(&image)?;
            let pixels = component_to_u8(component, image.width, image.height)?;
            let buffer = GrayImage::from_raw(image.width, image.height, pixels)
                .ok_or_else(|| anyhow!("failed to create grayscale image buffer"))?;
            Ok(DynamicImage::ImageLuma8(buffer))
        }
        ColorSpace::Rgb | ColorSpace::Srgb => {
            if image.components.len() != 3 {
                bail!(
                    "expected 3 components for RGB JPEG 2000 image, got {}",
                    image.components.len()
                );
            }
            let pixel_count = pixel_count(image.width, image.height)?;
            let mut pixels = Vec::with_capacity(pixel_count * 3);
            for index in 0..pixel_count {
                pixels.push(sample_to_u8(image.components[0].data[index]));
                pixels.push(sample_to_u8(image.components[1].data[index]));
                pixels.push(sample_to_u8(image.components[2].data[index]));
            }
            let buffer = RgbImage::from_raw(image.width, image.height, pixels)
                .ok_or_else(|| anyhow!("failed to create RGB image buffer"))?;
            Ok(DynamicImage::ImageRgb8(buffer))
        }
        other => bail!("unsupported jp2lam colorspace for image crate conversion: {other:?}"),
    }
}

fn only_component(image: &Image) -> Result<&Component> {
    match image.components.as_slice() {
        [component] => Ok(component),
        components => bail!(
            "expected 1 component for grayscale JPEG 2000 image, got {}",
            components.len()
        ),
    }
}

fn component_to_u8(component: &Component, width: u32, height: u32) -> Result<Vec<u8>> {
    let expected = pixel_count(width, height)?;
    if component.data.len() != expected {
        bail!(
            "component dimensions {}x{} imply {expected} samples, got {}",
            width,
            height,
            component.data.len()
        );
    }

    Ok(component
        .data
        .iter()
        .map(|&sample| sample_to_u8(sample))
        .collect())
}

fn pixel_count(width: u32, height: u32) -> Result<usize> {
    let width = usize::try_from(width).context("image width does not fit in usize")?;
    let height = usize::try_from(height).context("image height does not fit in usize")?;
    width
        .checked_mul(height)
        .ok_or_else(|| anyhow!("image dimensions overflow usize"))
}

fn sample_to_u8(sample: i32) -> u8 {
    sample.clamp(0, u8::MAX as i32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_writer_path_round_trips_rgb_dimensions() {
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(
                4,
                3,
                vec![
                    255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, //
                    32, 64, 96, 96, 64, 32, 10, 20, 30, 220, 180, 140, //
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 200, 210, 220,
                ],
            )
            .expect("valid RGB image"),
        );
        let temp = tempfile::NamedTempFile::new().expect("temp file");
        let size = encode_dynamic_image_to_jpeg2000_file(&image, 85, temp.path())
            .expect("JP2 encode should succeed");
        assert!(size > 0);

        let decoded = decode_jpeg2000_file(temp.path()).expect("JP2 decode should succeed");
        assert_eq!(decoded.width(), 4);
        assert_eq!(decoded.height(), 3);
    }
}
