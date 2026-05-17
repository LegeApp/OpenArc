// Native BPG library FFI bindings.
// The archival HEIC path uses direct planar YUV entry points so the encoder can
// keep source bit depth, chroma sampling, and range metadata intact.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

use anyhow::{anyhow, ensure, Result};

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

#[cfg(all(windows, target_env = "msvc"))]
mod bpg_ffi {
    use super::*;
    use libloading::Library;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    type EncoderCreateFn = unsafe extern "C" fn() -> *mut BPGEncoderContext;
    type EncoderCreateExFn = unsafe extern "C" fn(*const BPGEncoderConfig) -> *mut BPGEncoderContext;
    type EncoderSetConfigFn =
        unsafe extern "C" fn(*mut BPGEncoderContext, *const BPGEncoderConfig) -> c_int;
    type EncoderGetDefaultConfigFn = unsafe extern "C" fn(*mut BPGEncoderConfig);
    type EncodeFromFileFn =
        unsafe extern "C" fn(*mut BPGEncoderContext, *const c_char, *mut *mut u8, *mut usize) -> c_int;
    type EncodeFromMemoryFn = unsafe extern "C" fn(
        *mut BPGEncoderContext,
        *const u8,
        c_int,
        c_int,
        c_int,
        BPGImageFormat,
        *mut *mut u8,
        *mut usize,
    ) -> c_int;
    type EncodeFromPlanarU8Fn = unsafe extern "C" fn(
        *mut BPGEncoderContext,
        *const u8,
        c_int,
        *const u8,
        c_int,
        *const u8,
        c_int,
        c_int,
        c_int,
        BPGImageFormat,
        *mut *mut u8,
        *mut usize,
    ) -> c_int;
    type EncodeFromPlanarU16Fn = unsafe extern "C" fn(
        *mut BPGEncoderContext,
        *const u16,
        c_int,
        *const u16,
        c_int,
        *const u16,
        c_int,
        c_int,
        c_int,
        BPGImageFormat,
        *mut *mut u8,
        *mut usize,
    ) -> c_int;
    type EncodeToFileFn =
        unsafe extern "C" fn(*mut BPGEncoderContext, *const c_char, *const c_char) -> c_int;
    type EncoderGetErrorFn = unsafe extern "C" fn(*mut BPGEncoderContext) -> *const c_char;
    type EncoderDestroyFn = unsafe extern "C" fn(*mut BPGEncoderContext);
    type DecodeFileFn =
        unsafe extern "C" fn(*const c_char, *mut *mut u8, *mut c_int, *mut c_int, *mut BPGImageFormat) -> c_int;
    type FreeFn = unsafe extern "C" fn(*mut c_void);
    type GetVersionFn = unsafe extern "C" fn() -> *const c_char;
    type GetSupportedEncodersFn = unsafe extern "C" fn() -> c_int;

    struct BpgLibrary {
        _library: Library,
        encoder_create: EncoderCreateFn,
        encoder_create_ex: EncoderCreateExFn,
        encoder_set_config: EncoderSetConfigFn,
        encoder_get_default_config: EncoderGetDefaultConfigFn,
        encode_from_file: EncodeFromFileFn,
        encode_from_memory: EncodeFromMemoryFn,
        encode_from_planar_u8: EncodeFromPlanarU8Fn,
        encode_from_planar_u16: EncodeFromPlanarU16Fn,
        encode_to_file: EncodeToFileFn,
        encoder_get_error: EncoderGetErrorFn,
        encoder_destroy: EncoderDestroyFn,
        decode_file: DecodeFileFn,
        free: FreeFn,
        get_version: GetVersionFn,
        get_supported_encoders: GetSupportedEncodersFn,
    }

    static BPG_LIBRARY: OnceLock<std::result::Result<BpgLibrary, String>> = OnceLock::new();

