// Universal Thumbnail Generation Module
// Supports BPG, standard image formats, HEIC/HEIF, RAW, DNG, and JPEG2000 files
use std::path::Path;
use std::io::BufWriter;
use std::fs::File;
use anyhow::{Result, anyhow};
use image::{DynamicImage, ImageBuffer, Rgba, imageops::FilterType};

use crate::decoder::{decode_file as decode_bpg_file, DecodedImage};
use crate::thumbnail::{ThumbnailConfig, ThumbnailGenerator};

/// Universal thumbnail generator that handles all image formats
pub struct UniversalThumbnailGenerator {
    config: ThumbnailConfig,
}

impl UniversalThumbnailGenerator {
    /// Create a new universal thumbnail generator with default settings
    pub fn new() -> Self {
        Self {
            config: ThumbnailConfig::default(),
        }
    }

    /// Create a universal thumbnail generator with custom config
    pub fn with_config(config: ThumbnailConfig) -> Self {
        Self { config }
    }

    /// Create a universal thumbnail generator with specific dimensions
    pub fn with_dimensions(max_width: u32, max_height: u32) -> Self {
        Self {
            config: ThumbnailConfig {
                max_width,
                max_height,
                ..Default::default()
            },
        }
    }

    /// Generate a thumbnail from any supported image file
    pub fn generate_thumbnail(&self, input_path: &Path) -> Result<Vec<u8>> {
        let (data, _, _) = self.generate_thumbnail_with_dimensions(input_path)?;
        Ok(data)
    }

    /// Generate a thumbnail with dimensions (avoids double-decode)
    fn generate_thumbnail_with_dimensions(&self, input_path: &Path) -> Result<(Vec<u8>, u32, u32)> {
        let file_ext = input_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        match file_ext.as_str() {
            "bpg" => self.generate_bpg_thumbnail(input_path),
            "jpg" | "jpeg" => self.generate_jpeg_thumbnail(input_path),
            "heic" | "heif" => self.generate_heic_thumbnail(input_path),
            "dng" => self.generate_dng_thumbnail(input_path),
            "jp2" | "j2k" | "j2c" | "jpc" | "jpt" | "jph" | "jhc" => self.generate_jpeg2000_thumbnail(input_path),
            "cr2" | "nef" | "arw" | "orf" | "rw2" | "raf" | "3fr" | "fff" | "dcr" | "kdc" | "srf" | "sr2" | "erf" | "mef" | "mrw" | "nrw" | "pef" | "iiq" | "x3f" => {
                self.generate_raw_thumbnail(input_path)
            }
            _ => self.generate_standard_thumbnail(input_path),
        }
    }

    /// Generate a thumbnail and save it as PNG
    pub fn generate_thumbnail_to_png(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        let (thumbnail_data, width, height) = self.generate_thumbnail_with_dimensions(input_path)?;
        
        // Use fast png crate for encoding with optimized settings
        let file = File::create(output_path)?;
        let writer = BufWriter::with_capacity(64 * 1024, file); // 64KB buffer

        let mut encoder = png::Encoder::new(writer, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        encoder.set_filter(png::FilterType::Sub); // Faster filter
        encoder.set_adaptive_filter(png::AdaptiveFilterType::NonAdaptive); // Skip filter selection

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&thumbnail_data)?;

        Ok(())
    }

