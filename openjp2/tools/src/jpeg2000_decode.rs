//! Shared JPEG 2000 decode path for CLI (`opj_decompress`) and library consumers (e.g. OpenArc).

use crate::color::{
  color_apply_icc_profile, color_cielab_to_rgb, color_cmyk_to_rgb, color_esycc_to_rgb,
  color_sycc_to_rgb,
};
use crate::convert::{convert_to_dynamic_image, ImageError};
use crate::params::{DecompressParameters, PrecisionMode};
use image::DynamicImage;
use openjp2::{
  detect_format_from_file,
  image::opj_image,
  openjpeg::*,
  Codec, ICCProfile, J2KFormat, Stream,
};
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::path::Path;
use std::ptr;
use std::env;

extern "C" fn info_callback(msg: *const c_char, _data: *mut c_void) {
  unsafe {
    print!("[INFO] {}", CStr::from_ptr(msg).to_string_lossy());
  }
}

extern "C" fn warning_callback(msg: *const c_char, _data: *mut c_void) {
  unsafe {
    print!("[WARNING] {}", CStr::from_ptr(msg).to_string_lossy());
  }
}

extern "C" fn error_callback(msg: *const c_char, _data: *mut c_void) {
  unsafe {
    print!("[ERROR] {}", CStr::from_ptr(msg).to_string_lossy());
  }
}

/// Decode a JPEG 2000 file to a [`DynamicImage`] (same pipeline as `opj_decompress`, without a subprocess).
pub fn decode_jpeg2000_file_to_dynamic(path: &Path) -> Result<DynamicImage, ImageError> {
  let mut params = DecompressParameters::default();
  params.codec_format = detect_format_from_file(path).ok();
  params.quiet = true;
  decompress_to_dynamic(path, &params)
}

fn decompress_to_dynamic(
  input: &Path,
  params: &DecompressParameters,
) -> Result<DynamicImage, ImageError> {
  let mut image = decode_file_to_image(input, params)?;
  postprocess_decoded_image(&mut image, params)?;
  convert_to_dynamic_image(&image)
}

/// Full decompress used by the `opj_decompress` binary (writes `output` using `save_image`).
pub fn decompress_image<P: AsRef<Path>>(
  input: P,
  output: P,
  params: &DecompressParameters,
) -> Result<(), ImageError> {
  let input = input.as_ref();
  let output = output.as_ref();
  let mut image = decode_file_to_image(input, params)?;
  postprocess_decoded_image(&mut image, params)?;
  crate::convert::save_image(image.as_mut(), output, params.split_pnm)?;
  Ok(())
}

fn decode_file_to_image(
  input: &Path,
  params: &DecompressParameters,
) -> Result<Box<opj_image>, ImageError> {
  let cod_format = match params.codec_format {
    Some(J2KFormat::J2K) => OPJ_CODEC_J2K,
    Some(J2KFormat::JP2) => OPJ_CODEC_JP2,
    Some(J2KFormat::JPT) => OPJ_CODEC_JPT,
    None => {
      return Err(ImageError::InvalidFormat(
        "No codec format specified".into(),
      ));
    }
  };
  let mut codec = Codec::new_decoder(cod_format)
    .ok_or_else(|| ImageError::EncodeError("Failed to create codec".into()))?;

  let mut d_params = params.to_c_params();

  let set_decoded_resolution_factor =
    env::var("USE_OPJ_SET_DECODED_RESOLUTION_FACTOR")
      .ok()
      .map(|_| {
        let cp_reduce = d_params.cp_reduce;
        d_params.cp_reduce = 0;
        cp_reduce
      });

  if !params.quiet {
    codec.set_info_handler(Some(info_callback), ptr::null_mut());
    codec.set_warning_handler(Some(warning_callback), ptr::null_mut());
    codec.set_error_handler(Some(error_callback), ptr::null_mut());
  }

  let status = codec.setup_decoder(&mut d_params);
  if status == 0 {
    return Err(ImageError::EncodeError("Failed to setup decoder".into()));
  }

  if params.allow_partial {
    if codec.decoder_set_strict_mode(0) == 0 {
      return Err(ImageError::EncodeError("Failed to set strict mode".into()));
    }
  }

  let mut stream = Stream::new_file(input, 1_000_000, true)?;

  let mut image = codec
    .read_header(&mut stream)
    .ok_or_else(|| ImageError::DecodeError("Failed to read header".into()))?;

  if params.numcomps > 0 {
    if codec.set_decoded_components(&params.comps_indices, 0) == 0 {
      return Err(ImageError::DecodeError(
        "Failed to set decoded components".into(),
      ));
    }
  }
  if let Some(cp_reduce) = set_decoded_resolution_factor {
    if codec.set_decoded_resolution_factor(cp_reduce) == 0 {
      return Err(ImageError::DecodeError(
        "Failed to set decoded resolution factor".into(),
      ));
    }
  }

  let no_decode_area =
    params.da_x0 == 0 && params.da_y0 == 0 && params.da_x1 == 0 && params.da_y1 == 0;

  if let Some(tile_index) = params.tile_index {
    if !no_decode_area && !params.quiet {
      eprintln!("WARNING: -d option is ignored when decoding tiles");
    }
    if codec.get_decoded_tile(&mut stream, &mut image, tile_index) == 0 {
      return Err(ImageError::DecodeError(
        "Failed to set decoded tiles".into(),
      ));
    }
  } else {
    if env::var("SKIP_OPJ_SET_DECODE_AREA").is_ok() && no_decode_area {
    } else if codec.set_decode_area(
      &mut image,
      params.da_x0 as i32,
      params.da_y0 as i32,
      params.da_x1 as i32,
      params.da_y1 as i32,
    ) == 0
    {
      return Err(ImageError::DecodeError("Failed to set decode area".into()));
    }

    let status =
      codec.decode(&mut stream, &mut image) == 1 && codec.end_decompress(&mut stream) == 1;
    if !status {
      return Err(ImageError::DecodeError("Failed to decode image".into()));
    }
  }

  drop(stream);
  Ok(image)
}