    fn load_library() -> std::result::Result<BpgLibrary, String> {
        let mut candidates = Vec::new();
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(dir) = exe_path.parent() {
                candidates.push(dir.join("openarc_bpg.dll"));
            }
        }
        candidates.push(PathBuf::from("openarc_bpg.dll"));

        let mut errors = Vec::new();
        for candidate in candidates {
            let library = match unsafe { Library::new(&candidate) } {
                Ok(library) => library,
                Err(err) => {
                    errors.push(format!("{} ({err})", candidate.display()));
                    continue;
                }
            };

            unsafe {
                let encoder_create = *library
                    .get::<EncoderCreateFn>(b"bpg_encoder_create\0")
                    .map_err(|err| format!("bpg_encoder_create: {err}"))?;
                let encoder_create_ex = *library
                    .get::<EncoderCreateExFn>(b"bpg_encoder_create_ex\0")
                    .map_err(|err| format!("bpg_encoder_create_ex: {err}"))?;
                let encoder_set_config = *library
                    .get::<EncoderSetConfigFn>(b"bpg_encoder_set_config\0")
                    .map_err(|err| format!("bpg_encoder_set_config: {err}"))?;
                let encoder_get_default_config = *library
                    .get::<EncoderGetDefaultConfigFn>(b"bpg_encoder_get_default_config\0")
                    .map_err(|err| format!("bpg_encoder_get_default_config: {err}"))?;
                let encode_from_file = *library
                    .get::<EncodeFromFileFn>(b"bpg_encode_from_file\0")
                    .map_err(|err| format!("bpg_encode_from_file: {err}"))?;
                let encode_from_memory = *library
                    .get::<EncodeFromMemoryFn>(b"bpg_encode_from_memory\0")
                    .map_err(|err| format!("bpg_encode_from_memory: {err}"))?;
                let encode_from_planar_u8 = *library
                    .get::<EncodeFromPlanarU8Fn>(b"bpg_encode_from_planar_u8\0")
                    .map_err(|err| format!("bpg_encode_from_planar_u8: {err}"))?;
                let encode_from_planar_u16 = *library
                    .get::<EncodeFromPlanarU16Fn>(b"bpg_encode_from_planar_u16\0")
                    .map_err(|err| format!("bpg_encode_from_planar_u16: {err}"))?;
                let encode_to_file = *library
                    .get::<EncodeToFileFn>(b"bpg_encode_to_file\0")
                    .map_err(|err| format!("bpg_encode_to_file: {err}"))?;
                let encoder_get_error = *library
                    .get::<EncoderGetErrorFn>(b"bpg_encoder_get_error\0")
                    .map_err(|err| format!("bpg_encoder_get_error: {err}"))?;
                let encoder_destroy = *library
                    .get::<EncoderDestroyFn>(b"bpg_encoder_destroy\0")
                    .map_err(|err| format!("bpg_encoder_destroy: {err}"))?;
                let decode_file = *library
                    .get::<DecodeFileFn>(b"bpg_decode_file\0")
                    .map_err(|err| format!("bpg_decode_file: {err}"))?;
                let free = *library
                    .get::<FreeFn>(b"bpg_free\0")
                    .map_err(|err| format!("bpg_free: {err}"))?;
                let get_version = *library
                    .get::<GetVersionFn>(b"bpg_get_version\0")
                    .map_err(|err| format!("bpg_get_version: {err}"))?;
                let get_supported_encoders = *library
                    .get::<GetSupportedEncodersFn>(b"bpg_get_supported_encoders\0")
                    .map_err(|err| format!("bpg_get_supported_encoders: {err}"))?;

                return Ok(BpgLibrary {
                    _library: library,
                    encoder_create,
                    encoder_create_ex,
                    encoder_set_config,
                    encoder_get_default_config,
                    encode_from_file,
                    encode_from_memory,
                    encode_from_planar_u8,
                    encode_from_planar_u16,
                    encode_to_file,
                    encoder_get_error,
                    encoder_destroy,
                    decode_file,
                    free,
                    get_version,
                    get_supported_encoders,
                });
            }
        }

