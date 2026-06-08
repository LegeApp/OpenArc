use std::io::Cursor;

use crate::codec::filters::{
    BcjX86Filter, BcjX86Options, DeltaFilter, DispackFilter, Filter, FloatXorFilter, LpcFilter,
    RawBayerFilter, RawBayerOptions, RowFilter,
};
#[cfg(feature = "bsc")]
use crate::codec::BscCodec;
use crate::codec::{
    BrotliCodec, Codec, DictCodec, EncryptionCodec, Lz4Codec, LzmaCodec, LzpCodec, OoxmlCodec,
    PpmdCodec, RepCodec, SnappyCodec, SqlTemplateCodec, SrepCodec, TokenDictCodec, TornadoCodec,
    XzCodec, ZstdCodec,
};
use crate::error::{ArcError, Result};
use crate::method::Method;

/// Runtime context passed to the planner so it can construct context-dependent
/// stages (e.g. the encryption codec needs the password at planning time).
#[derive(Debug, Clone, Default)]
pub struct PipelineContext {
    /// Raw password bytes. When `None`, `Method::Encryption` stages are left
    /// as `Stage::Unsupported` rather than failing at construction.
    pub password: Option<Vec<u8>>,
}

/// A single planned pipeline stage ready for execution.
///
/// Stages are produced by [`plan`]. `Unsupported` entries pass through the planner
/// without error; execution fails with a precise message only if a stage is actually
/// run, which lets callers inspect or validate the plan before committing to it.
pub enum Stage {
    Codec(Box<dyn Codec>),
    Filter(Box<dyn Filter>),
    /// Placeholder for codecs that are recognized but not yet implemented natively.
    Unsupported(String),
}

impl Stage {
    pub fn compress(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        match self {
            Stage::Codec(c) => {
                c.compress(&mut Cursor::new(input), &mut out)?;
            }
            Stage::Filter(f) => {
                f.encode(input, &mut out)?;
            }
            Stage::Unsupported(name) => {
                return Err(ArcError::UnsupportedCodec(format!(
                    "pipeline execution not wired for: {name}"
                )));
            }
        }
        Ok(out)
    }

    pub fn decompress(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        match self {
            Stage::Codec(c) => {
                c.decompress(&mut Cursor::new(input), &mut out)?;
            }
            Stage::Filter(f) => {
                f.decode(input, &mut out)?;
            }
            Stage::Unsupported(name) => {
                return Err(ArcError::UnsupportedCodec(format!(
                    "pipeline execution not wired for: {name}"
                )));
            }
        }
        Ok(out)
    }

    /// Returns the stage display name for diagnostics.
    pub fn name(&self) -> &str {
        match self {
            Stage::Codec(c) => c.name(),
            Stage::Filter(f) => f.name(),
            Stage::Unsupported(n) => n.as_str(),
        }
    }

    /// Returns true if this stage can be executed (not `Unsupported`).
    pub fn is_supported(&self) -> bool {
        !matches!(self, Stage::Unsupported(_))
    }
}

/// Translate a `Method` into a flat list of executable [`Stage`]s.
///
/// `Pipeline` variants are flattened; `Blocked` is treated as a single
/// unsupported stage until parallel-block execution is wired. This function
/// never fails: unrecognized methods become `Stage::Unsupported` entries.
pub fn plan(method: &Method) -> Vec<Stage> {
    plan_with_context(method, &PipelineContext::default())
}

