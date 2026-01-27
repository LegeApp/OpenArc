use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use libloading::Library;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoSpeedPreset {
    Fast,
    Medium,
    Slow,
}

impl VideoSpeedPreset {
    fn as_x264_preset(self) -> &'static str {
        match self {
            VideoSpeedPreset::Fast => "veryfast",
            VideoSpeedPreset::Medium => "medium",
            VideoSpeedPreset::Slow => "slow",
        }
    }

    fn as_x265_preset(self) -> &'static str {
        match self {
            VideoSpeedPreset::Fast => "fast",
            VideoSpeedPreset::Medium => "medium",
            VideoSpeedPreset::Slow => "slow",
        }
    }
}

fn openarc_ffmpeg_dll_path() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("Failed to resolve executable directory"))?;
    Ok(dir.join("openarc_ffmpeg.dll"))
}

#[derive(Debug, Clone)]
pub struct FfmpegEncodeOptions {
    pub codec: VideoCodec,
    pub speed: VideoSpeedPreset,
    pub crf: Option<u8>,
    pub copy_audio: bool,
}

impl Default for FfmpegEncodeOptions {
    fn default() -> Self {
        Self {
            codec: VideoCodec::H265,
            speed: VideoSpeedPreset::Medium,
            crf: None,
            copy_audio: true,
        }
    }
}

impl FfmpegEncodeOptions {
    fn effective_crf(&self) -> u8 {
        if let Some(crf) = self.crf {
            return crf;
        }

        match self.codec {
            VideoCodec::H264 => 23,
            VideoCodec::H265 => 28,
        }
    }
}

pub struct FFmpegEncoder {
    options: FfmpegEncodeOptions,
}

impl FFmpegEncoder {
    pub fn with_options(options: FfmpegEncodeOptions) -> Self {
        Self { options }
    }

    pub fn with_preset(codec: VideoCodec, speed: VideoSpeedPreset) -> Self {
        Self::with_options(FfmpegEncodeOptions {
            codec,
            speed,
            ..FfmpegEncodeOptions::default()
        })
    }

    pub fn encode_file(&self, input: &Path, output: &Path) -> Result<()> {
        let (codec, preset) = match self.options.codec {
            VideoCodec::H264 => (264, self.options.speed.as_x264_preset()),
            VideoCodec::H265 => (265, self.options.speed.as_x265_preset()),
        };

        let input_c = CString::new(input.to_string_lossy().as_bytes())?;
        let output_c = CString::new(output.to_string_lossy().as_bytes())?;
        let preset_c = CString::new(preset)?;

        let crf = self.options.effective_crf() as i32;
        let copy_audio = if self.options.copy_audio { 1 } else { 0 };

        let dll_path = openarc_ffmpeg_dll_path()?;
        let lib = unsafe { Library::new(&dll_path) }
            .map_err(|e| anyhow!("Failed to load {}: {}", dll_path.display(), e))?;

        type TranscodeFn = unsafe extern "C" fn(
            *const c_char,
            *const c_char,
            c_int,
            *const c_char,
            c_int,
            c_int,
        ) -> c_int;
        type StrerrorFn = unsafe extern "C" fn(c_int, *mut c_char, c_int) -> c_int;

        let transcode: libloading::Symbol<TranscodeFn> = unsafe { lib.get(b"openarc_ffmpeg_transcode\0") }
            .map_err(|e| anyhow!("Missing symbol openarc_ffmpeg_transcode: {}", e))?;
        let strerror: libloading::Symbol<StrerrorFn> = unsafe { lib.get(b"openarc_ffmpeg_strerror\0") }
            .map_err(|e| anyhow!("Missing symbol openarc_ffmpeg_strerror: {}", e))?;

        let ret = unsafe {
            transcode(
                input_c.as_ptr(),
                output_c.as_ptr(),
                codec,
                preset_c.as_ptr(),
                crf,
                copy_audio,
            )
        };

        if ret < 0 {
            return Err(anyhow!(
                "FFmpeg transcode failed: {} ({})",
                ffmpeg_err_to_string(ret, &strerror),
                ret
            ));
        }

        Ok(())
    }
}

fn ffmpeg_err_to_string(err: i32, strerror: &libloading::Symbol<unsafe extern "C" fn(c_int, *mut c_char, c_int) -> c_int>) -> String {
    let mut buf = vec![0 as c_char; 256];
    let ret = unsafe { strerror(err, buf.as_mut_ptr(), buf.len() as c_int) };
    if ret < 0 {
        return "unknown error".to_string();
    }

    unsafe {
        let cstr = std::ffi::CStr::from_ptr(buf.as_ptr());
        cstr.to_string_lossy().trim().to_string()
    }
}
