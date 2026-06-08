use std::io::{Cursor, Read, Write};

use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::codec::zstd::ZstdOptions;
use crate::error::{ArcError, Result};

/// Zstd codec backed by the `zstd` crate (libzstd C binding).
///
/// Zstd frames are self-describing (magic `0xFD2FB528`); no size-prepend header
/// is needed. Levels 1–22 are supported (libzstd standard range); negative
/// levels select "ultra-fast" modes. Default is level 3.
pub struct ZstdCodec {
    options: ZstdOptions,
}

impl ZstdCodec {
    pub fn new(options: ZstdOptions) -> Self {
        Self { options }
    }
}

impl Codec for ZstdCodec {
    fn name(&self) -> &'static str {
        "zstd"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        input.read_to_end(&mut source)?;
        let uncompressed_len = source.len();

        let compressed =
            zstd::encode_all(Cursor::new(&source), self.options.level).map_err(|e| {
                ArcError::Codec {
                    codec: "zstd",
                    message: format!("encode failed: {e}"),
                }
            })?;

        let bytes_out = compressed.len() as u64;
        output.write_all(&compressed)?;

        Ok(CodecReport {
            bytes_in: uncompressed_len as u64,
            bytes_out,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut compressed = Vec::new();
        input.read_to_end(&mut compressed)?;
        let bytes_in = compressed.len();

        let decompressed =
            zstd::decode_all(Cursor::new(&compressed)).map_err(|e| ArcError::Codec {
                codec: "zstd",
                message: format!("decode failed: {e}"),
            })?;

        let bytes_out = decompressed.len() as u64;
        output.write_all(&decompressed)?;

        Ok(CodecReport {
            bytes_in: bytes_in as u64,
            bytes_out,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage::default()
    }

    fn capabilities(&self) -> crate::codec::traits::CodecCapabilities {
        let lvl = self.options.level;
        let (comp, ratio, mem) = if lvl <= 3 {
            (400, 28u8, 32u32)
        } else if lvl <= 9 {
            (80, 24, 64)
        } else {
            (15, 20, 128)
        };
        crate::codec::traits::CodecCapabilities {
            compress_speed_mb_per_sec: comp,
            decompress_speed_mb_per_sec: 1_000,
            typical_ratio_pct: ratio,
            min_useful_bytes: 256,
            peak_memory_mib: mem,
            parallelizable: true,
        }
    }
}