fn postprocess_decoded_image(
  image: &mut Box<opj_image>,
  params: &DecompressParameters,
) -> Result<(), ImageError> {
  log::debug!(
    "Image: color_space: {:?}, numcomps: {}",
    image.color_space,
    image.numcomps
  );
  let comps = image
    .comps()
    .ok_or_else(|| ImageError::DecodeError("No components".into()))?;
  if image.color_space != OPJ_CLRSPC_SYCC
    && image.numcomps == 3
    && comps[0].dx == comps[0].dy
    && comps[1].dx != 1
  {
    image.color_space = OPJ_CLRSPC_SYCC;
  } else if image.numcomps <= 2 {
    image.color_space = OPJ_CLRSPC_GRAY;
  }

  log::debug!(
    "Image: 1 color_space: {:?}, numcomps: {}",
    image.color_space,
    image.numcomps
  );

  if image.color_space == OPJ_CLRSPC_SYCC {
    log::debug!("Converting SYCC to RGB");
    color_sycc_to_rgb(image.as_mut());
  } else if image.color_space == OPJ_CLRSPC_CMYK {
    log::debug!("Converting CMYK to RGB");
    color_cmyk_to_rgb(image.as_mut());
  } else if image.color_space == OPJ_CLRSPC_EYCC {
    log::debug!("Converting eYCC to RGB");
    color_esycc_to_rgb(image.as_mut());
  }

  if let Some(profile) = image.take_icc_profile() {
    match profile {
      ICCProfile::ICC(profile) => {
        log::debug!("Applying ICC profile");
        color_apply_icc_profile(image.as_mut(), &profile);
      }
      ICCProfile::CIELab(profile) => {
        log::debug!("Applying cielab to RGB");
        color_cielab_to_rgb(image.as_mut(), &profile);
      }
    }
  }

  if !params.precision.is_empty() {
    if let Some(comps) = image.comps_mut() {
      for (i, comp) in comps.iter_mut().enumerate() {
        let prec_idx = std::cmp::min(i, params.precision.len() - 1);
        let param = &params.precision[prec_idx];

        let prec = if param.prec > 0 {
          param.prec
        } else {
          comp.prec
        };

        match param.mode {
          PrecisionMode::Clip => comp.clip(prec),
          PrecisionMode::Scale => comp.scale(prec),
        }
      }
    }
  }

  if params.upsample {
    match upsample_image_components(image.as_ref())? {
      Some(new_image) => *image = new_image,
      None => {
        if !params.quiet {
          println!("Image is already upsampled");
        }
      }
    }
  }

  if params.force_rgb {
    match convert_gray_to_rgb(image.as_ref())? {
      Some(new_image) => *image = new_image,
      None => {
        if !params.quiet {
          println!("Image is already in RGB colorspace");
        }
      }
    }
  }

  Ok(())
}

