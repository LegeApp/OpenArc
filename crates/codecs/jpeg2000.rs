//! In-process JPEG 2000 decode via vendored `jp2lam`.

use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use image::{DynamicImage, GrayImage, RgbImage};
use jp2lam::{ColorSpace, Component, Image};

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

    Ok(component.data.iter().map(|&sample| sample_to_u8(sample)).collect())
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
