// Native BPG library FFI bindings.
// The archival HEIC path uses direct planar YUV entry points so the encoder can
// keep source bit depth, chroma sampling, and range metadata intact.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::{Mutex, MutexGuard};

use anyhow::{anyhow, ensure, Result};

/// The JCTVC (HM reference) encoder is built on extensive mutable global/static
/// state and is NOT thread-safe: running two `TAppEncTop` encodes concurrently in
/// the same process corrupts shared globals and trips internal assertions such as
/// "codeCoeffNxN called for empty TU!". OpenArc encodes images in parallel across
/// worker threads, so every JCTVC encode must hold this process-wide lock,
/// serializing them. x265 (encoder_type 0) is per-instance safe and unaffected.
static JCTVC_LOCK: Mutex<()> = Mutex::new(());

/// `encoder_type` value selecting the JCTVC (HM reference) HEVC encoder.
const ENCODER_TYPE_JCTVC: c_int = 1;

#[repr(C)]
pub struct BPGEncoderContext {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct BPGEncoderConfig {
    pub quality: c_int,
    pub bit_depth: c_int,
    pub lossless: c_int,
    pub chroma_format: c_int,
    pub encoder_type: c_int,
    pub compress_level: c_int,
    pub color_space: c_int,
    pub limited_range: c_int,
}

#[repr(C)]
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

#[repr(C)]
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
            _ => Err(anyhow!("Format {:?} is not planar YCbCr", self)),
        }
    }
}

mod bpg_ffi {
    use super::*;

    extern "C" {
        fn bpg_encoder_create() -> *mut BPGEncoderContext;
        fn bpg_encoder_create_ex(config: *const BPGEncoderConfig) -> *mut BPGEncoderContext;
        fn bpg_encoder_set_config(ctx: *mut BPGEncoderContext, config: *const BPGEncoderConfig)
            -> c_int;
        fn bpg_encoder_get_default_config(config: *mut BPGEncoderConfig);

        fn bpg_encode_from_file(
            ctx: *mut BPGEncoderContext,
            input_path: *const c_char,
            output_data: *mut *mut u8,
            output_size: *mut usize,
        ) -> c_int;

        fn bpg_encode_from_memory(
            ctx: *mut BPGEncoderContext,
            input_data: *const u8,
            width: c_int,
            height: c_int,
            stride: c_int,
            format: BPGImageFormat,
            output_data: *mut *mut u8,
            output_size: *mut usize,
        ) -> c_int;

        fn bpg_encode_from_planar_u8(
            ctx: *mut BPGEncoderContext,
            y_plane: *const u8,
            y_stride: c_int,
            cb_plane: *const u8,
            cb_stride: c_int,
            cr_plane: *const u8,
            cr_stride: c_int,
            width: c_int,
            height: c_int,
            format: BPGImageFormat,
            output_data: *mut *mut u8,
            output_size: *mut usize,
        ) -> c_int;

        fn bpg_encode_from_planar_u16(
            ctx: *mut BPGEncoderContext,
            y_plane: *const u16,
            y_stride: c_int,
            cb_plane: *const u16,
            cb_stride: c_int,
            cr_plane: *const u16,
            cr_stride: c_int,
            width: c_int,
            height: c_int,
            format: BPGImageFormat,
            output_data: *mut *mut u8,
            output_size: *mut usize,
        ) -> c_int;

        fn bpg_encode_to_file(
            ctx: *mut BPGEncoderContext,
            input_path: *const c_char,
            output_path: *const c_char,
        ) -> c_int;

        fn bpg_encoder_get_error(ctx: *mut BPGEncoderContext) -> *const c_char;
        fn bpg_encoder_destroy(ctx: *mut BPGEncoderContext);

        fn bpg_decode_file(
            input_path: *const c_char,
            output_data: *mut *mut u8,
            width: *mut c_int,
            height: *mut c_int,
            format: *mut BPGImageFormat,
        ) -> c_int;

        fn bpg_free(ptr: *mut c_void);
        fn bpg_get_version() -> *const c_char;
        fn bpg_get_supported_encoders() -> c_int;
    }

