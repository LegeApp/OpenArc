use std::str::FromStr;

use crate::codec::brotli::BrotliOptions;
use crate::codec::dict::DictOptions;
use crate::codec::filters::{
    BcjX86Options, DeltaOptions, DispackOptions, FloatPrecision, FloatXorOptions, LpcOptions,
    RowFilterOptions,
};
use crate::codec::grzip::GrzipOptions;
use crate::codec::lz4::Lz4Options;
use crate::codec::lzma::LzmaOptions;
use crate::codec::lzp::LzpOptions;
use crate::codec::ppmd::PpmdOptions;
use crate::codec::rep::RepOptions;
use crate::codec::snappy::SnappyOptions;
use crate::codec::tornado::TornadoOptions;
use crate::codec::zstd::ZstdOptions;
use crate::crypto::{CipherAlgorithm, CipherMode, EncryptionOptions};
use crate::error::ArcError;
use crate::method::ast::{BlockedOptions, Method};
use crate::srep::SrepConfig;

impl FromStr for Method {
    type Err = ArcError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        MethodParser::new(s).parse()
    }
}

#[derive(Debug, Clone)]
pub struct MethodParser<'a> {
    input: &'a str,
}

impl<'a> MethodParser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    pub fn parse(&self) -> Result<Method, ArcError> {
        let trimmed = self.input.trim();
        if trimmed.is_empty() {
            return Ok(Method::Store);
        }

        let stages = trimmed
            .split('+')
            .filter(|part| !part.trim().is_empty())
            .map(parse_stage)
            .collect::<Result<Vec<_>, _>>()?;

        match stages.as_slice() {
            [] => Ok(Method::Store),
            [single] => Ok(single.clone()),
            _ => Ok(Method::Pipeline(stages)),
        }
    }
}

fn parse_stage(stage: &str) -> Result<Method, ArcError> {
    let mut parts = stage.split(':');
    let name = parts.next().unwrap_or("").trim().to_ascii_lowercase();
    let params = parts
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();

    // Encryption method names may include the cipher mode: "blowfish-448/ctr", "aes-256/cfb".
    if is_cipher_name(&name) {
        return parse_encryption(&name, &params);
    }

    match name.as_str() {
        "" | "store" | "storing" => Ok(Method::Store),
        "tornado" | "tor" => parse_tornado(&params),
        "rep" => parse_rep(&params),
        "srep" => parse_srep(&params),
        "dict" => parse_dict(&params),
        "lzp" => parse_lzp(&params),
        "grzip" => parse_grzip(&params),
        "lzma" => parse_lzma(false, &params),
        "lzma2" => parse_lzma(true, &params),
        "lz4" => Ok(Method::Lz4(Lz4Options::default())),
        "ppmd" | "ppmdh" | "ppmd7" => parse_ppmd(&params),
        "delta" => parse_delta(&params),
        "exe" | "bcj" | "bcj_x86" | "bcj-x86" | "x86" => Ok(Method::BcjX86(BcjX86Options)),
        "dispack" => Ok(Method::Dispack(DispackOptions::default())),
        "rawbayer" | "raw-bayer" | "raw_bayer" => {
            Ok(Method::RawBayer(crate::codec::filters::RawBayerOptions))
        }
        "rowfilter" | "row-filter" | "row_filter" | "png" => parse_row_filter(&params),
        "4x4" => parse_blocked(&params),
        "xz" => parse_xz(&params),
        "zstd" | "zst" => parse_zstd(&params),
        "brotli" | "br" => parse_brotli(&params),
        "snappy" | "snap" => Ok(Method::Snappy(SnappyOptions)),
        "ooxml" | "odf" => Ok(Method::Ooxml),
        "tokendict" | "tknd" => Ok(Method::TokenDict),
        "sqltemplate" | "sqlt" => Ok(Method::SqlTemplate),
        "floatxor" | "float-xor" | "float_xor" => parse_floatxor(&params),
        "lpc" => parse_lpc(&params),
        "encrypt" | "encryption" => parse_encryption(&name, &params),
        #[cfg(feature = "bsc")]
        "bsc" => Ok(Method::Bsc(crate::codec::BscOptions::default())),
        _ => Err(ArcError::InvalidMethod(format!("unknown method: {stage}"))),
    }
}

// Returns true for names that look like cipher specifications ("aes…", "blowfish…").
fn is_cipher_name(name: &str) -> bool {
    name.starts_with("aes") || name.starts_with("blowfish")
}

