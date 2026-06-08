use std::io::{Cursor, Read, Write};

use lzma_rust2::{CountingWriter, LZMA2Options, LZMA2Reader, LZMA2Writer, LZMAReader, LZMAWriter};

use crate::codec::framing;
use crate::codec::lzma::LzmaOptions;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

/// Native LZMA/LZMA2 codec backed by `lzma-rust2`.
///
/// Framing matches `LzmaFfiCodec`: an 8-byte LE uncompressed-size header
/// precedes the raw LZMA/LZMA2 stream so the decoder knows how much to allocate.
///
/// When `options.lzma2 == true`, produces raw LZMA2 chunks (no XZ container).
/// When `options.lzma2 == false`, produces a raw LZMA1 stream with an end marker.
pub struct LzmaCodec {
    options: LzmaOptions,
}

impl LzmaCodec {
    pub fn new(options: LzmaOptions) -> Self {
        Self { options }
    }

    fn lzma2_opts(&self) -> LZMA2Options {
        let preset = self.options.level.unwrap_or(5) as u32;
        let mut opts = LZMA2Options::with_preset(preset.min(9));
        opts.dict_size = self.options.dict_size;
        opts.lc = self.options.lc;
        opts.lp = self.options.lp;
        opts.pb = self.options.pb;
        if let Some(nice_len) = self.options.nice_len {
            opts.nice_len = nice_len.clamp(LZMA2Options::NICE_LEN_MIN, LZMA2Options::NICE_LEN_MAX);
        }
        opts
    }
}

impl Codec for LzmaCodec {
    fn name(&self) -> &'static str {
        if self.options.lzma2 {
            "lzma2"
        } else {
            "lzma"
        }
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        input.read_to_end(&mut source)?;
        let uncompressed_len = source.len();

        let mut compressed = Vec::new();
        let opts = self.lzma2_opts();

        if self.options.lzma2 {
            let cw = CountingWriter::new(&mut compressed);
            let mut writer = LZMA2Writer::new(cw, &opts);
            writer.write_all(&source).map_err(|e| ArcError::Codec {
                codec: "lzma2",
                message: format!("encode failed: {e}"),
            })?;
            writer.finish().map_err(|e| ArcError::Codec {
                codec: "lzma2",
                message: format!("encode finish failed: {e}"),
            })?;
        } else {
            let cw = CountingWriter::new(&mut compressed);
            let mut writer =
                LZMAWriter::new_no_header(cw, &opts, true).map_err(|e| ArcError::Codec {
                    codec: "lzma",
                    message: format!("encoder init failed: {e}"),
                })?;
            writer.write_all(&source).map_err(|e| ArcError::Codec {
                codec: "lzma",
                message: format!("encode failed: {e}"),
            })?;
            writer.finish().map_err(|e| ArcError::Codec {
                codec: "lzma",
                message: format!("encode finish failed: {e}"),
            })?;
        }

        framing::write_size_header(uncompressed_len, output)?;
        output.write_all(&compressed)?;

        Ok(CodecReport {
            bytes_in: uncompressed_len as u64,
            bytes_out: (framing::SIZE_HEADER_LEN + compressed.len()) as u64,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let expected_len = framing::read_size_header(input)?;

        let mut compressed = Vec::new();
        input.read_to_end(&mut compressed)?;
        let bytes_in = (framing::SIZE_HEADER_LEN + compressed.len()) as u64;

        let decompressed = if self.options.lzma2 {
            let mut reader =
                LZMA2Reader::new(Cursor::new(&compressed), self.options.dict_size, None);
            let mut buf = Vec::with_capacity(expected_len);
            reader.read_to_end(&mut buf).map_err(|e| ArcError::Codec {
                codec: "lzma2",
                message: format!("decode failed: {e}"),
            })?;
            buf
        } else {
            let reader = LZMAReader::new(
                Cursor::new(&compressed),
                expected_len as u64,
                self.options.lc,
                self.options.lp,
                self.options.pb,
                self.options.dict_size,
                None,
            )
            .map_err(|e| ArcError::Codec {
                codec: "lzma",
                message: format!("decoder init failed: {e}"),
            })?;
            let mut buf = Vec::with_capacity(expected_len);
            reader
                .take(expected_len as u64)
                .read_to_end(&mut buf)
                .map_err(|e| ArcError::Codec {
                    codec: "lzma",
                    message: format!("decode failed: {e}"),
                })?;
            buf
        };

        output.write_all(&decompressed)?;

        Ok(CodecReport {
            bytes_in,
            bytes_out: decompressed.len() as u64,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage {
            working_bytes: self.options.dict_size as u64 * 2,
            ..MemoryUsage::default()
        }
    }

    fn capabilities(&self) -> crate::codec::traits::CodecCapabilities {
        let lvl = self.options.level.unwrap_or(5);
        let dict_mib = (self.options.dict_size / (1024 * 1024)).max(4) as u32;
        let (comp, ratio) = if lvl <= 5 { (15u32, 22u8) } else { (5, 18) };
        crate::codec::traits::CodecCapabilities {
            compress_speed_mb_per_sec: comp,
            decompress_speed_mb_per_sec: 100,
            typical_ratio_pct: ratio,
            min_useful_bytes: 4096,
            peak_memory_mib: dict_mib * 3,
            parallelizable: false,
        }
    }
}