    pub unsafe fn encoder_create() -> Result<*mut BPGEncoderContext> {
        Ok(bpg_encoder_create())
    }

    pub unsafe fn encoder_create_ex(config: *const BPGEncoderConfig) -> Result<*mut BPGEncoderContext> {
        Ok(bpg_encoder_create_ex(config))
    }

    pub unsafe fn encoder_set_config(
        ctx: *mut BPGEncoderContext,
        config: *const BPGEncoderConfig,
    ) -> Result<c_int> {
        Ok(bpg_encoder_set_config(ctx, config))
    }

    pub unsafe fn encoder_get_default_config(config: *mut BPGEncoderConfig) -> Result<()> {
        bpg_encoder_get_default_config(config);
        Ok(())
    }

    pub unsafe fn encode_from_file(
        ctx: *mut BPGEncoderContext,
        input_path: *const c_char,
        output_data: *mut *mut u8,
        output_size: *mut usize,
    ) -> Result<c_int> {
        Ok(bpg_encode_from_file(ctx, input_path, output_data, output_size))
    }

    pub unsafe fn encode_from_memory(
        ctx: *mut BPGEncoderContext,
        input_data: *const u8,
        width: c_int,
        height: c_int,
        stride: c_int,
        format: BPGImageFormat,
        output_data: *mut *mut u8,
        output_size: *mut usize,
    ) -> Result<c_int> {
        Ok(bpg_encode_from_memory(
            ctx,
            input_data,
            width,
            height,
            stride,
            format,
            output_data,
            output_size,
        ))
    }

    pub unsafe fn encode_from_planar_u8(
        ctx: *mut BPGEncoderContext,
        y_plane: *const u8,
        y_stride: c_int,
        cb_plane: *const u8,
        cb_stride: c_int,
        cr_plane: *const u8,
        cr_stride: c_int,
        width: c_int,
        height: c_int,
        format: BPGImageFormat,
        output_data: *mut *mut u8,
        output_size: *mut usize,
    ) -> Result<c_int> {
        Ok(bpg_encode_from_planar_u8(
            ctx,
            y_plane,
            y_stride,
            cb_plane,
            cb_stride,
            cr_plane,
            cr_stride,
            width,
            height,
            format,
            output_data,
            output_size,
        ))
    }

    pub unsafe fn encode_from_planar_u16(
        ctx: *mut BPGEncoderContext,
        y_plane: *const u16,
        y_stride: c_int,
        cb_plane: *const u16,
        cb_stride: c_int,
        cr_plane: *const u16,
        cr_stride: c_int,
        width: c_int,
        height: c_int,
        format: BPGImageFormat,
        output_data: *mut *mut u8,
        output_size: *mut usize,
    ) -> Result<c_int> {
        Ok(bpg_encode_from_planar_u16(
            ctx,
            y_plane,
            y_stride,
            cb_plane,
            cb_stride,
            cr_plane,
            cr_stride,
            width,
            height,
            format,
            output_data,
            output_size,
        ))
    }

    pub unsafe fn encode_to_file(
        ctx: *mut BPGEncoderContext,
        input_path: *const c_char,
        output_path: *const c_char,
    ) -> Result<c_int> {
        Ok(bpg_encode_to_file(ctx, input_path, output_path))
    }

    pub unsafe fn encoder_get_error(ctx: *mut BPGEncoderContext) -> Result<*const c_char> {
        Ok(bpg_encoder_get_error(ctx))
    }

    pub unsafe fn encoder_destroy(ctx: *mut BPGEncoderContext) {
        bpg_encoder_destroy(ctx);
    }

    pub unsafe fn decode_file(
        input_path: *const c_char,
        output_data: *mut *mut u8,
        width: *mut c_int,
        height: *mut c_int,
        format: *mut BPGImageFormat,
    ) -> Result<c_int> {
        Ok(bpg_decode_file(input_path, output_data, width, height, format))
    }

    pub unsafe fn free(ptr: *mut c_void) -> Result<()> {
        bpg_free(ptr);
        Ok(())
    }

    pub unsafe fn get_version() -> Result<*const c_char> {
        Ok(bpg_get_version())
    }