fn parse_lzma(lzma2: bool, params: &[&str]) -> Result<Method, ArcError> {
    let mut options = LzmaOptions {
        lzma2,
        ..LzmaOptions::default()
    };
    for param in params {
        if let Some(v) = param.strip_prefix('d') {
            options.dict_size = parse_size_u32(v)?;
        } else if let Some(v) = param.strip_prefix("lc") {
            options.lc = v
                .parse::<u32>()
                .map_err(|_| ArcError::InvalidMethod(format!("invalid LZMA lc: {param}")))?;
        } else if let Some(v) = param.strip_prefix("lp") {
            options.lp = v
                .parse::<u32>()
                .map_err(|_| ArcError::InvalidMethod(format!("invalid LZMA lp: {param}")))?;
        } else if let Some(v) = param.strip_prefix("pb") {
            options.pb = v
                .parse::<u32>()
                .map_err(|_| ArcError::InvalidMethod(format!("invalid LZMA pb: {param}")))?;
        } else if let Some(v) = param
            .strip_prefix("fb")
            .or_else(|| param.strip_prefix("nice"))
        {
            options.nice_len =
                Some(v.parse::<u32>().map_err(|_| {
                    ArcError::InvalidMethod(format!("invalid LZMA nice_len: {param}"))
                })?);
        } else if let Some(v) = param.strip_prefix('l') {
            options.level =
                Some(v.parse::<u8>().map_err(|_| {
                    ArcError::InvalidMethod(format!("invalid LZMA level: {param}"))
                })?);
        } else if let Ok(level) = param.parse::<u8>() {
            // Bare number treated as compression level preset.
            options.level = Some(level);
        } else {
            return Err(ArcError::InvalidMethod(format!(
                "unknown LZMA parameter: {param}"
            )));
        }
    }
    Ok(Method::Lzma(options))
}

fn parse_srep(params: &[&str]) -> Result<Method, ArcError> {
    use crate::srep::config::{Acceleration, Method as SrepMethod};
    let mut config = SrepConfig::default();
    for param in params {
        if let Some(v) = param.strip_prefix('m') {
            config.method = match v {
                "0" => SrepMethod::M0,
                "1" => SrepMethod::M1,
                "2" => SrepMethod::M2,
                "3" => SrepMethod::M3,
                "4" => SrepMethod::M4,
                "5" => SrepMethod::M5,
                _ => {
                    return Err(ArcError::InvalidMethod(format!(
                        "invalid SREP method: {param}"
                    )))
                }
            };
        } else if let Some(v) = param.strip_prefix('b') {
            config.block_size = parse_size(v)?;
        } else if let Some(v) = param.strip_prefix('l') {
            config.min_match = parse_usize(v, param)?;
        } else if let Some(v) = param.strip_prefix('a') {
            let accel = v.parse::<u8>().map_err(|_| {
                ArcError::InvalidMethod(format!("invalid SREP acceleration: {param}"))
            })?;
            config.acceleration = Acceleration(accel);
        } else {
            return Err(ArcError::InvalidMethod(format!(
                "unknown SREP parameter: {param}"
            )));
        }
    }
    Ok(Method::Srep(config))
}

fn parse_rep(params: &[&str]) -> Result<Method, ArcError> {
    let mut options = RepOptions::default();
    for param in params {
        // "mem" must be checked before 'm' to avoid partial prefix match.
        if let Some(v) = param.strip_prefix("mem") {
            options.block_size = parse_size(v)?;
        } else if let Some(v) = param.strip_prefix('b') {
            options.block_size = parse_size(v)?;
        } else if let Some(v) = param.strip_prefix('l') {
            options.min_match_len = parse_i32(v, param)?;
        } else if let Some(v) = param.strip_prefix('m') {
            options.min_match_len = parse_i32(v, param)?;
        } else if let Some(v) = param.strip_prefix('c') {
            options.chunk_size = parse_usize(v, param)?;
        } else if let Some(v) = param.strip_prefix('h') {
            options.hash_size_log = parse_u32(v, param)?;
        } else if let Some(v) = param.strip_prefix('d') {
            options.barrier = parse_i32(v, param)?;
        } else if let Some(v) = param.strip_prefix('s') {
            options.smallest_len = parse_i32(v, param)?;
        } else if let Some(v) = param.strip_prefix('a') {
            options.amplifier = parse_i32(v, param)?;
        } else if param.ends_with('%') {
            let n = param.trim_end_matches('%');
            options.min_compression = parse_i32(n, param)?;
        } else if *param == "f" {
            options.amplifier = 99;
        } else {
            // C++ parse_REP treats an unnamed numeric parameter as MinMatchLen,
            // otherwise it tries to parse it as BlockSize (e.g. "rep:64m").
            match param.parse::<i32>() {
                Ok(n) => options.min_match_len = n,
                Err(_) => options.block_size = parse_size(param)?,
            }
        }
    }
    Ok(Method::Rep(options))
}