    /// Generate a thumbnail and save it as JPEG (smaller files for photographic content)
    ///
    /// JPEG thumbnails are typically 3-5x smaller than PNG for photos.
    /// `quality` is 1-100; 85 is a good default for thumbnails.
    ///
    /// **Optimization**: For HEIC files, uses YCbCr fast path to avoid conversion overhead.
    pub fn generate_thumbnail_to_jpeg(
        &self,
        input_path: &Path,
        output_path: &Path,
        quality: u8,
    ) -> Result<()> {
        // Try YCbCr fast path for HEIC files
        let file_ext = input_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        if matches!(file_ext.as_str(), "heic" | "heif") {
            if let Ok(()) = self.generate_heic_thumbnail_ycbcr_fast(input_path, output_path, quality) {
                return Ok(());
            }
            // Fall through to standard path if YCbCr fast path fails
        }

        // Standard path: decode to RGBA, resize, encode to JPEG
        let (thumbnail_data, width, height) = self.generate_thumbnail_with_dimensions(input_path)?;

        // Convert RGBA → RGB (JPEG doesn't support alpha)
        let rgb_data: Vec<u8> = thumbnail_data
            .chunks_exact(4)
            .flat_map(|rgba| [rgba[0], rgba[1], rgba[2]])
            .collect();

        let rgb_image: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(width, height, rgb_data)
                .ok_or_else(|| anyhow!("Failed to create RGB image buffer for JPEG encoding"))?;

        let file = File::create(output_path)?;
        let writer = BufWriter::with_capacity(32 * 1024, file);

        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, quality);
        image::ImageEncoder::write_image(
            encoder,
            &rgb_image,
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )?;