    pub unsafe fn get_supported_encoders() -> Result<c_int> {
        Ok(bpg_get_supported_encoders())
    }
}

pub struct NativeBPGEncoder {
    ctx: *mut BPGEncoderContext,
    /// Mirrors the native context's `encoder_type` so encode calls know whether
    /// they must serialize through `JCTVC_LOCK` (JCTVC is not thread-safe).
    encoder_type: c_int,
}

impl NativeBPGEncoder {
    pub fn new() -> Result<Self> {
        let ctx = unsafe { bpg_ffi::encoder_create()? };
        if ctx.is_null() {
            return Err(anyhow!("Failed to create BPG encoder"));
        }
        Ok(Self { ctx, encoder_type: 0 })
    }

    pub fn with_quality(quality: u8) -> Result<Self> {
        let mut config = Self::default_config();
        config.quality = quality as c_int;

        let ctx = unsafe { bpg_ffi::encoder_create_ex(&config)? };
        if ctx.is_null() {
            return Err(anyhow!("Failed to create BPG encoder"));
        }
        Ok(Self { ctx, encoder_type: config.encoder_type })
    }

    /// Acquire the process-wide JCTVC serialization lock when this encoder uses
    /// the (non-thread-safe) JCTVC backend; returns `None` for thread-safe x265.
    fn jctvc_guard(&self) -> Option<MutexGuard<'static, ()>> {
        if self.encoder_type == ENCODER_TYPE_JCTVC {
            // A poisoned lock just means a prior JCTVC encode panicked; the native
            // encoder holds no Rust-visible invariants across the boundary, so
            // recovering the guard and proceeding is safe.
            Some(JCTVC_LOCK.lock().unwrap_or_else(|e| e.into_inner()))
        } else {
            None
        }
    }

    pub fn default_config() -> BPGEncoderConfig {
        let mut config = BPGEncoderConfig {
            quality: 28,
            bit_depth: 8,
            lossless: 0,
            chroma_format: 1,
            encoder_type: 0,
            compress_level: 8,
            color_space: 3,
            limited_range: 0,
        };
        let _ = unsafe { bpg_ffi::encoder_get_default_config(&mut config) };
        config
    }

    pub fn set_config(&mut self, config: &BPGEncoderConfig) -> Result<()> {
        let result = unsafe { bpg_ffi::encoder_set_config(self.ctx, config)? };
        if result != 0 {
            return Err(anyhow!("Failed to set config: {}", self.get_error()));
        }
        self.encoder_type = config.encoder_type;
        Ok(())
    }

    pub fn encode_from_file(&self, input_path: &str) -> Result<Vec<u8>> {
        let input_cstr = CString::new(input_path)?;
        let mut output_data: *mut u8 = ptr::null_mut();
        let mut output_size: usize = 0;

        let _jctvc = self.jctvc_guard();
        let result = unsafe {
            bpg_ffi::encode_from_file(
                self.ctx,
                input_cstr.as_ptr(),
                &mut output_data,
                &mut output_size,
            )?
        };

        if result != 0 {
            return Err(anyhow!("Encoding failed: {}", self.get_error()));
        }

        Self::take_output(output_data, output_size)
    }

    pub fn encode_to_file(&self, input_path: &str, output_path: &str) -> Result<()> {
        let input_cstr = CString::new(input_path)?;
        let output_cstr = CString::new(output_path)?;

        let _jctvc = self.jctvc_guard();
        let result = unsafe {
            bpg_ffi::encode_to_file(self.ctx, input_cstr.as_ptr(), output_cstr.as_ptr())?
        };

        if result != 0 {
            return Err(anyhow!("Encoding failed: {}", self.get_error()));
        }

        Ok(())
    }

    pub fn encode_from_memory(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        format: BPGImageFormat,
    ) -> Result<Vec<u8>> {
        let mut output_data: *mut u8 = ptr::null_mut();
        let mut output_size: usize = 0;

        let _jctvc = self.jctvc_guard();
        let result = unsafe {
            bpg_ffi::encode_from_memory(
                self.ctx,
                data.as_ptr(),
                width as c_int,
                height as c_int,
                stride as c_int,
                format,
                &mut output_data,
                &mut output_size,
            )?
        };

        if result != 0 {
            return Err(anyhow!("Encoding failed: {}", self.get_error()));
        }

        Self::take_output(output_data, output_size)
    }

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

        let mut output_data: *mut u8 = ptr::null_mut();
        let mut output_size: usize = 0;
        let cb_ptr = if cb_plane.is_empty() {
            ptr::null()
        } else {
            cb_plane.as_ptr()
        };
        let cr_ptr = if cr_plane.is_empty() {
            ptr::null()
        } else {
            cr_plane.as_ptr()
        };

        let _jctvc = self.jctvc_guard();
        let result = unsafe {
            bpg_ffi::encode_from_planar_u8(
                self.ctx,
                y_plane.as_ptr(),
                y_stride as c_int,
                cb_ptr,
                cb_stride as c_int,
                cr_ptr,
                cr_stride as c_int,
                width as c_int,
                height as c_int,
                format,
                &mut output_data,
                &mut output_size,
            )?
        };

        if result != 0 {
            return Err(anyhow!("Encoding failed: {}", self.get_error()));
        }

        Self::take_output(output_data, output_size)
    }

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

        let mut output_data: *mut u8 = ptr::null_mut();
        let mut output_size: usize = 0;
        let cb_ptr = if cb_plane.is_empty() {
            ptr::null()
        } else {
            cb_plane.as_ptr()
        };
        let cr_ptr = if cr_plane.is_empty() {
            ptr::null()
        } else {
            cr_plane.as_ptr()
        };

        let _jctvc = self.jctvc_guard();
        let result = unsafe {
            bpg_ffi::encode_from_planar_u16(
                self.ctx,
                y_plane.as_ptr(),
                y_stride as c_int,
                cb_ptr,
                cb_stride as c_int,
                cr_ptr,
                cr_stride as c_int,
                width as c_int,
                height as c_int,
                format,
                &mut output_data,
                &mut output_size,
            )?
        };

        if result != 0 {
            return Err(anyhow!("Encoding failed: {}", self.get_error()));
        }

        Self::take_output(output_data, output_size)
    }

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

    fn take_output(output_data: *mut u8, output_size: usize) -> Result<Vec<u8>> {
        if output_data.is_null() || output_size == 0 {
            return Err(anyhow!("Encoding produced no output"));
        }

        let data = unsafe {
            let slice = std::slice::from_raw_parts(output_data, output_size);
            let vec = slice.to_vec();
            bpg_ffi::free(output_data as *mut c_void)?;
            vec
        };

        Ok(data)
    }

    fn get_error(&self) -> String {
        unsafe {
            let err_ptr = match bpg_ffi::encoder_get_error(self.ctx) {
                Ok(err_ptr) => err_ptr,
                Err(err) => return err.to_string(),
            };
            if err_ptr.is_null() {
                return "Unknown error".to_string();
            }
            CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
        }
    }
}

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
    validate_planar_inputs(y_plane.len(), cb_plane.len(), cr_plane.len(), width, height, y_stride, cb_stride, cr_stride, format)
}

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
    validate_planar_inputs(y_plane.len(), cb_plane.len(), cr_plane.len(), width, height, y_stride, cb_stride, cr_stride, format)
}

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
    ensure!(format.is_planar_yuv(), "Format {:?} is not planar YUV", format);
    ensure!(width > 0 && height > 0, "Invalid planar image dimensions");
    ensure!(y_stride >= width, "Luma stride is smaller than width");

    let y_required = checked_plane_len(y_stride, height)?;
    ensure!(y_len >= y_required, "Luma plane is smaller than expected");

    let (chroma_width, chroma_height) = format.chroma_dimensions(width, height)?;
    if format == BPGImageFormat::Gray {
        ensure!(cb_len == 0 && cr_len == 0, "Gray input should not provide chroma data");
        return Ok(());
    }

    ensure!(cb_stride >= chroma_width, "Cb stride is smaller than chroma width");
    ensure!(cr_stride >= chroma_width, "Cr stride is smaller than chroma width");

    let cb_required = checked_plane_len(cb_stride, chroma_height)?;
    let cr_required = checked_plane_len(cr_stride, chroma_height)?;
    ensure!(cb_len >= cb_required, "Cb plane is smaller than expected");
    ensure!(cr_len >= cr_required, "Cr plane is smaller than expected");
    Ok(())
}

