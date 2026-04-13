//! Codec implementations for different file types

// Link LittleCMS once through `lcms2-sys`.
#[allow(unused_imports)]
use lcms2 as _;

pub mod bpg;
pub mod bpg_js;
pub mod heic;

// Future codecs
pub mod ffmpeg;
pub mod video_analyzer;
pub mod freearc_wrapper;
// pub mod arc;