        Ok(())
    }

    /// Fast path for HEIC→JPEG thumbnail (YCbCr→YCbCr, no RGB conversion)
    ///
    /// This avoids the YCbCr→RGB→YCbCr round-trip by keeping data in YCbCr throughout.
    /// Typically 2-3x faster than the RGB path for HEIC files.
    fn generate_heic_thumbnail_ycbcr_fast(
        &self,
        input_path: &Path,
        output_path: &Path,
        quality: u8,
    ) -> Result<()> {
        use crate::universal_decode::UniversalDecodedImage;
        use crate::image_data::ImageData;

        // Decode HEIC to YCbCr 4:2:0
        let decoded = UniversalDecodedImage::decode_file(input_path)?;

        // Only use fast path if we got YCbCr data
        let ImageData::Ycbcr420 {
            y,
            cb,
            cr,
            width,
            height,
            y_stride,
            chroma_stride,
        } = decoded.data
        else {
            return Err(anyhow!("Not YCbCr data"));
        };

        // Calculate new dimensions
        let (new_width, new_height) = self.calculate_dimensions(width, height);

        // Resize YCbCr planes using fast_image_resize
        let resized_ycbcr = self.resize_ycbcr420(
            &y,
            &cb,
            &cr,
            width,
            height,
            y_stride,
            chroma_stride,
            new_width,
            new_height,
        )?;

        // Encode directly to JPEG using toojpeg's YCbCr path
        let file = File::create(output_path)?;
        let mut writer = BufWriter::with_capacity(32 * 1024, file);

        toojpeg::encode_ycbcr420(
            &resized_ycbcr,
            toojpeg::EncodeOptions {
                width: new_width,
                height: new_height,
                format: toojpeg::ImageFormat::YCbCr420,
                quality,
                baseline: true,
                optimized: true,
                downsample: false, // Already 4:2:0
            },
            &mut writer,
        )
        .map_err(|e| anyhow!("JPEG encoding failed: {}", e))?;

        Ok(())
    }

    /// Resize YCbCr 4:2:0 data (all planes)
    ///
    /// Returns concatenated buffer: Y plane + Cb plane + Cr plane
    fn resize_ycbcr420(
        &self,
        y: &[u8],
        cb: &[u8],
        cr: &[u8],
        width: u32,
        height: u32,
        _y_stride: u32,
        _chroma_stride: u32,
        new_width: u32,
        new_height: u32,
    ) -> Result<Vec<u8>> {
        // Resize Y plane (luma - full resolution)
        let y_img = image::GrayImage::from_raw(width, height, y.to_vec())
            .ok_or_else(|| anyhow!("Failed to create Y plane image"))?;
        let y_resized = image::imageops::resize(&y_img, new_width, new_height, self.config.filter);

        // Resize Cb and Cr planes (chroma - half resolution)
        let chroma_width = (width + 1) / 2;
        let chroma_height = (height + 1) / 2;
        let new_chroma_width = (new_width + 1) / 2;
        let new_chroma_height = (new_height + 1) / 2;

        let cb_img = image::GrayImage::from_raw(chroma_width, chroma_height, cb.to_vec())
            .ok_or_else(|| anyhow!("Failed to create Cb plane image"))?;
        let cb_resized = image::imageops::resize(&cb_img, new_chroma_width, new_chroma_height, self.config.filter);

        let cr_img = image::GrayImage::from_raw(chroma_width, chroma_height, cr.to_vec())
            .ok_or_else(|| anyhow!("Failed to create Cr plane image"))?;
        let cr_resized = image::imageops::resize(&cr_img, new_chroma_width, new_chroma_height, self.config.filter);

        // Concatenate planes: Y + Cb + Cr
        let y_size = (new_width * new_height) as usize;
        let c_size = (new_chroma_width * new_chroma_height) as usize;
        let mut combined = Vec::with_capacity(y_size + 2 * c_size);

        combined.extend_from_slice(y_resized.as_raw());
        combined.extend_from_slice(cb_resized.as_raw());
        combined.extend_from_slice(cr_resized.as_raw());

        Ok(combined)
    }

    /// Generate thumbnail from BPG file
    fn generate_bpg_thumbnail(&self, input_path: &Path) -> Result<(Vec<u8>, u32, u32)> {
        // Use existing BPG thumbnail generator
        let bpg_generator = ThumbnailGenerator::with_config(self.config.clone());
        let data = bpg_generator.generate_thumbnail(input_path)?;

        // Get dimensions from decoded BPG
        let decoded = decode_bpg_file(input_path.to_str().unwrap())?;
        let (new_width, new_height) = self.calculate_dimensions(decoded.width, decoded.height);
        Ok((data, new_width, new_height))
    }

    /// Generate thumbnail from JPEG files using zune-image fork (faster decoder)
    fn generate_jpeg_thumbnail(&self, input_path: &Path) -> Result<(Vec<u8>, u32, u32)> {
        use zune_image::JpegDecoder;
        use std::io::Cursor;

        let data = std::fs::read(input_path)
            .map_err(|e| anyhow!("Failed to read JPEG file {}: {}", input_path.display(), e))?;

        let mut decoder = JpegDecoder::new(Cursor::new(&data))
            .map_err(|e| anyhow!("Failed to create JPEG decoder for {}: {}", input_path.display(), e))?;

        let (pixels, width, height) = decoder.decode_rgb()
            .map_err(|e| anyhow!("JPEG decode error for {}: {}", input_path.display(), e))?;

        // Fork's decode_rgb() returns RGB data
        // Convert RGB to RGBA
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for rgb in pixels.chunks(3) {
            if rgb.len() == 3 {
                rgba.push(rgb[0]);
                rgba.push(rgb[1]);
                rgba.push(rgb[2]);
                rgba.push(255);
            }
        }

        // Calculate new dimensions and resize
        let (new_width, new_height) = self.calculate_dimensions(width, height);
        let resized = self.resize_rgba_data(&rgba, width, height, new_width, new_height)?;
        Ok((resized, new_width, new_height))
    }

    /// Generate thumbnail from standard image formats (PNG, TIFF, etc.)
    fn generate_standard_thumbnail(&self, input_path: &Path) -> Result<(Vec<u8>, u32, u32)> {
        // Load image using the image crate
        let img = image::open(input_path)
            .map_err(|e| anyhow!("Failed to open image {}: {}", input_path.display(), e))?;

        // Calculate new dimensions
        let (orig_width, orig_height) = (img.width(), img.height());
        let (new_width, new_height) = self.calculate_dimensions(orig_width, orig_height);

        // Resize the image
        let resized = img.resize_exact(new_width, new_height, self.config.filter);

        // Convert to RGBA8 and return raw data with dimensions
        Ok((resized.to_rgba8().into_raw(), new_width, new_height))
    }

    /// Generate thumbnail from HEIC/HEIF files
    fn generate_heic_thumbnail(&self, input_path: &Path) -> Result<(Vec<u8>, u32, u32)> {
        let decoded = codecs::heic::decode_heic_file(input_path)?;

        let mut rgba = Vec::with_capacity(decoded.width as usize * decoded.height as usize * 4);
        if decoded.has_alpha {
            // Data is RGBA interleaved
            rgba.extend_from_slice(&decoded.data);
        } else {
            // Data is RGB interleaved
            for rgb in decoded.data.chunks(3) {
                if rgb.len() == 3 {
                    rgba.push(rgb[0]);
                    rgba.push(rgb[1]);
                    rgba.push(rgb[2]);
                    rgba.push(255);
                }
            }
        }

        let (new_width, new_height) = self.calculate_dimensions(decoded.width, decoded.height);
        let resized = self.resize_rgba_data(&rgba, decoded.width, decoded.height, new_width, new_height)?;
        Ok((resized, new_width, new_height))
    }

    /// Generate thumbnail from RAW files
    fn generate_raw_thumbnail(&self, input_path: &Path) -> Result<(Vec<u8>, u32, u32)> {
        use rawloader::RawLoader;

        // Try to load RAW file
        let raw = RawLoader::new().decode_file(input_path)
            .map_err(|e| anyhow!("Failed to decode RAW file {}: {}", input_path.display(), e))?;

        // Get image data
        let width = raw.width;
        let height = raw.height;

        // Convert to RGBA (simple demosaicing)
        let mut rgba_data = vec![0u8; (width * height * 4) as usize];

        match &raw.data {
            rawloader::RawImageData::Float(data) => {
                for (i, &value) in data.iter().enumerate() {
                    let pixel_value = (value * 255.0) as u8;
                    rgba_data[i * 4] = pixel_value;
                    rgba_data[i * 4 + 1] = pixel_value;
                    rgba_data[i * 4 + 2] = pixel_value;
                    rgba_data[i * 4 + 3] = 255;
                }
            }
            rawloader::RawImageData::Integer(data) => {
                for (i, &value) in data.iter().enumerate() {
                    let pixel_value = (value >> 8) as u8; // Convert from 16-bit to 8-bit
                    rgba_data[i * 4] = pixel_value;
                    rgba_data[i * 4 + 1] = pixel_value;
                    rgba_data[i * 4 + 2] = pixel_value;
                    rgba_data[i * 4 + 3] = 255;
                }
            }
        }

        // Calculate new dimensions and resize
        let (new_width, new_height) = self.calculate_dimensions(width as u32, height as u32);
        let resized = self.resize_rgba_data(&rgba_data, width as u32, height as u32, new_width, new_height)?;
        Ok((resized, new_width, new_height))
    }

    /// Generate thumbnail from DNG files
    fn generate_dng_thumbnail(&self, input_path: &Path) -> Result<(Vec<u8>, u32, u32)> {
        // Prefer embedded JPEG preview when present (much faster and more accurate)
        if let Ok(preview) = self.try_decode_dng_embedded_jpeg_preview(input_path) {
            return self.generate_standard_thumbnail_from_dynamic_image(&preview);
        }

        // Fallback to RAW decode
        self.generate_raw_thumbnail(input_path)
    }

    fn try_decode_dng_embedded_jpeg_preview(&self, input_path: &Path) -> Result<DynamicImage> {
        use std::fs::File;
        use std::io::{Read, Seek};
        use dng::DngReader;
        use dng::ifd::IfdPath;
        use dng::tags::ifd;

        let file = File::open(input_path)
            .map_err(|e| anyhow!("Failed to open DNG {}: {}", input_path.display(), e))?;

        let reader = DngReader::read(file)
            .map_err(|e| anyhow!("Failed to parse DNG {}: {}", input_path.display(), e))?;

        // Standard TIFF/EXIF embedded thumbnail
        let path = IfdPath::default().chain_tag(ifd::JPEGInterchangeFormat);
        let entry = reader
            .get_entry_by_path(&path)
            .ok_or_else(|| anyhow!("No embedded JPEG preview"))?;

        let len = reader
            .needed_buffer_size_for_offsets(entry)
            .map_err(|e| anyhow!("Failed reading DNG preview length: {}", e))?;

        let mut buf = vec![0u8; len];
        reader
            .read_offsets_to_buffer(entry, &mut buf)
            .map_err(|e| anyhow!("Failed reading DNG preview bytes: {}", e))?;

        image::load_from_memory(&buf)
            .map_err(|e| anyhow!("Failed decoding embedded DNG JPEG preview: {}", e))
    }

    fn generate_standard_thumbnail_from_dynamic_image(&self, img: &DynamicImage) -> Result<(Vec<u8>, u32, u32)> {
        let (orig_width, orig_height) = (img.width(), img.height());
        let (new_width, new_height) = self.calculate_dimensions(orig_width, orig_height);
        let resized = img.resize_exact(new_width, new_height, self.config.filter);
        Ok((resized.to_rgba8().into_raw(), new_width, new_height))
    }

    fn generate_jpeg2000_thumbnail(&self, input_path: &Path) -> Result<(Vec<u8>, u32, u32)> {
        use openjp2::{Codec, CODEC_FORMAT, Stream};
        use openjp2::openjpeg::opj_set_default_decoder_parameters;

        let format = match openjp2::detect_format_from_extension(input_path.extension()) {
            Ok(openjp2::J2KFormat::JP2) => CODEC_FORMAT::OPJ_CODEC_JP2,
            Ok(openjp2::J2KFormat::J2K) => CODEC_FORMAT::OPJ_CODEC_J2K,
            Ok(openjp2::J2KFormat::JPT) => CODEC_FORMAT::OPJ_CODEC_JPT,
            Err(_) => CODEC_FORMAT::OPJ_CODEC_J2K,
        };

        let mut stream = Stream::new_file(input_path, 1 << 20, true)
            .map_err(|e| anyhow!("Failed to open JPEG2000 {}: {}", input_path.display(), e))?;

        let mut codec = Codec::new_decoder(format)
            .ok_or_else(|| anyhow!("Failed to create JPEG2000 decoder"))?;

        let mut params = openjp2::opj_dparameters_t::default();
        unsafe { opj_set_default_decoder_parameters(&mut params) };
        if codec.setup_decoder(&mut params) == 0 {
            return Err(anyhow!("JPEG2000 setup_decoder failed"));
        }

        let mut img = codec
            .read_header(&mut stream)
            .ok_or_else(|| anyhow!("JPEG2000 read_header failed"))?;

        if codec.decode(&mut stream, &mut img) == 0 {
            return Err(anyhow!("JPEG2000 decode failed"));
        }
        let _ = codec.end_decompress(&mut stream);

        // Convert decoded components to RGBA8
        let comps = img
            .comps_data_iter()
            .ok_or_else(|| anyhow!("JPEG2000: no components"))?
            .collect::<Vec<_>>();

        if comps.is_empty() {
            return Err(anyhow!("JPEG2000: empty components"));
        }

        let (w, h, prec) = img.comp0_dims_prec();
        if w == 0 || h == 0 {
            return Err(anyhow!("JPEG2000: invalid dimensions"));
        }

        let max_val = if prec > 0 && prec < 31 { (1i64 << prec) - 1 } else { 255 };
        let scale = 255.0 / max_val as f64;
        let pixel_count = w * h;

        let mut rgba = Vec::with_capacity(pixel_count * 4);

        let comp_to_u8 = |comp: &openjp2::image::ImageCompRef<'_>, i: usize| -> u8 {
            let v = comp.data[i] + comp.adjust;
            let v = (v as f64 * scale).round();
            v.clamp(0.0, 255.0) as u8
        };

        if comps.len() >= 3 {
            for i in 0..pixel_count {
                rgba.push(comp_to_u8(&comps[0], i));
                rgba.push(comp_to_u8(&comps[1], i));
                rgba.push(comp_to_u8(&comps[2], i));
                if comps.len() >= 4 {
                    rgba.push(comp_to_u8(&comps[3], i));
                } else {
                    rgba.push(255);
                }
            }
        } else {
            for i in 0..pixel_count {
                let g = comp_to_u8(&comps[0], i);
                rgba.push(g);
                rgba.push(g);
                rgba.push(g);
                rgba.push(255);
            }
        }

        let (new_width, new_height) = self.calculate_dimensions(w as u32, h as u32);
        let resized = self.resize_rgba_data(&rgba, w as u32, h as u32, new_width, new_height)?;
        Ok((resized, new_width, new_height))
    }

    fn get_jpeg2000_dimensions(&self, input_path: &Path) -> Result<(u32, u32)> {
        use openjp2::{Codec, CODEC_FORMAT, Stream};
        use openjp2::openjpeg::opj_set_default_decoder_parameters;

        let format = match openjp2::detect_format_from_extension(input_path.extension()) {
            Ok(openjp2::J2KFormat::JP2) => CODEC_FORMAT::OPJ_CODEC_JP2,
            Ok(openjp2::J2KFormat::J2K) => CODEC_FORMAT::OPJ_CODEC_J2K,
            Ok(openjp2::J2KFormat::JPT) => CODEC_FORMAT::OPJ_CODEC_JPT,
            Err(_) => CODEC_FORMAT::OPJ_CODEC_J2K,
        };

        let mut stream = Stream::new_file(input_path, 1 << 20, true)
            .map_err(|e| anyhow!("Failed to open JPEG2000 {}: {}", input_path.display(), e))?;

        let mut codec = Codec::new_decoder(format)
            .ok_or_else(|| anyhow!("Failed to create JPEG2000 decoder"))?;

        let mut params = openjp2::opj_dparameters_t::default();
        unsafe { opj_set_default_decoder_parameters(&mut params) };
        if codec.setup_decoder(&mut params) == 0 {
            return Err(anyhow!("JPEG2000 setup_decoder failed"));
        }

        let img = codec
            .read_header(&mut stream)
            .ok_or_else(|| anyhow!("JPEG2000 read_header failed"))?;

        let (w, h, _) = img.comp0_dims_prec();
        if w == 0 || h == 0 {
            return Err(anyhow!("JPEG2000: invalid dimensions"));
        }

        Ok((w as u32, h as u32))
    }

    /// Calculate thumbnail dimensions while maintaining aspect ratio
    fn calculate_dimensions(&self, orig_width: u32, orig_height: u32) -> (u32, u32) {
        let scale_x = self.config.max_width as f32 / orig_width as f32;
        let scale_y = self.config.max_height as f32 / orig_height as f32;
        let scale = scale_x.min(scale_y).min(1.0); // Don't upscale

        let new_width = (orig_width as f32 * scale) as u32;
        let new_height = (orig_height as f32 * scale) as u32;

        (new_width.max(1), new_height.max(1))
    }

    /// Resize RGBA image data using the image crate
    fn resize_rgba_data(
        &self,
        data: &[u8],
        src_w: u32,
        src_h: u32,
        dst_w: u32,
        dst_h: u32,
    ) -> Result<Vec<u8>> {
        // Create an ImageBuffer from the raw RGBA data
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(src_w, src_h, data.to_vec())
                .ok_or_else(|| anyhow!("Failed to create image buffer"))?;

        // Convert to DynamicImage for resizing
        let dynamic_img = DynamicImage::ImageRgba8(img);

        // Resize using the configured filter
        let resized = dynamic_img.resize_exact(dst_w, dst_h, self.config.filter);

        // Convert back to raw RGBA data
        Ok(resized.to_rgba8().into_raw())
    }

    /// Get the expected thumbnail dimensions for a given input file
    fn get_thumbnail_dimensions(&self, input_path: &Path) -> Result<(u32, u32)> {
        let file_ext = input_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (orig_width, orig_height) = match file_ext.as_str() {
            "bpg" => {
                let decoded = decode_bpg_file(input_path.to_str().unwrap())?;
                (decoded.width, decoded.height)
            }
            "heic" | "heif" => {
                let decoded = codecs::heic::decode_heic_file(input_path)?;
                (decoded.width, decoded.height)
            }
            "dng" => {
                if let Ok(preview) = self.try_decode_dng_embedded_jpeg_preview(input_path) {
                    (preview.width(), preview.height())
                } else {
                    // Fallback: try image crate (will likely fail) or let caller handle error
                    let img = image::open(input_path)?;
                    (img.width(), img.height())
                }
            }
            "jp2" | "j2k" | "j2c" | "jpc" | "jpt" | "jph" | "jhc" => self.get_jpeg2000_dimensions(input_path)?,
            _ => {
                let img = image::open(input_path)?;
                (img.width(), img.height())
            }
        };

        Ok(self.calculate_dimensions(orig_width, orig_height))
    }

    /// Check if a file extension is supported
    pub fn is_supported_format(file_path: &Path) -> bool {
        let file_ext = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        matches!(file_ext.as_str(),
            // BPG
            "bpg" |
            // Standard formats
            "jpg" | "jpeg" | "png" | "tiff" | "tif" | "bmp" | "webp" | "gif" | "ico" |
            "pnm" | "pbm" | "pgm" | "ppm" | "pam" | "dds" | "tga" |
            "hdr" | "exr" |
            // HEIC/HEIF
            "heic" | "heif" |
            // RAW formats
            "cr2" | "nef" | "arw" | "orf" | "rw2" | "raf" | "3fr" | "fff" | "dcr" |
            "kdc" | "srf" | "sr2" | "erf" | "mef" | "mrw" | "nrw" | "pef" | "iiq" | "x3f" |
            // DNG
            "dng" |
            // JPEG2000
            "jp2" | "j2k" | "j2c" | "jpc" | "jpt" | "jph" | "jhc"
        )
    }
}

