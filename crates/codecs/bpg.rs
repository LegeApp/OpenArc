//! BPG encode/decode facade backed by the Rust `bpg-rs` crates.

use anyhow::{Context, Result, anyhow, bail, ensure};
use bpg_decode::{DecoderConfig, PixelLayout};
use bpg_encode::{EncoderTuning, HevcEncoder, encode_still_image};
use bpg_image::{ChromaFormat, ColorSpace, Image, Plane};
use std::path::Path;
use still265::backend::RustStillHevcEncoder;
use still265::batch;
use still265::{AqMode, DeblockMode, Effort, SaoMode, StillHevcConfig};

#[derive(Debug, Clone)]
pub struct BPGEncoderConfig {
    pub quality: i32,
    pub bit_depth: i32,
    pub lossless: i32,
    pub chroma_format: i32,
    pub encoder_type: i32,
    pub compress_level: i32,
    pub aq_mode: i32,
    pub aq_strength: f32,
    pub aq_clamp: u8,
    pub two_pass_gate: bool,
    pub color_space: i32,
    pub limited_range: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BPGError {
    Ok = 0,
    InvalidParam = -1,
    OutOfMemory = -2,
    UnsupportedFormat = -3,
    EncodeFailed = -4,
    DecodeFailed = -5,
    FileIO = -6,
    InvalidImage = -7,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BPGImageFormat {
    Gray = 0,
    RGB24,
    RGBA32,
    BGR24,
    BGRA32,
    YCbCr420P,
    YCbCr444P,
    YCbCr422P,
}

impl BPGImageFormat {
    pub const fn is_planar_yuv(self) -> bool {
        matches!(
            self,
            Self::Gray | Self::YCbCr420P | Self::YCbCr422P | Self::YCbCr444P
        )
    }

    fn chroma_dimensions(self, width: u32, height: u32) -> Result<(u32, u32)> {
        match self {
            Self::Gray => Ok((0, 0)),
            Self::YCbCr420P => Ok((width.div_ceil(2), height.div_ceil(2))),
            Self::YCbCr422P => Ok((width.div_ceil(2), height)),
            Self::YCbCr444P => Ok((width, height)),
            _ => Err(anyhow!("format {self:?} is not planar YCbCr")),
        }
    }

    const fn chroma_format(self) -> ChromaFormat {
        match self {
            Self::Gray => ChromaFormat::Gray,
            Self::YCbCr420P => ChromaFormat::Yuv420,
            Self::YCbCr422P => ChromaFormat::Yuv422,
            Self::YCbCr444P => ChromaFormat::Yuv444,
            _ => ChromaFormat::Yuv444,
        }
    }
}

pub struct NativeBPGEncoder {
    config: BPGEncoderConfig,
}

impl NativeBPGEncoder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: Self::default_config(),
        })
    }

    pub fn with_quality(quality: u8) -> Result<Self> {
        let mut encoder = Self::new()?;
        encoder.config.quality = i32::from(quality);
        Ok(encoder)
    }

    pub fn default_config() -> BPGEncoderConfig {
        BPGEncoderConfig {
            quality: 29,
            bit_depth: 8,
            lossless: 0,
            chroma_format: 1,
            encoder_type: 0,
            compress_level: 8,
            aq_mode: 0,
            aq_strength: 0.0,
            aq_clamp: 2,
            two_pass_gate: true,
            color_space: 0,
            limited_range: 0,
        }
    }

    pub fn set_config(&mut self, config: &BPGEncoderConfig) -> Result<()> {
        validate_config(config)?;
        self.config = config.clone();
        Ok(())
    }

    pub fn encode_from_file(&self, input_path: &str) -> Result<Vec<u8>> {
        let image = image::open(Path::new(input_path))
            .with_context(|| format!("failed to open image: {input_path}"))?;
        self.encode_dynamic_image(image)
    }

    pub fn encode_to_file(&self, input_path: &str, output_path: &str) -> Result<()> {
        let data = self.encode_from_file(input_path)?;
        std::fs::write(output_path, data)
            .with_context(|| format!("failed to write BPG file: {output_path}"))
    }

    pub fn encode_from_memory(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        format: BPGImageFormat,
    ) -> Result<Vec<u8>> {
        ensure!(width > 0 && height > 0, "image dimensions must be non-zero");
        let bit_depth = target_bit_depth(self.config.bit_depth)?;
        let color_space = color_space_from_config(self.config.color_space);
        let limited_range = self.config.limited_range != 0;

        let mut image = match format {
            BPGImageFormat::Gray => image_from_gray_memory(
                data,
                width,
                height,
                stride,
                bit_depth,
                color_space,
                limited_range,
            )?,
            BPGImageFormat::RGB24
            | BPGImageFormat::RGBA32
            | BPGImageFormat::BGR24
            | BPGImageFormat::BGRA32 => image_from_rgb_memory(
                data,
                width,
                height,
                stride,
                format,
                bit_depth,
                color_space,
                limited_range,
            )?,
            other => bail!("encode_from_memory does not accept planar format {other:?}"),
        };

        apply_requested_chroma(&mut image, self.config.chroma_format)?;
        self.encode_image(image)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode_from_planar_u8(
        &self,
        y_plane: &[u8],
        cb_plane: &[u8],
        cr_plane: &[u8],
        width: u32,
        height: u32,
        y_stride: u32,
        cb_stride: u32,
        cr_stride: u32,
        format: BPGImageFormat,
    ) -> Result<Vec<u8>> {
        validate_planar_inputs_u8(
            y_plane, cb_plane, cr_plane, width, height, y_stride, cb_stride, cr_stride, format,
        )?;

        let bit_depth = target_bit_depth(self.config.bit_depth)?;
        let image = planar_image(
            width,
            height,
            bit_depth,
            format.chroma_format(),
            color_space_from_config(self.config.color_space),
            self.config.limited_range != 0,
            plane_from_u8(y_plane, width, height, y_stride, bit_depth)?,
            chroma_plane_from_u8(cb_plane, width, height, cb_stride, format, bit_depth)?,
            chroma_plane_from_u8(cr_plane, width, height, cr_stride, format, bit_depth)?,
        )?;
        self.encode_image(image)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode_from_planar_u16(
        &self,
        y_plane: &[u16],
        cb_plane: &[u16],
        cr_plane: &[u16],
        width: u32,
        height: u32,
        y_stride: u32,
        cb_stride: u32,
        cr_stride: u32,
        format: BPGImageFormat,
    ) -> Result<Vec<u8>> {
        validate_planar_inputs_u16(
            y_plane, cb_plane, cr_plane, width, height, y_stride, cb_stride, cr_stride, format,
        )?;

        let bit_depth = target_bit_depth(self.config.bit_depth)?;
        let image = planar_image(
            width,
            height,
            bit_depth,
            format.chroma_format(),
            color_space_from_config(self.config.color_space),
            self.config.limited_range != 0,
            plane_from_u16(y_plane, width, height, y_stride, bit_depth)?,
            chroma_plane_from_u16(cb_plane, width, height, cb_stride, format, bit_depth)?,
            chroma_plane_from_u16(cr_plane, width, height, cr_stride, format, bit_depth)?,
        )?;
        self.encode_image(image)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode_from_ycbcr420_planar(
        &self,
        y_plane: &[u8],
        cb_plane: &[u8],
        cr_plane: &[u8],
        width: u32,
        height: u32,
        y_stride: u32,
        cb_stride: u32,
        cr_stride: u32,
    ) -> Result<Vec<u8>> {
        self.encode_from_planar_u8(
            y_plane,
            cb_plane,
            cr_plane,
            width,
            height,
            y_stride,
            cb_stride,
            cr_stride,
            BPGImageFormat::YCbCr420P,
        )
    }

    pub fn get_error(&self) -> String {
        "bpg-rs encoder error".to_string()
    }

    fn encode_dynamic_image(&self, dyn_img: image::DynamicImage) -> Result<Vec<u8>> {
        let bit_depth = target_bit_depth(self.config.bit_depth)?;
        let color_space = color_space_from_config(self.config.color_space);
        let limited_range = self.config.limited_range != 0;

        let mut image = match dyn_img {
            image::DynamicImage::ImageLuma16(_) | image::DynamicImage::ImageLumaA16(_) => {
                let luma = dyn_img.to_luma16();
                let (w, h) = luma.dimensions();
                Image::from_luma16(&luma, w, h, color_space, limited_range, bit_depth)
            }
            image::DynamicImage::ImageRgb16(_) | image::DynamicImage::ImageRgba16(_) => {
                let rgb = dyn_img.to_rgb16();
                let (w, h) = rgb.dimensions();
                Image::from_rgb16(&rgb, w, h, color_space, limited_range, bit_depth)
            }
            image::DynamicImage::ImageLuma8(_) | image::DynamicImage::ImageLumaA8(_) => {
                let luma = dyn_img.to_luma8();
                let (w, h) = luma.dimensions();
                Image::from_luma8(&luma, w, h, color_space, limited_range, bit_depth)
            }
            _ => {
                let rgb = dyn_img.to_rgb8();
                let (w, h) = rgb.dimensions();
                Image::from_rgb8(&rgb, w, h, color_space, limited_range, bit_depth)
            }
        };
        apply_requested_chroma(&mut image, self.config.chroma_format)?;
        self.encode_image(image)
    }

    /// Encode a pre-built [`bpg_image::Image`] directly.
    /// This is the low-level workhorse; use the higher-level
    /// [`encode_from_memory`]/[`encode_from_planar_u16`] etc. for normal usage.
    pub fn encode_image_data(&self, image: Image) -> Result<Vec<u8>> {
        self.encode_image(image)
    }

    fn encode_image(&self, image: Image) -> Result<Vec<u8>> {
        if self.config.lossless != 0 {
            bail!("bpg-rs lossless encode is not implemented yet");
        }

        let effort = effort_from_encoder_type(self.config.encoder_type);
        let (aq_mode, aq_strength, aq_clamp) = aq_from_config(&self.config);
        if aq_mode.is_two_pass() && !matches!(effort, Effort::Slow | Effort::Placebo) {
            bail!("BPG two-pass AQ requires slow or placebo effort");
        }

        let backend = RustStillHevcEncoder::new(effort)
            .with_sao(SaoMode::On)
            .with_deblock(DeblockMode::On)
            .with_aq(aq_mode, aq_strength, aq_clamp)
            .with_two_pass_gate(self.config.two_pass_gate);
        let caps = backend.caps();
        if !caps.supports_bit_depth(image.bit_depth) {
            bail!(
                "bpg-rs backend does not support {}-bit BPG encode",
                image.bit_depth
            );
        }
        if !caps.supports_chroma_format(image.chroma_format) {
            bail!(
                "bpg-rs backend does not support {:?} BPG encode",
                image.chroma_format
            );
        }

        let qp = self.config.quality.clamp(0, 51) as u8;
        let compress_level = self.config.compress_level.clamp(1, 9) as u8;
        encode_still_image(
            image,
            &backend,
            qp,
            compress_level,
            EncoderTuning::neutral(),
        )
        .map_err(|err| anyhow!("bpg-rs encode failed: {err}"))
    }
}

pub fn decode_file(input_path: &str) -> Result<(Vec<u8>, u32, u32, BPGImageFormat)> {
    let data = std::fs::read(input_path)
        .with_context(|| format!("failed to read BPG file: {input_path}"))?;
    let decoded = DecoderConfig::new()
        .decode(&data, PixelLayout::Rgba8)
        .map_err(|err| anyhow!("bpg-rs decode failed: {err}"))?;
    Ok((
        decoded.data,
        decoded.width,
        decoded.height,
        BPGImageFormat::RGBA32,
    ))
}

pub fn get_version() -> String {
    "bpg-rs".to_string()
}

pub fn get_supported_encoders() -> i32 {
    1
}

fn validate_config(config: &BPGEncoderConfig) -> Result<()> {
    ensure!(
        (0..=51).contains(&config.quality),
        "BPG QP must be in 0..=51"
    );
    target_bit_depth(config.bit_depth)?;
    ensure!(
        matches!(config.chroma_format, 0..=3),
        "BPG chroma format must be 0..=3"
    );
    ensure!(
        matches!(config.encoder_type, 0..=2),
        "BPG encoder type must be 0=balanced, 1=slow, or 2=placebo"
    );
    ensure!(matches!(config.aq_mode, 0..=6), "BPG AQ mode must be 0..=6");
    ensure!(
        config.aq_strength.is_finite() && config.aq_strength >= 0.0,
        "BPG AQ strength must be finite and non-negative"
    );
    Ok(())
}

fn effort_from_encoder_type(encoder_type: i32) -> Effort {
    match encoder_type {
        2 => Effort::Placebo,
        1 => Effort::Slow,
        _ => Effort::Fast,
    }
}

fn aq_from_code(code: i32) -> AqMode {
    match code {
        1 => AqMode::LegacyShrink,
        2 => AqMode::Perceptual,
        3 => AqMode::PerceptualChroma,
        4 => AqMode::PsnrProbe,
        5 => AqMode::PositiveProbe,
        6 => AqMode::TwoPassMeasured,
        _ => AqMode::Off,
    }
}

fn aq_from_config(config: &BPGEncoderConfig) -> (AqMode, f32, u8) {
    let mode = aq_from_code(config.aq_mode);
    let fallback = match mode {
        AqMode::Off => (0.0, 2),
        AqMode::LegacyShrink => (0.0, 8),
        AqMode::Perceptual | AqMode::PerceptualChroma => (0.35, 2),
        AqMode::TwoPassMeasured => (0.60, 3),
        AqMode::PsnrProbe | AqMode::PositiveProbe => (0.35, 2),
    };
    let strength = if config.aq_strength > 0.0 {
        config.aq_strength
    } else {
        fallback.0
    };
    let clamp = if config.aq_clamp > 0 {
        config.aq_clamp
    } else {
        fallback.1
    };
    (mode, strength, clamp)
}

pub fn resolve_aq_preset(name: &str) -> Option<(i32, f32, u8)> {
    let (mode, strength, clamp) = still265::aq_preset(name)?;
    let code = match mode {
        AqMode::Off => 0,
        AqMode::LegacyShrink => 1,
        AqMode::Perceptual => 2,
        AqMode::PerceptualChroma => 3,
        AqMode::PsnrProbe => 4,
        AqMode::PositiveProbe => 5,
        AqMode::TwoPassMeasured => 6,
    };
    Some((code, strength, clamp))
}

fn target_bit_depth(bit_depth: i32) -> Result<u8> {
    match bit_depth {
        8 | 10 | 12 => Ok(bit_depth as u8),
        other => bail!("bpg-rs supports 8, 10, and 12-bit encode, got {other}"),
    }
}

fn color_space_from_config(color_space: i32) -> ColorSpace {
    match color_space {
        3 => ColorSpace::YCbCrBt709,
        4 => ColorSpace::YCbCrBt2020,
        _ => ColorSpace::YCbCr,
    }
}

fn apply_requested_chroma(image: &mut Image, chroma_format: i32) -> Result<()> {
    if image.chroma_format == ChromaFormat::Gray {
        return Ok(());
    }
    match chroma_format {
        2 => image.subsample_to_422(1),
        3 => {}
        _ => image.subsample_to_420(1),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn image_from_rgb_memory(
    data: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    format: BPGImageFormat,
    bit_depth: u8,
    color_space: ColorSpace,
    limited_range: bool,
) -> Result<Image> {
    let channels = match format {
        BPGImageFormat::RGB24 | BPGImageFormat::BGR24 => 3usize,
        BPGImageFormat::RGBA32 | BPGImageFormat::BGRA32 => 4usize,
        _ => bail!("unsupported packed format {format:?}"),
    };
    let width_usize = width as usize;
    let height_usize = height as usize;
    let stride = stride as usize;
    let samples_8_stride = width_usize
        .checked_mul(channels)
        .context("packed row width overflow")?;
    let samples_16_stride = samples_8_stride
        .checked_mul(2)
        .context("packed 16-bit row width overflow")?;

    if stride >= samples_16_stride && data.len() >= stride.saturating_mul(height_usize) {
        let mut pixels = Vec::<u16>::with_capacity(width_usize * height_usize * 3);
        for row in 0..height_usize {
            let row_data = &data[row * stride..row * stride + samples_16_stride];
            for px in row_data.chunks_exact(channels * 2) {
                let first = u16::from_ne_bytes([px[0], px[1]]);
                let second = u16::from_ne_bytes([px[2], px[3]]);
                let third = u16::from_ne_bytes([px[4], px[5]]);
                let (r, g, b) = if matches!(format, BPGImageFormat::BGR24 | BPGImageFormat::BGRA32)
                {
                    (third, second, first)
                } else {
                    (first, second, third)
                };
                pixels.extend([r, g, b]);
            }
        }
        let rgb = image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::from_raw(width, height, pixels)
            .ok_or_else(|| anyhow!("invalid RGB16 buffer dimensions"))?;
        return Ok(Image::from_rgb16(
            &rgb,
            width,
            height,
            color_space,
            limited_range,
            bit_depth,
        ));
    }

    ensure!(
        stride >= samples_8_stride,
        "packed stride {stride} is too small for {width}x{channels}"
    );
    ensure!(
        data.len() >= stride.saturating_mul(height_usize),
        "packed buffer is shorter than stride * height"
    );
    let mut pixels = Vec::<u8>::with_capacity(width_usize * height_usize * 3);
    for row in 0..height_usize {
        let row_data = &data[row * stride..row * stride + samples_8_stride];
        for px in row_data.chunks_exact(channels) {
            let (r, g, b) = if matches!(format, BPGImageFormat::BGR24 | BPGImageFormat::BGRA32) {
                (px[2], px[1], px[0])
            } else {
                (px[0], px[1], px[2])
            };
            pixels.extend([r, g, b]);
        }
    }
    let rgb = image::RgbImage::from_raw(width, height, pixels)
        .ok_or_else(|| anyhow!("invalid RGB8 buffer dimensions"))?;
    Ok(Image::from_rgb8(
        &rgb,
        width,
        height,
        color_space,
        limited_range,
        bit_depth,
    ))
}

fn image_from_gray_memory(
    data: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    bit_depth: u8,
    color_space: ColorSpace,
    limited_range: bool,
) -> Result<Image> {
    let width_usize = width as usize;
    let height_usize = height as usize;
    let stride = stride as usize;
    ensure!(stride >= width_usize, "gray stride is too small");
    ensure!(
        data.len() >= stride.saturating_mul(height_usize),
        "gray buffer is shorter than stride * height"
    );
    let mut pixels = Vec::<u8>::with_capacity(width_usize * height_usize);
    for row in 0..height_usize {
        pixels.extend_from_slice(&data[row * stride..row * stride + width_usize]);
    }
    let gray = image::GrayImage::from_raw(width, height, pixels)
        .ok_or_else(|| anyhow!("invalid gray buffer dimensions"))?;
    Ok(Image::from_luma8(
        &gray,
        width,
        height,
        color_space,
        limited_range,
        bit_depth,
    ))
}

#[allow(clippy::too_many_arguments)]
fn planar_image(
    width: u32,
    height: u32,
    bit_depth: u8,
    chroma_format: ChromaFormat,
    color_space: ColorSpace,
    limited_range: bool,
    y: Plane<u16>,
    cb: Option<Plane<u16>>,
    cr: Option<Plane<u16>>,
) -> Result<Image> {
    let mut planes = vec![y];
    if chroma_format != ChromaFormat::Gray {
        planes.push(cb.context("missing Cb plane")?);
        planes.push(cr.context("missing Cr plane")?);
    }
    Ok(Image {
        width,
        height,
        bit_depth,
        chroma_format,
        color_space,
        limited_range,
        planes,
        has_alpha: false,
    })
}

fn plane_from_u8(
    data: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    bit_depth: u8,
) -> Result<Plane<u16>> {
    let samples = copy_plane(data, width, height, stride)?;
    let scaled = samples
        .into_iter()
        .map(|v| scale_sample(u16::from(v), 8, bit_depth))
        .collect();
    Ok(Plane {
        data: scaled,
        width,
        height,
        stride: width as usize,
    })
}

fn plane_from_u16(
    data: &[u16],
    width: u32,
    height: u32,
    stride: u32,
    bit_depth: u8,
) -> Result<Plane<u16>> {
    let samples = copy_plane(data, width, height, stride)?;
    let max = (1u16 << bit_depth) - 1;
    let clamped = samples.into_iter().map(|v| v.min(max)).collect();
    Ok(Plane {
        data: clamped,
        width,
        height,
        stride: width as usize,
    })
}

fn chroma_plane_from_u8(
    data: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    format: BPGImageFormat,
    bit_depth: u8,
) -> Result<Option<Plane<u16>>> {
    let (cw, ch) = format.chroma_dimensions(width, height)?;
    if cw == 0 || ch == 0 {
        return Ok(None);
    }
    Ok(Some(plane_from_u8(data, cw, ch, stride, bit_depth)?))
}

fn chroma_plane_from_u16(
    data: &[u16],
    width: u32,
    height: u32,
    stride: u32,
    format: BPGImageFormat,
    bit_depth: u8,
) -> Result<Option<Plane<u16>>> {
    let (cw, ch) = format.chroma_dimensions(width, height)?;
    if cw == 0 || ch == 0 {
        return Ok(None);
    }
    Ok(Some(plane_from_u16(data, cw, ch, stride, bit_depth)?))
}

fn copy_plane<T: Copy>(data: &[T], width: u32, height: u32, stride: u32) -> Result<Vec<T>> {
    let width = width as usize;
    let height = height as usize;
    let stride = stride as usize;
    ensure!(stride >= width, "plane stride is smaller than width");
    ensure!(
        data.len() >= stride.saturating_mul(height),
        "plane buffer is shorter than stride * height"
    );
    let mut out = Vec::with_capacity(width * height);
    for row in 0..height {
        out.extend_from_slice(&data[row * stride..row * stride + width]);
    }
    Ok(out)
}

fn scale_sample(sample: u16, in_bit_depth: u8, out_bit_depth: u8) -> u16 {
    if in_bit_depth == out_bit_depth {
        return sample;
    }
    let in_max = (1u32 << in_bit_depth) - 1;
    let out_max = (1u32 << out_bit_depth) - 1;
    ((u32::from(sample) * out_max + in_max / 2) / in_max) as u16
}

#[allow(clippy::too_many_arguments)]
fn validate_planar_inputs_u8(
    y_plane: &[u8],
    cb_plane: &[u8],
    cr_plane: &[u8],
    width: u32,
    height: u32,
    y_stride: u32,
    cb_stride: u32,
    cr_stride: u32,
    format: BPGImageFormat,
) -> Result<()> {
    validate_planar_inputs(
        y_plane.len(),
        cb_plane.len(),
        cr_plane.len(),
        width,
        height,
        y_stride,
        cb_stride,
        cr_stride,
        format,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_planar_inputs_u16(
    y_plane: &[u16],
    cb_plane: &[u16],
    cr_plane: &[u16],
    width: u32,
    height: u32,
    y_stride: u32,
    cb_stride: u32,
    cr_stride: u32,
    format: BPGImageFormat,
) -> Result<()> {
    validate_planar_inputs(
        y_plane.len(),
        cb_plane.len(),
        cr_plane.len(),
        width,
        height,
        y_stride,
        cb_stride,
        cr_stride,
        format,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_planar_inputs(
    y_len: usize,
    cb_len: usize,
    cr_len: usize,
    width: u32,
    height: u32,
    y_stride: u32,
    cb_stride: u32,
    cr_stride: u32,
    format: BPGImageFormat,
) -> Result<()> {
    ensure!(
        format.is_planar_yuv(),
        "format {format:?} is not planar YUV"
    );
    ensure!(width > 0 && height > 0, "image dimensions must be non-zero");
    ensure!(y_stride >= width, "Y stride is smaller than width");
    ensure!(
        y_len >= y_stride as usize * height as usize,
        "Y plane is shorter than y_stride * height"
    );
    let (cw, ch) = format.chroma_dimensions(width, height)?;
    if cw == 0 || ch == 0 {
        return Ok(());
    }
    ensure!(cb_stride >= cw, "Cb stride is smaller than chroma width");
    ensure!(cr_stride >= cw, "Cr stride is smaller than chroma width");
    ensure!(
        cb_len >= cb_stride as usize * ch as usize,
        "Cb plane is shorter than cb_stride * chroma_height"
    );
    ensure!(
        cr_len >= cr_stride as usize * ch as usize,
        "Cr plane is shorter than cr_stride * chroma_height"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Batch / memory-budget-aware concurrent encoding
// ---------------------------------------------------------------------------

/// A job ready for batch BPG encoding: an [`Image`] plus its config.
pub struct EncodeBatchJob {
    pub image: Image,
    pub config: BPGEncoderConfig,
}

/// Estimate how much peak memory one encode of `config` at `(w, h)` would need.
/// Uses the same CTB-alignment and worker-clone accounting as
/// `still265::batch::estimate_memory`.
pub fn estimate_bpg_memory(
    w: u32,
    h: u32,
    bit_depth: u8,
    chroma: ChromaFormat,
    effort: Effort,
) -> batch::MemoryEstimate {
    let cfg = StillHevcConfig {
        width: w,
        height: h,
        bit_depth,
        chroma,
        qp: 28,
        effort,
        sao: SaoMode::Off,
        deblock: DeblockMode::On,
        adaptive_qp: false,
        aq_mode: AqMode::Off,
        aq_strength: 0.35,
        aq_clamp: 2,
        two_pass_gate: true,
    };
    batch::estimate_memory(&cfg)
}

/// How many images of `(w, h, bit_depth, chroma)` can safely be encoded
/// concurrently within `ram_budget_bytes`. Clamped to `[1, CPU count]`.
pub fn recommended_bpg_concurrency(
    w: u32,
    h: u32,
    bit_depth: u8,
    chroma: ChromaFormat,
    effort: Effort,
    ram_budget_bytes: u64,
) -> usize {
    let cfg = StillHevcConfig {
        width: w,
        height: h,
        bit_depth,
        chroma,
        qp: 28,
        effort,
        sao: SaoMode::Off,
        deblock: DeblockMode::On,
        adaptive_qp: false,
        aq_mode: AqMode::Off,
        aq_strength: 0.35,
        aq_clamp: 2,
        two_pass_gate: true,
    };
    batch::recommended_concurrency(&cfg, ram_budget_bytes)
}

/// Encode a batch of [`EncodeBatchJob`]s concurrently, returning BPG data in
/// input order. `concurrency` caps image-level parallelism (call
/// [`recommended_bpg_concurrency`] to size it against a RAM budget).
///
/// Note: when `concurrency > 1`, set `BPG_ENC_THREADS=1` so per-image
/// multi-threading does not oversubscribe CPUs.
pub fn batch_encode_images(jobs: &[EncodeBatchJob], concurrency: usize) -> Result<Vec<Vec<u8>>> {
    let conc = concurrency.max(1).min(jobs.len());
    if conc <= 1 {
        return jobs
            .iter()
            .map(|j| {
                let mut enc = NativeBPGEncoder::new().with_context(|| "batch: create encoder")?;
                enc.set_config(&j.config)
                    .with_context(|| "batch: set config")?;
                enc.encode_image_data(j.image.clone())
            })
            .collect();
    }

    // Scoped threads: each worker picks the next unclaimed job.
    let next = std::sync::atomic::AtomicUsize::new(0);
    let n = jobs.len();
    let mut slots: Vec<Option<Vec<u8>>> = (0..n).map(|_| None).collect();
    let scope_result: Result<()> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..conc)
            .map(|_| {
                let next = &next;
                s.spawn(|| -> Result<Vec<(usize, Vec<u8>)>> {
                    let mut local: Vec<(usize, Vec<u8>)> = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if i >= n {
                            break;
                        }
                        let job = &jobs[i];
                        let mut enc = NativeBPGEncoder::new()
                            .with_context(|| format!("batch job {i}: create encoder"))?;
                        enc.set_config(&job.config)
                            .with_context(|| format!("batch job {i}: set config"))?;
                        let bpg = enc
                            .encode_image_data(job.image.clone())
                            .with_context(|| format!("batch job {i}: encode image"))?;
                        local.push((i, bpg));
                    }
                    Ok(local)
                })
            })
            .collect();
        for h in handles {
            let local = h.join().map_err(|_| anyhow!("batch worker panicked"))??;
            for (i, bpg) in local {
                slots[i] = Some(bpg);
            }
        }
        Ok(())
    });
    scope_result?;
    slots
        .into_iter()
        .map(|o| o.ok_or_else(|| anyhow!("batch worker did not produce output for a job")))
        .collect()
}

// ---------------------------------------------------------------------------
// Convenience wrappers (integer-based, no extra imports needed by callers)
// ---------------------------------------------------------------------------

/// Peak memory estimate for one BPG encode, using integer chroma/encoder codes
/// matching the orchestrator's convention:
/// - `chroma_format`: 0=4:2:0, 1=4:2:0(default), 2=4:2:2, 3=4:4:4
/// - `encoder_type`: 0=Fast/Balanced, 1=Slow, 2=Placebo
pub fn estimate_encode_peak(
    w: u32,
    h: u32,
    bit_depth: u8,
    chroma_format: i32,
    encoder_type: i32,
) -> u64 {
    use bpg_image::ChromaFormat;
    let chroma = match chroma_format {
        2 => ChromaFormat::Yuv422,
        3 => ChromaFormat::Yuv444,
        _ => ChromaFormat::Yuv420,
    };
    let effort = effort_from_encoder_type(encoder_type);
    estimate_bpg_memory(w, h, bit_depth, chroma, effort).peak_bytes
}

/// Safe concurrency for a given image size and system RAM budget.
pub fn safe_encode_concurrency(
    w: u32,
    h: u32,
    bit_depth: u8,
    chroma_format: i32,
    encoder_type: i32,
    ram_budget_bytes: u64,
) -> usize {
    use bpg_image::ChromaFormat;
    let chroma = match chroma_format {
        2 => ChromaFormat::Yuv422,
        3 => ChromaFormat::Yuv444,
        _ => ChromaFormat::Yuv420,
    };
    let effort = effort_from_encoder_type(encoder_type);
    recommended_bpg_concurrency(w, h, bit_depth, chroma, effort, ram_budget_bytes)
}

/// Build a StillHevcConfig from integer codes (for external callers).
pub fn still_hevc_config(
    w: u32,
    h: u32,
    bit_depth: u8,
    chroma_format: i32,
    encoder_type: i32,
) -> StillHevcConfig {
    use bpg_image::ChromaFormat;
    let chroma = match chroma_format {
        2 => ChromaFormat::Yuv422,
        3 => ChromaFormat::Yuv444,
        _ => ChromaFormat::Yuv420,
    };
    let effort = effort_from_encoder_type(encoder_type);
    StillHevcConfig {
        width: w,
        height: h,
        bit_depth,
        chroma,
        qp: 28,
        effort,
        sao: SaoMode::Off,
        deblock: DeblockMode::On,
        adaptive_qp: false,
        aq_mode: AqMode::Off,
        aq_strength: 0.35,
        aq_clamp: 2,
        two_pass_gate: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_qp_is_29() {
        assert_eq!(NativeBPGEncoder::default_config().quality, 29);
    }

    #[test]
    fn chroma_dimensions_match_bpg_formats() {
        assert_eq!(
            BPGImageFormat::YCbCr420P.chroma_dimensions(11, 7).unwrap(),
            (6, 4)
        );
        assert_eq!(
            BPGImageFormat::YCbCr422P.chroma_dimensions(11, 7).unwrap(),
            (6, 7)
        );
        assert_eq!(
            BPGImageFormat::YCbCr444P.chroma_dimensions(11, 7).unwrap(),
            (11, 7)
        );
    }

    #[test]
    fn encode_rgb8_smoke() {
        let encoder = NativeBPGEncoder::new().unwrap();
        let rgb = vec![128u8; 8 * 8 * 3];
        let bpg = encoder
            .encode_from_memory(&rgb, 8, 8, 8 * 3, BPGImageFormat::RGB24)
            .unwrap();
        assert!(bpg.starts_with(b"BPG"));
        assert!(bpg.len() > 8);
    }

    #[test]
    fn estimate_memory_smoke() {
        let est = estimate_bpg_memory(1920, 1080, 8, ChromaFormat::Yuv420, Effort::Slow);
        assert!(est.peak_bytes > 0);
        assert!(est.peak_mib() > 0.0);
        let conc =
            recommended_bpg_concurrency(1920, 1080, 8, ChromaFormat::Yuv420, Effort::Fast, 8 << 30);
        assert!(conc >= 1);
    }
}
