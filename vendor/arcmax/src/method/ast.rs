use std::fmt;

use crate::codec::brotli::BrotliOptions;
use crate::codec::dict::DictOptions;
use crate::codec::filters::{
    BcjX86Options, DeltaOptions, DispackOptions, FloatXorOptions, LpcOptions, RawBayerOptions,
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
use crate::codec::xz::XzOptions;
use crate::codec::zstd::ZstdOptions;
use crate::crypto::EncryptionOptions;
use crate::srep::SrepConfig;

#[cfg(feature = "bsc")]
use crate::codec::BscOptions;

#[derive(Debug, Clone)]
pub enum Method {
    Store,
    Tornado(TornadoOptions),
    Lzma(LzmaOptions),
    Lz4(Lz4Options),
    Xz(XzOptions),
    Zstd(ZstdOptions),
    Brotli(BrotliOptions),
    Snappy(SnappyOptions),
    Ppmd(PpmdOptions),
    Rep(RepOptions),
    Srep(SrepConfig),
    Grzip(GrzipOptions),
    Lzp(LzpOptions),
    Dict(DictOptions),
    Delta(DeltaOptions),
    BcjX86(BcjX86Options),
    Dispack(DispackOptions),
    RawBayer(RawBayerOptions),
    RowFilter(RowFilterOptions),
    Encryption(EncryptionOptions),
    Blocked(BlockedOptions),
    Pipeline(Vec<Method>),
    /// OOXML/ODF ZIP unwrapper: strips deflate layer for better PPMd/LZMA compression.
    Ooxml,
    /// JSON/XML token-dictionary pre-compressor.
    TokenDict,
    /// SQL INSERT row-template pre-compressor.
    SqlTemplate,
    /// Float XOR-difference predictor (f32/f64 scientific data).
    FloatXor(FloatXorOptions),
    /// LPC audio predictor (16-bit LE PCM).
    Lpc(LpcOptions),
    /// Sentinel for the archive writer: resolve the best method per-file at
    /// write time via the filetype router. Never stored in a block header.
    Auto,
    /// libbsc BWT+QLFC codec — requires the `bsc` feature.
    #[cfg(feature = "bsc")]
    Bsc(BscOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedOptions {
    pub inner: Box<Method>,
    pub block_size: usize,
    pub threads: usize,
    pub min_compress_ratio: Option<u8>,
}

impl Default for Method {
    fn default() -> Self {
        Method::Store
    }
}

impl PartialEq for Method {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl Eq for Method {}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Method::Store => f.write_str("storing"),
            Method::Tornado(options) => match options.level {
                Some(level) => write!(f, "tornado:{level}"),
                None => f.write_str("tornado"),
            },
            Method::Lzma(options) => {
                let name = if options.lzma2 { "lzma2" } else { "lzma" };
                write!(
                    f,
                    "{name}:d{}:lc{}:lp{}:pb{}",
                    options.dict_size, options.lc, options.lp, options.pb
                )?;
                if let Some(nice_len) = options.nice_len {
                    write!(f, ":fb{nice_len}")?;
                }
                Ok(())
            }
            Method::Lz4(_) => f.write_str("lz4"),
            Method::Xz(options) => match options.level {
                Some(l) => write!(f, "xz:{l}"),
                None => f.write_str("xz"),
            },
            Method::Zstd(options) => write!(f, "zstd:{}", options.level),
            Method::Brotli(options) => {
                write!(f, "brotli:q{}:w{}", options.quality, options.lgwin)
            }
            Method::Snappy(_) => f.write_str("snappy"),
            Method::Ppmd(options) => {
                write!(f, "ppmd:o{}:mem{}", options.order, options.memory_size)
            }
            Method::Rep(_) => f.write_str("rep"),
            Method::Srep(_) => f.write_str("srep"),
            Method::Grzip(options) => write!(f, "grzip:{}", options.mode),
            Method::Lzp(options) => {
                write!(f, "lzp:m{}:h{}", options.min_match, options.hash_size_log)
            }
            Method::Dict(_) => f.write_str("dict"),
            Method::Delta(options) => write!(f, "delta:{}", options.stride),
            Method::BcjX86(_) => f.write_str("exe"),
            Method::Dispack(_) => f.write_str("dispack"),
            Method::RawBayer(_) => f.write_str("rawbayer"),
            Method::RowFilter(options) => {
                write!(
                    f,
                    "rowfilter:s{}:bpp{}",
                    options.row_stride, options.bytes_per_pixel
                )
            }
            Method::Encryption(_) => f.write_str("encryption"),
            Method::Blocked(options) => {
                write!(
                    f,
                    "4x4:b{}:t{}:{}",
                    options.block_size, options.threads, options.inner
                )
            }
            Method::Pipeline(stages) => {
                let parts = stages.iter().map(ToString::to_string).collect::<Vec<_>>();
                f.write_str(&parts.join("+"))
            }
            Method::Ooxml => f.write_str("ooxml"),
            Method::TokenDict => f.write_str("tokendict"),
            Method::SqlTemplate => f.write_str("sqltemplate"),
            Method::FloatXor(opts) => write!(
                f,
                "floatxor:{}:{}",
                if opts.precision == crate::codec::filters::FloatPrecision::F32 {
                    "f32"
                } else {
                    "f64"
                },
                opts.stride
            ),
            Method::Lpc(opts) => write!(f, "lpc:{}", opts.max_order),
            Method::Auto => f.write_str("auto"),
            #[cfg(feature = "bsc")]
            Method::Bsc(_) => f.write_str("bsc"),
        }
    }
}

impl Method {
    /// Return the static performance and resource profile for this method.
    ///
    /// For [`Method::Pipeline`], capabilities are composed sequentially
    /// (speed = bottleneck, memory = sum, ratio = product).
    pub fn capabilities(&self) -> crate::codec::traits::CodecCapabilities {
        use crate::codec::traits::CodecCapabilities;

        // Convenience macro to reduce repetition.
        macro_rules! cap {
            (comp=$c:expr, decomp=$d:expr, ratio=$r:expr, min=$m:expr, mem=$mem:expr, par=$p:expr) => {
                CodecCapabilities {
                    compress_speed_mb_per_sec: $c,
                    decompress_speed_mb_per_sec: $d,
                    typical_ratio_pct: $r,
                    min_useful_bytes: $m,
                    peak_memory_mib: $mem,
                    parallelizable: $p,
                }
            };
        }

        match self {
            Method::Store => cap!(
                comp = 10_000,
                decomp = 10_000,
                ratio = 100,
                min = 0,
                mem = 1,
                par = true
            ),

            // ── LZ-family ────────────────────────────────────────────────────
            Method::Lz4(_) => cap!(
                comp = 500,
                decomp = 2_000,
                ratio = 45,
                min = 64,
                mem = 4,
                par = false
            ),
            Method::Snappy(_) => cap!(
                comp = 400,
                decomp = 1_800,
                ratio = 50,
                min = 64,
                mem = 4,
                par = false
            ),
            Method::Zstd(o) => {
                let lvl = o.level;
                if lvl <= 3 {
                    cap!(
                        comp = 400,
                        decomp = 1_000,
                        ratio = 28,
                        min = 256,
                        mem = 32,
                        par = true
                    )
                } else if lvl <= 9 {
                    cap!(
                        comp = 80,
                        decomp = 1_000,
                        ratio = 24,
                        min = 512,
                        mem = 64,
                        par = true
                    )
                } else {
                    cap!(
                        comp = 15,
                        decomp = 900,
                        ratio = 20,
                        min = 1024,
                        mem = 128,
                        par = true
                    )
                }
            }
            Method::Lzma(o) => {
                let lvl = o.level.unwrap_or(5);
                let mem_mib = (o.dict_size / (1024 * 1024)).max(4) as u32;
                if lvl <= 5 {
                    cap!(
                        comp = 15,
                        decomp = 100,
                        ratio = 22,
                        min = 4096,
                        mem = mem_mib * 3,
                        par = false
                    )
                } else {
                    cap!(
                        comp = 5,
                        decomp = 80,
                        ratio = 18,
                        min = 4096,
                        mem = mem_mib * 3,
                        par = false
                    )
                }
            }
            Method::Xz(_) => cap!(
                comp = 8,
                decomp = 80,
                ratio = 18,
                min = 4096,
                mem = 64,
                par = false
            ),
            Method::Lzp(_) => cap!(
                comp = 300,
                decomp = 300,
                ratio = 38,
                min = 512,
                mem = 8,
                par = false
            ),

            // ── Statistical / BWT ────────────────────────────────────────────
            Method::Ppmd(o) => {
                let mem_mib = o.memory_size / (1024 * 1024);
                cap!(
                    comp = 30,
                    decomp = 30,
                    ratio = 20,
                    min = 4096,
                    mem = mem_mib as u32,
                    par = false
                )
            }
            Method::Brotli(_) => cap!(
                comp = 25,
                decomp = 500,
                ratio = 22,
                min = 1024,
                mem = 16,
                par = false
            ),
            Method::Grzip(_) => cap!(
                comp = 5,
                decomp = 15,
                ratio = 18,
                min = 4096,
                mem = 64,
                par = false
            ),
            Method::Tornado(_) => cap!(
                comp = 150,
                decomp = 400,
                ratio = 32,
                min = 1024,
                mem = 32,
                par = false
            ),

            // ── Pre-processors (no compression alone) ───────────────────────
            Method::Srep(_) => cap!(
                comp = 200,
                decomp = 600,
                ratio = 100,
                min = 65536,
                mem = 128,
                par = false
            ),
            Method::Rep(_) => cap!(
                comp = 150,
                decomp = 400,
                ratio = 90,
                min = 4096,
                mem = 32,
                par = false
            ),
            Method::Dict(_) => cap!(
                comp = 200,
                decomp = 500,
                ratio = 90,
                min = 256,
                mem = 8,
                par = false
            ),

            // ── Byte-level filters ───────────────────────────────────────────
            Method::Delta(_) => cap!(
                comp = 5_000,
                decomp = 5_000,
                ratio = 100,
                min = 0,
                mem = 1,
                par = true
            ),
            Method::BcjX86(_) => cap!(
                comp = 5_000,
                decomp = 5_000,
                ratio = 100,
                min = 0,
                mem = 1,
                par = true
            ),
            Method::Dispack(_) => cap!(
                comp = 3_000,
                decomp = 3_000,
                ratio = 100,
                min = 0,
                mem = 1,
                par = true
            ),
            Method::RawBayer(_) => cap!(
                comp = 3_000,
                decomp = 3_000,
                ratio = 100,
                min = 0,
                mem = 2,
                par = true
            ),
            Method::RowFilter(_) => cap!(
                comp = 4_000,
                decomp = 4_000,
                ratio = 100,
                min = 0,
                mem = 2,
                par = true
            ),
            Method::FloatXor(_) => cap!(
                comp = 4_000,
                decomp = 4_000,
                ratio = 100,
                min = 0,
                mem = 1,
                par = true
            ),
            Method::Lpc(_) => cap!(
                comp = 2_000,
                decomp = 2_000,
                ratio = 100,
                min = 0,
                mem = 2,
                par = true
            ),

            // ── Format-aware pre-compressors ─────────────────────────────────
            Method::Ooxml => cap!(
                comp = 200,
                decomp = 200,
                ratio = 70,
                min = 1024,
                mem = 16,
                par = false
            ),
            Method::TokenDict => cap!(
                comp = 300,
                decomp = 300,
                ratio = 65,
                min = 512,
                mem = 8,
                par = false
            ),
            Method::SqlTemplate => cap!(
                comp = 400,
                decomp = 400,
                ratio = 40,
                min = 128,
                mem = 4,
                par = false
            ),

            // ── Encryption ───────────────────────────────────────────────────
            // AES-NI path reaches several GB/s; 5 GB/s is conservative.
            Method::Encryption(_) => cap!(
                comp = 5_000,
                decomp = 5_000,
                ratio = 100,
                min = 0,
                mem = 1,
                par = false
            ),

            // ── Pipeline ─────────────────────────────────────────────────────
            Method::Pipeline(stages) => stages
                .iter()
                .fold(None::<CodecCapabilities>, |acc, stage| {
                    let c = stage.capabilities();
                    Some(match acc {
                        None => c,
                        Some(prev) => prev.compose(c),
                    })
                })
                .unwrap_or_default(),

            // ── Blocked / Auto / BSC ─────────────────────────────────────────
            Method::Blocked(opts) => opts.inner.capabilities(),
            Method::Auto => CodecCapabilities::default(),
            #[cfg(feature = "bsc")]
            Method::Bsc(_) => cap!(
                comp = 50,
                decomp = 300,
                ratio = 17,
                min = 4096,
                mem = 256,
                par = true
            ),
        }
    }
}