fn checked_plane_len(stride: u32, height: u32) -> Result<usize> {
    (stride as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| anyhow!("Plane size overflow"))
}

impl Drop for NativeBPGEncoder {
    fn drop(&mut self) {
        unsafe {
            bpg_ffi::encoder_destroy(self.ctx);
        }
    }
}

unsafe impl Send for NativeBPGEncoder {}

pub fn decode_file(input_path: &str) -> Result<(Vec<u8>, u32, u32, BPGImageFormat)> {
    let input_cstr = CString::new(input_path)?;
    let mut output_data: *mut u8 = ptr::null_mut();
    let mut width: c_int = 0;
    let mut height: c_int = 0;
    let mut format = BPGImageFormat::RGBA32;

    let result = unsafe {
        bpg_ffi::decode_file(
            input_cstr.as_ptr(),
            &mut output_data,
            &mut width,
            &mut height,
            &mut format,
        )?
    };

    if result != 0 {
        return Err(anyhow!("Decoding failed with error code: {}", result));
    }

    if output_data.is_null() {
        return Err(anyhow!("Decoding produced no output"));
    }
    if width <= 0 || height <= 0 {
        unsafe {
            let _ = bpg_ffi::free(output_data as *mut c_void);
        }
        return Err(anyhow!("Decoding produced invalid dimensions"));
    }

    let size = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| anyhow!("Decoded BPG dimensions are too large"))?;
    let data = unsafe {
        let slice = std::slice::from_raw_parts(output_data, size);
        let vec = slice.to_vec();
        bpg_ffi::free(output_data as *mut c_void)?;
        vec
    };

    Ok((data, width as u32, height as u32, format))
}