fn parse_dict(params: &[&str]) -> Result<Method, ArcError> {
    let mut options = DictOptions::default();
    for param in params {
        if *param == "p" {
            options.min_large_cnt = 8192;
            options.min_medium_cnt = 400;
            options.min_small_cnt = 100;
            options.min_ratio = 4;
        } else if *param == "f" {
            options.min_large_cnt = 2048;
            options.min_medium_cnt = 100;
            options.min_small_cnt = 50;
            options.min_ratio = 0;
        } else if let Some(v) = param.strip_prefix('b') {
            options.block_size = parse_size(v)?;
        } else if let Some(v) = param.strip_prefix('c') {
            options.min_weak_chars = parse_i32(v, param)?;
        } else if let Some(v) = param.strip_prefix('l') {
            options.min_large_cnt = parse_i32(v, param)?;
        } else if let Some(v) = param.strip_prefix('m') {
            options.min_medium_cnt = parse_i32(v, param)?;
        } else if let Some(v) = param.strip_prefix('s') {
            options.min_small_cnt = parse_i32(v, param)?;
        } else if let Some(v) = param.strip_prefix('r') {
            options.min_ratio = parse_i32(v, param)?;
        } else if param.ends_with('%') {
            let n = param.trim_end_matches('%');
            options.min_compression = parse_i32(n, param)?;
        } else {
            // C++ parse_DICT treats an unnamed numeric parameter as MinWeakChars,
            // otherwise it tries to parse it as BlockSize (e.g. "dict:64m").
            match param.parse::<i32>() {
                Ok(n) => options.min_weak_chars = n,
                Err(_) => options.block_size = parse_size(param)?,
            }
        }
    }
    Ok(Method::Dict(options))
}

fn parse_encryption(name: &str, _params: &[&str]) -> Result<Method, ArcError> {
    // Name may embed the mode: "blowfish-448/ctr", "aes-256/cfb".
    let (cipher_part, mode_part) = match name.find('/') {
        Some(pos) => (&name[..pos], Some(&name[pos + 1..])),
        None => (name, None),
    };

    let cipher = match cipher_part {
        "aes" | "aes-256" | "encrypt" | "encryption" => CipherAlgorithm::Aes256,
        "blowfish" | "blowfish-448" => CipherAlgorithm::Blowfish448,
        _ => {
            return Err(ArcError::InvalidMethod(format!(
                "unknown cipher: {cipher_part}"
            )))
        }
    };

    let mode = match mode_part {
        None | Some("ctr") => CipherMode::Ctr,
        Some("cfb") => CipherMode::Cfb,
        Some(other) => {
            return Err(ArcError::InvalidMethod(format!(
                "unknown cipher mode: {other}"
            )))
        }
    };

    Ok(Method::Encryption(EncryptionOptions {
        cipher,
        mode,
        ..EncryptionOptions::default()
    }))
}

fn parse_tornado(params: &[&str]) -> Result<Method, ArcError> {
    let level = params
        .first()
        .and_then(|p| p.parse::<u8>().ok())
        .unwrap_or(5);
    Ok(Method::Tornado(TornadoOptions::preset(level)?))
}

fn parse_lzp(params: &[&str]) -> Result<Method, ArcError> {
    let mut options = LzpOptions::default();
    for param in params {
        if let Some(value) = param.strip_prefix('m') {
            options.min_match = parse_usize(value, param)?;
        } else if let Some(value) = param.strip_prefix('h') {
            options.hash_size_log = value
                .parse::<u8>()
                .map_err(|_| ArcError::InvalidMethod(format!("invalid LZP hash size: {param}")))?;
        }
    }
    Ok(Method::Lzp(options))
}