        Err(format!(
            "openarc_bpg.dll was not found or could not be loaded: {}",
            errors.join("; ")
        ))
    }

    fn library() -> Result<&'static BpgLibrary> {
        match BPG_LIBRARY.get_or_init(load_library) {
            Ok(library) => Ok(library),
            Err(err) => Err(anyhow!(err.clone())),
        }
    }

    pub unsafe fn encoder_create() -> Result<*mut BPGEncoderContext> {
        Ok((library()?.encoder_create)())
    }

    pub unsafe fn encoder_create_ex(config: *const BPGEncoderConfig) -> Result<*mut BPGEncoderContext> {
        Ok((library()?.encoder_create_ex)(config))
    }

    pub unsafe fn encoder_set_config(
        ctx: *mut BPGEncoderContext,
        config: *const BPGEncoderConfig,
    ) -> Result<c_int> {
        Ok((library()?.encoder_set_config)(ctx, config))
    }

    pub unsafe fn encoder_get_default_config(config: *mut BPGEncoderConfig) -> Result<()> {
        (library()?.encoder_get_default_config)(config);
        Ok(())
    }

    pub unsafe fn encode_from_file(
        ctx: *mut BPGEncoderContext,
        input_path: *const c_char,
        output_data: *mut *mut u8,
        output_size: *mut usize,
    ) -> Result<c_int> {
        Ok((library()?.encode_from_file)(ctx, input_path, output_data, output_size))
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
        Ok((library()?.encode_from_memory)(
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
        Ok((library()?.encode_from_planar_u8)(
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
        Ok((library()?.encode_from_planar_u16)(
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
        Ok((library()?.encode_to_file)(ctx, input_path, output_path))
    }

    pub unsafe fn encoder_get_error(ctx: *mut BPGEncoderContext) -> Result<*const c_char> {
        Ok((library()?.encoder_get_error)(ctx))
    }

    pub unsafe fn encoder_destroy(ctx: *mut BPGEncoderContext) {
        if let Ok(library) = library() {
            (library.encoder_destroy)(ctx);
        }
    }

    pub unsafe fn decode_file(
        input_path: *const c_char,
        output_data: *mut *mut u8,
        width: *mut c_int,
        height: *mut c_int,
        format: *mut BPGImageFormat,
    ) -> Result<c_int> {
        Ok((library()?.decode_file)(input_path, output_data, width, height, format))
    }

    pub unsafe fn free(ptr: *mut c_void) -> Result<()> {
        (library()?.free)(ptr);
        Ok(())
    }

    pub unsafe fn get_version() -> Result<*const c_char> {
        Ok((library()?.get_version)())
    }

    pub unsafe fn get_supported_encoders() -> Result<c_int> {
        Ok((library()?.get_supported_encoders)())
    }
}

#[cfg(not(all(windows, target_env = "msvc")))]
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
}

impl NativeBPGEncoder {
    pub fn new() -> Result<Self> {
        let ctx = unsafe { bpg_ffi::encoder_create()? };
        if ctx.is_null() {
            return Err(anyhow!("Failed to create BPG encoder"));
        }
        Ok(Self { ctx })
    }

    pub fn with_quality(quality: u8) -> Result<Self> {
        let mut config = Self::default_config();
        config.quality = quality as c_int;

        let ctx = unsafe { bpg_ffi::encoder_create_ex(&config)? };
        if ctx.is_null() {
            return Err(anyhow!("Failed to create BPG encoder"));
        }
        Ok(Self { ctx })
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
        Ok(())
    }

    pub fn encode_from_file(&self, input_path: &str) -> Result<Vec<u8>> {
        let input_cstr = CString::new(input_path)?;
        let mut output_data: *mut u8 = ptr::null_mut();
        let mut output_size: usize = 0;

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
