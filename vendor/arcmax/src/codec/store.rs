use std::io::{Read, Write};

use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::Result;

#[derive(Debug, Clone, Copy, Default)]
pub struct StoreCodec;

impl Codec for StoreCodec {
    fn name(&self) -> &'static str {
        "storing"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        copy_passthrough(input, output)
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        copy_passthrough(input, output)
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage::default()
    }

    fn capabilities(&self) -> crate::codec::traits::CodecCapabilities {
        crate::codec::traits::CodecCapabilities {
            compress_speed_mb_per_sec: 10_000,
            decompress_speed_mb_per_sec: 10_000,
            typical_ratio_pct: 100,
            min_useful_bytes: 0,
            peak_memory_mib: 1,
            parallelizable: true,
        }
    }
}

fn copy_passthrough(input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
    let bytes = std::io::copy(input, output)?;
    Ok(CodecReport {
        bytes_in: bytes,
        bytes_out: bytes,
    })
}
