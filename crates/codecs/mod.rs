//! Codec implementations for different file types

// Link LittleCMS once through `lcms2-sys`.
#[allow(unused_imports)]
use lcms2 as _;

pub mod bpg;
pub mod bpg_js;
pub mod heic;
pub mod jpeg2000;

// Future codecs
pub mod video_analyzer;
pub mod freearc_wrapper;
// pub mod arc;