impl Default for UniversalThumbnailGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_formats() {
        assert!(UniversalThumbnailGenerator::is_supported_format(Path::new("test.jpg")));
        assert!(UniversalThumbnailGenerator::is_supported_format(Path::new("test.bpg")));
        assert!(UniversalThumbnailGenerator::is_supported_format(Path::new("test.cr2")));
        assert!(UniversalThumbnailGenerator::is_supported_format(Path::new("test.dng")));
        assert!(UniversalThumbnailGenerator::is_supported_format(Path::new("test.heic")));
        assert!(!UniversalThumbnailGenerator::is_supported_format(Path::new("test.txt")));
    }

    #[test]
    fn test_calculate_dimensions() {
        let generator = UniversalThumbnailGenerator::with_dimensions(100, 100);

        // Test landscape
        let (w, h) = generator.calculate_dimensions(200, 100);
        assert_eq!(w, 100);
        assert_eq!(h, 50);

        // Test portrait
        let (w, h) = generator.calculate_dimensions(100, 200);
        assert_eq!(w, 50);
        assert_eq!(h, 100);

        // Test square
        let (w, h) = generator.calculate_dimensions(200, 200);
        assert_eq!(w, 100);
        assert_eq!(h, 100);

        // Test no upscaling
        let (w, h) = generator.calculate_dimensions(50, 50);
        assert_eq!(w, 50);
        assert_eq!(h, 50);
    }
}
