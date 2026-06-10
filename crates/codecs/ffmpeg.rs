use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::path::Path;

use anyhow::{anyhow, Result};

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

extern "C" {
    fn openarc_ffmpeg_transcode(
        input_path: *const c_char,
        output_path: *const c_char,
        codec: c_int,
        preset: *const c_char,
        crf: c_int,
        copy_audio: c_int,
    ) -> c_int;

    fn openarc_ffmpeg_strerror(err: c_int, buf: *mut c_char, buf_size: c_int) -> c_int;
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
        self.encode_file_with_wrapper(input, output)
    }

    fn encode_file_with_wrapper(&self, input: &Path, output: &Path) -> Result<()> {
        let (codec, preset) = match self.options.codec {
            VideoCodec::H264 => (264, self.options.speed.as_x264_preset()),
            VideoCodec::H265 => (265, self.options.speed.as_x265_preset()),
        };

        let input_c = CString::new(input.to_string_lossy().as_bytes())?;
        let output_c = CString::new(output.to_string_lossy().as_bytes())?;
        let preset_c = CString::new(preset)?;

        let crf = self.options.effective_crf() as c_int;
        let copy_audio: c_int = if self.options.copy_audio { 1 } else { 0 };

        let ret = unsafe {
            openarc_ffmpeg_transcode(
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
                ffmpeg_err_to_string(ret),
                ret
            ));
        }

        Ok(())
    }

}

fn ffmpeg_err_to_string(err: c_int) -> String {
    let mut buf = vec![0 as c_char; 256];
    let ret = unsafe { openarc_ffmpeg_strerror(err, buf.as_mut_ptr(), buf.len() as c_int) };
    if ret < 0 {
        return "unknown error".to_string();
    }

    unsafe {
        let cstr = std::ffi::CStr::from_ptr(buf.as_ptr());
        cstr.to_string_lossy().trim().to_string()
    }
}