/// Like [`plan`] but with a context that carries runtime values (e.g. password).
pub fn plan_with_context(method: &Method, ctx: &PipelineContext) -> Vec<Stage> {
    // When the ffi-codecs feature is enabled, try to satisfy the method from
    // the C bridge adapters before falling through to native/unsupported.
    #[cfg(feature = "ffi-codecs")]
    if let Some(stages) = try_plan_ffi(method) {
        return stages;
    }

    match method {
        Method::Store => Vec::new(),

        Method::Srep(opts) => vec![Stage::Codec(Box::new(SrepCodec::new(opts.clone())))],

        Method::Lz4(opts) => vec![Stage::Codec(Box::new(Lz4Codec::new(*opts)))],

        Method::Ppmd(opts) => vec![Stage::Codec(Box::new(PpmdCodec::new(*opts)))],

        Method::Delta(opts) => match DeltaFilter::new(*opts) {
            Ok(f) => vec![Stage::Filter(Box::new(f))],
            Err(e) => vec![Stage::Unsupported(e.to_string())],
        },

        Method::BcjX86(_) => vec![Stage::Filter(Box::new(BcjX86Filter::new(BcjX86Options)))],

        Method::Dispack(opts) => vec![Stage::Filter(Box::new(DispackFilter::new(opts.clone())))],

        Method::RawBayer(opts) => vec![Stage::Filter(Box::new(RawBayerFilter::new(*opts)))],

        Method::RowFilter(opts) => match RowFilter::new(*opts) {
            Ok(f) => vec![Stage::Filter(Box::new(f))],
            Err(e) => vec![Stage::Unsupported(e.to_string())],
        },

        Method::Tornado(opts) => vec![Stage::Codec(Box::new(TornadoCodec::new(opts.clone())))],

        Method::Rep(opts) => vec![Stage::Codec(Box::new(RepCodec::new(opts.clone())))],

        Method::Dict(opts) => vec![Stage::Codec(Box::new(DictCodec::new(opts.clone())))],

        Method::Xz(opts) => vec![Stage::Codec(Box::new(XzCodec::new(*opts)))],

        Method::Zstd(opts) => vec![Stage::Codec(Box::new(ZstdCodec::new(opts.clone())))],

        Method::Brotli(opts) => vec![Stage::Codec(Box::new(BrotliCodec::new(*opts)))],

        Method::Snappy(opts) => vec![Stage::Codec(Box::new(SnappyCodec::new(*opts)))],

        Method::Lzma(opts) => vec![Stage::Codec(Box::new(LzmaCodec::new(*opts)))],

        Method::Lzp(opts) => vec![Stage::Codec(Box::new(LzpCodec::new(*opts)))],

        Method::Encryption(opts) => {
            if let Some(password) = &ctx.password {
                vec![Stage::Codec(Box::new(EncryptionCodec::new(
                    opts.clone(),
                    password.clone(),
                )))]
            } else {
                vec![Stage::Unsupported(
                    "encryption: no password provided in pipeline context".to_string(),
                )]
            }
        }

        Method::Ooxml => vec![Stage::Codec(Box::new(OoxmlCodec))],

        Method::TokenDict => vec![Stage::Codec(Box::new(TokenDictCodec))],

        Method::SqlTemplate => vec![Stage::Codec(Box::new(SqlTemplateCodec))],

        Method::FloatXor(opts) => match FloatXorFilter::new(*opts) {
            Ok(f) => vec![Stage::Filter(Box::new(f))],
            Err(e) => vec![Stage::Unsupported(e.to_string())],
        },

        Method::Lpc(opts) => match LpcFilter::new(*opts) {
            Ok(f) => vec![Stage::Filter(Box::new(f))],
            Err(e) => vec![Stage::Unsupported(e.to_string())],
        },

        Method::Pipeline(stages) => stages
            .iter()
            .flat_map(|s| plan_with_context(s, ctx))
            .collect(),

        #[cfg(feature = "bsc")]
        Method::Bsc(opts) => vec![Stage::Codec(Box::new(BscCodec::new(*opts)))],

        // Recognized but not yet natively implemented.
        other => vec![Stage::Unsupported(other.to_string())],
    }
}

/// Try to resolve `method` using an FFI-backed adapter.
///
/// Returns `Some(stages)` when a C bridge adapter is available for this method,
/// `None` to let the caller fall through to native or unsupported handling.
#[cfg(feature = "ffi-codecs")]
fn try_plan_ffi(method: &Method) -> Option<Vec<Stage>> {
    use crate::codec::{GrzipFfiCodec, LzmaFfiCodec, LzpFfiCodec, TornadoFfiCodec};
    match method {
        Method::Tornado(opts) => Some(vec![Stage::Codec(Box::new(TornadoFfiCodec::new(
            opts.clone(),
        )))]),
        Method::Lzma(opts) => Some(vec![Stage::Codec(Box::new(LzmaFfiCodec::new(*opts)))]),
        Method::Grzip(opts) => Some(vec![Stage::Codec(Box::new(GrzipFfiCodec::new(*opts)))]),
        Method::Lzp(opts) => Some(vec![Stage::Codec(Box::new(LzpFfiCodec::new(*opts)))]),
        _ => None,
    }
}