fn parse_grzip(params: &[&str]) -> Result<Method, ArcError> {
    let mode = params
        .first()
        .map(|value| {
            value
                .parse::<i32>()
                .map_err(|_| ArcError::InvalidMethod(format!("invalid GRZip mode: {value}")))
        })
        .transpose()?
        .unwrap_or_default();
    Ok(Method::Grzip(GrzipOptions { mode }))
}

fn parse_ppmd(params: &[&str]) -> Result<Method, ArcError> {
    let mut options = PpmdOptions::default();
    for param in params {
        if let Some(value) = param.strip_prefix('o') {
            options.order = value
                .parse::<u8>()
                .map_err(|_| ArcError::InvalidMethod(format!("invalid PPMD order: {param}")))?;
        } else if let Some(value) = param.strip_prefix("mem") {
            options.memory_size = parse_size(value)?;
        } else if let Ok(order) = param.parse::<u8>() {
            options.order = order;
        } else {
            options.memory_size = parse_size(param)?;
        }
    }
    options.validate()?;
    Ok(Method::Ppmd(options))
}

fn parse_delta(params: &[&str]) -> Result<Method, ArcError> {
    let stride = params
        .first()
        .map(|value| parse_usize(value, value))
        .transpose()?
        .unwrap_or(1);
    if stride == 0 {
        return Err(ArcError::InvalidMethod(
            "delta stride must be greater than zero".to_string(),
        ));
    }
    Ok(Method::Delta(DeltaOptions { stride }))
}

fn parse_blocked(params: &[&str]) -> Result<Method, ArcError> {
    let mut block_size = 16 * 1024 * 1024;
    let mut threads = 1usize;
    let mut inner = Method::Store;

    for param in params {
        if let Some(value) = param.strip_prefix('b') {
            block_size = parse_size(value)?;
        } else if let Some(value) = param.strip_prefix('t') {
            threads = parse_usize(value, param)?;
        } else {
            inner = Method::from_str(param)?;
        }
    }

    Ok(Method::Blocked(BlockedOptions {
        inner: Box::new(inner),
        block_size,
        threads,
        min_compress_ratio: None,
    }))
}

fn parse_xz(params: &[&str]) -> Result<Method, ArcError> {
    use crate::codec::xz::XzOptions;
    let mut opts = XzOptions::default();
    for param in params {
        if let Ok(level) = param.parse::<u8>() {
            opts.level = Some(level.min(9));
        } else if let Some(v) = param.strip_prefix('d') {
            opts.dict_size = parse_size(v)? as u32;
        } else {
            return Err(ArcError::InvalidMethod(format!(
                "unknown xz parameter: {param}"
            )));
        }
    }
    Ok(Method::Xz(opts))
}

fn parse_zstd(params: &[&str]) -> Result<Method, ArcError> {
    let level = params
        .first()
        .map(|value| {
            value
                .parse::<i32>()
                .map_err(|_| ArcError::InvalidMethod(format!("invalid zstd level: {value}")))
        })
        .transpose()?
        .unwrap_or(3);
    Ok(Method::Zstd(ZstdOptions { level }))
}

fn parse_row_filter(params: &[&str]) -> Result<Method, ArcError> {
    let mut options = RowFilterOptions::default();
    for param in params {
        if let Some(v) = param.strip_prefix('s') {
            options.row_stride = parse_size(v)?;
        } else if let Some(v) = param.strip_prefix("bpp") {
            options.bytes_per_pixel = v
                .parse::<usize>()
                .map_err(|_| ArcError::InvalidMethod(format!("invalid row_filter bpp: {param}")))?;
        } else {
            return Err(ArcError::InvalidMethod(format!(
                "unknown row_filter parameter: {param}"
            )));
        }
    }
    Ok(Method::RowFilter(options))
}

