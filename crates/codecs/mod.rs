//! Codec implementations for different file types.
//!
//! [`jxl`] is the image encoder: every image OpenArc writes is JPEG XL. The
//! other modules are readers — [`heic`] and [`jpeg2000`] for source formats the
//! `image` crate cannot open, [`bpg_legacy`] for archives written before the
//! JPEG XL switch — plus [`video_analyzer`].

// Link LittleCMS once through `lcms2-sys`.
#[allow(unused_imports)]
use lcms2 as _;

pub mod jxl;

pub mod bpg_legacy;
pub mod heic;
pub mod jpeg2000;

// Future codecs
pub mod freearc_wrapper;
pub mod video_analyzer;
// pub mod arc;