fn upsample_image_components(
  orig: &opj_image,
) -> Result<Option<Box<opj_image>>, ImageError> {
  let mut upsample_needed = false;

  for comp in orig.comps().unwrap().iter() {
    if comp.dx > 1 || comp.dy > 1 {
      upsample_needed = true;
      break;
    }
  }

  if !upsample_needed {
    return Ok(None);
  }

  let mut image = opj_image::new();
  image.x0 = orig.x0;
  image.y0 = orig.y0;
  image.x1 = orig.x1;
  image.y1 = orig.y1;
  image.color_space = orig.color_space;

  if !image.alloc_comps(orig.numcomps) {
    return Err(ImageError::DecodeError(
      "Failed to allocate components".into(),
    ));
  }

  let orig_comps = orig
    .comps()
    .ok_or_else(|| ImageError::DecodeError("No components".into()))?;
  let new_comps = image
    .comps_mut()
    .ok_or_else(|| ImageError::DecodeError("No components".into()))?;

  for (new_comp, org_comp) in new_comps.iter_mut().zip(orig_comps.iter()) {
    if org_comp.dx <= 1 && org_comp.dy <= 1 {
      new_comp.copy(org_comp);
      new_comp.x0 = orig.x0;
      new_comp.y0 = orig.y0;
      continue;
    }
    new_comp.dx = 1;
    new_comp.dy = 1;
    new_comp.w = org_comp.w;
    new_comp.h = org_comp.h;
    new_comp.x0 = orig.x0;
    new_comp.y0 = orig.y0;
    new_comp.prec = org_comp.prec;
    new_comp.bpp = 0;
    new_comp.sgnd = org_comp.sgnd;
    new_comp.factor = org_comp.factor;
    new_comp.alpha = org_comp.alpha;
    new_comp.resno_decoded = org_comp.resno_decoded;

    if org_comp.dx > 1 {
      new_comp.w = orig.x1 - orig.x0;
    }
    if org_comp.dy > 1 {
      new_comp.h = orig.y1 - orig.y0;
    }
    if !new_comp.alloc_data() {
      return Err(ImageError::DecodeError(
        "Failed to allocate component data".into(),
      ));
    }
    let new_w = new_comp.w as usize;
    let new_h = new_comp.h as usize;

    let src = org_comp
      .data()
      .ok_or_else(|| ImageError::DecodeError("No component data".into()))?;
    let dst = new_comp
      .data_mut()
      .ok_or_else(|| ImageError::DecodeError("No component data".into()))?;

    let xoff = (org_comp.dx * org_comp.x0 - orig.x0) as usize;
    let yoff = (org_comp.dy * org_comp.y0 - orig.y0) as usize;
    let orig_dx = org_comp.dx as usize;
    let orig_dy = org_comp.dy as usize;
    if xoff >= orig_dx || yoff >= orig_dy {
      return Err(ImageError::DecodeError(
        "Invalid image/component parameters found when upsampling".into(),
      ));
    }

    let mut src_idx = 0;
    let mut y = yoff;
    let max_y = if new_h > (orig_dy - 1) {
      new_h - (orig_dy - 1)
    } else {
      0
    };
    let max_x = if new_w > (orig_dx - 1) {
      new_w - (orig_dx - 1)
    } else {
      0
    };

    let mut dst_idx = 0;
    for _ in 0..yoff {
      let end = dst_idx + new_w as usize;
      dst[dst_idx..end].fill(0);
      dst_idx += new_w as usize;
    }

    while y < max_y {
      for x in 0..xoff {
        dst[dst_idx + x] = 0;
      }

      let mut x = xoff;
      let mut src_x = 0;
      while x < max_x {
        let val = src[src_idx + src_x];
        for dx in 0..orig_dx {
          dst[dst_idx + (x + dx)] = val;
        }
        x += orig_dx;
        src_x += 1;
      }

      while x < new_w {
        dst[dst_idx + x] = src[src_idx + src_x];
        x += 1;
      }
      dst_idx += new_w;

      for _ in 1..org_comp.dy {
        dst.copy_within(dst_idx - new_w..dst_idx, dst_idx);
        dst_idx += new_w;
      }

      y += orig_dy;
      src_idx += org_comp.w as usize;
    }

    if y < new_h {
      for x in 0..xoff {
        dst[dst_idx + x] = 0;
      }

      let mut x = xoff;
      let mut src_x = 0;
      while x < max_x {
        let val = src[src_idx + src_x];
        for dx in 0..orig_dx {
          dst[dst_idx + x + dx] = val;
        }
        x += orig_dx;
        src_x += 1;
      }

      while x < new_w {
        dst[dst_idx + x] = src[src_idx + src_x];
        x += 1;
      }
      dst_idx += new_w;
      y += 1;

      for _ in y..new_h {
        dst.copy_within(dst_idx - new_w..dst_idx, dst_idx);
        dst_idx += new_w;
      }
    }
  }

  Ok(Some(image))
}

fn convert_gray_to_rgb(orig: &opj_image) -> Result<Option<Box<opj_image>>, ImageError> {
  match orig.color_space {
    OPJ_CLRSPC_SRGB => {
      return Ok(None);
    }
    OPJ_CLRSPC_GRAY => (),
    _ => {
      return Err(ImageError::DecodeError(
        "Don't know how to convert image to RGB colorspace".into(),
      ))
    }
  }

  let mut image = opj_image::new();
  image.x0 = orig.x0;
  image.y0 = orig.y0;
  image.x1 = orig.x1;
  image.y1 = orig.y1;
  image.color_space = OPJ_CLRSPC_SRGB;

  let num_new_comp = orig.numcomps + 2;
  if !image.alloc_comps(num_new_comp) {
    return Err(ImageError::DecodeError(
      "Failed to allocate components".into(),
    ));
  }

  let orig_comps = orig
    .comps()
    .ok_or_else(|| ImageError::DecodeError("No components".into()))?;
  let new_comps = image
    .comps_mut()
    .ok_or_else(|| ImageError::DecodeError("No components".into()))?;

  let (gray, old_remain) = orig_comps
    .split_first()
    .ok_or_else(|| ImageError::DecodeError("No components".into()))?;
  let (rgb, new_remain) = new_comps.split_at_mut(3);

  for comp in rgb.iter_mut() {
    comp.copy(gray);
  }
  for (old, new) in old_remain.iter().zip(new_remain.iter_mut()) {
    new.copy(old);
  }

  Ok(Some(image))
}