fn parse_brotli(params: &[&str]) -> Result<Method, ArcError> {
    let mut options = BrotliOptions::default();
    for param in params {
        if let Some(v) = param.strip_prefix('q') {
            options.quality = v
                .parse::<u32>()
                .map_err(|_| ArcError::InvalidMethod(format!("invalid brotli quality: {param}")))?;
            if options.quality > 11 {
                return Err(ArcError::InvalidMethod(format!(
                    "brotli quality must be 0..=11, got {}",
                    options.quality
                )));
            }
        } else if let Some(v) = param.strip_prefix('w') {
            options.lgwin = v
                .parse::<u32>()
                .map_err(|_| ArcError::InvalidMethod(format!("invalid brotli lgwin: {param}")))?;
            if !(10..=24).contains(&options.lgwin) {
                return Err(ArcError::InvalidMethod(format!(
                    "brotli lgwin must be 10..=24, got {}",
                    options.lgwin
                )));
            }
        } else if let Ok(q) = param.parse::<u32>() {
            // Bare number is treated as quality.
            if q > 11 {
                return Err(ArcError::InvalidMethod(format!(
                    "brotli quality must be 0..=11, got {q}"
                )));
            }
            options.quality = q;
        } else {
            return Err(ArcError::InvalidMethod(format!(
                "unknown brotli parameter: {param}"
            )));
        }
    }
    Ok(Method::Brotli(options))
}

fn parse_i32(value: &str, original: &str) -> Result<i32, ArcError> {
    value
        .parse::<i32>()
        .map_err(|_| ArcError::InvalidMethod(format!("invalid numeric parameter: {original}")))
}

fn parse_u32(value: &str, original: &str) -> Result<u32, ArcError> {
    value
        .parse::<u32>()
        .map_err(|_| ArcError::InvalidMethod(format!("invalid numeric parameter: {original}")))
}

fn parse_usize(value: &str, original: &str) -> Result<usize, ArcError> {
    value
        .parse::<usize>()
        .map_err(|_| ArcError::InvalidMethod(format!("invalid numeric parameter: {original}")))
}

fn parse_size(value: &str) -> Result<usize, ArcError> {
    let lower = value.to_ascii_lowercase();
    let (digits, multiplier) = if let Some(digits) = lower.strip_suffix('g') {
        (digits, 1024usize * 1024 * 1024)
    } else if let Some(digits) = lower.strip_suffix('m') {
        (digits, 1024usize * 1024)
    } else if let Some(digits) = lower.strip_suffix('k') {
        (digits, 1024usize)
    } else {
        (lower.as_str(), 1usize)
    };

    let base = digits
        .parse::<usize>()
        .map_err(|_| ArcError::InvalidMethod(format!("invalid size: {value}")))?;
    base.checked_mul(multiplier)
        .ok_or_else(|| ArcError::InvalidMethod(format!("size overflows usize: {value}")))
}

fn parse_floatxor(params: &[&str]) -> Result<Method, ArcError> {
    let mut opts = FloatXorOptions::default();
    for param in params {
        match *param {
            "f32" | "32" => opts.precision = FloatPrecision::F32,
            "f64" | "64" => opts.precision = FloatPrecision::F64,
            _ => {
                if let Some(v) = param.strip_prefix('s') {
                    opts.stride = parse_usize(v, param)?;
                } else if let Ok(n) = param.parse::<usize>() {
                    opts.stride = n;
                } else {
                    return Err(ArcError::InvalidMethod(format!(
                        "unknown floatxor parameter: {param}"
                    )));
                }
            }
        }
    }
    if opts.stride == 0 {
        return Err(ArcError::InvalidMethod(
            "floatxor stride must be > 0".to_string(),
        ));
    }
    Ok(Method::FloatXor(opts))
}

fn parse_lpc(params: &[&str]) -> Result<Method, ArcError> {
    let mut opts = LpcOptions::default();
    for param in params {
        if let Ok(n) = param.parse::<usize>() {
            opts.max_order = n;
        } else if let Some(v) = param.strip_prefix('o') {
            opts.max_order = parse_usize(v, param)?;
        } else {
            return Err(ArcError::InvalidMethod(format!(
                "unknown lpc parameter: {param}"
            )));
        }
    }
    Ok(Method::Lpc(opts))
}

fn parse_size_u32(value: &str) -> Result<u32, ArcError> {
    let n = parse_size(value)?;
    u32::try_from(n)
        .map_err(|_| ArcError::InvalidMethod(format!("size too large for 32-bit field: {value}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lzma_fast_bytes() {
        let method = Method::from_str("lzma2:9:d128m:fb273").expect("parse lzma2 fb");
        match method {
            Method::Lzma(options) => {
                assert!(options.lzma2);
                assert_eq!(options.level, Some(9));
                assert_eq!(options.dict_size, 128 * 1024 * 1024);
                assert_eq!(options.nice_len, Some(273));
            }
            other => panic!("expected lzma method, got {other:?}"),
        }
    }
}