pub fn get_version() -> String {
    unsafe {
        let ver_ptr = match bpg_ffi::get_version() {
            Ok(ver_ptr) => ver_ptr,
            Err(_) => return "unavailable".to_string(),
        };
        if ver_ptr.is_null() {
            return "unknown".to_string();
        }
        CStr::from_ptr(ver_ptr).to_string_lossy().into_owned()
    }
}

pub fn get_supported_encoders() -> i32 {
    unsafe { bpg_ffi::get_supported_encoders().unwrap_or(0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_creation() {
        assert!(NativeBPGEncoder::new().is_ok());
    }

    #[test]
    fn test_version() {
        assert!(!get_version().is_empty());
    }

    #[test]
    fn test_supported_encoders() {
        let encoders = get_supported_encoders();
        assert!(encoders & 0x01 != 0);
    }

    #[test]
    #[cfg(feature = "jctvc")]
    fn test_jctvc_encoder_available() {
        assert!(get_supported_encoders() & 0x02 != 0, "JCTVC encoder bit not set");
    }

    #[test]
    #[cfg(feature = "jctvc")]
    fn test_jctvc_encode_roundtrip() {
        let mut encoder = NativeBPGEncoder::new().unwrap();
        let mut config = NativeBPGEncoder::default_config();
        config.encoder_type = 1; // JCTVC
        config.quality = 35;
        encoder.set_config(&config).unwrap();

        let (width, height) = (64u32, 64u32);
        let y: Vec<u8> = (0..width * height).map(|i| (i % 251) as u8).collect();
        let (cw, ch) = (width.div_ceil(2), height.div_ceil(2));
        let cb = vec![128u8; (cw * ch) as usize];
        let cr = vec![128u8; (cw * ch) as usize];

        let bpg = encoder
            .encode_from_planar_u8(
                &y,
                &cb,
                &cr,
                width,
                height,
                width,
                cw,
                cw,
                BPGImageFormat::YCbCr420P,
            )
            .unwrap();
        assert!(bpg.len() > 8, "JCTVC produced an implausibly small stream");
    }

    #[test]
    fn test_planar_format_dimensions() {
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
}
