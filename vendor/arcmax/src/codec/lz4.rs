use std::io::{Read, Write};

use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lz4Options {
    pub mode: Lz4Mode,
}

impl Default for Lz4Options {
    fn default() -> Self {
        Self {
            mode: Lz4Mode::SizePrependedBlock,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lz4Mode {
    SizePrependedBlock,
}

#[derive(Debug, Clone)]
pub struct Lz4Codec {
    options: Lz4Options,
}

impl Lz4Codec {
    pub fn new(options: Lz4Options) -> Self {
        Self { options }
    }
}

impl Default for Lz4Codec {
    fn default() -> Self {
        Self::new(Lz4Options::default())
    }
}

impl Codec for Lz4Codec {
    fn name(&self) -> &'static str {
        "lz4"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        input.read_to_end(&mut source)?;

        let encoded = match self.options.mode {
            Lz4Mode::SizePrependedBlock => lz4_flex::compress_prepend_size(&source),
        };
        output.write_all(&encoded)?;

        Ok(CodecReport {
            bytes_in: source.len() as u64,
            bytes_out: encoded.len() as u64,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        input.read_to_end(&mut source)?;

        let decoded = match self.options.mode {
            Lz4Mode::SizePrependedBlock => {
                lz4_flex::decompress_size_prepended(&source).map_err(|err| ArcError::Codec {
                    codec: "lz4",
                    message: err.to_string(),
                })?
            }
        };
        output.write_all(&decoded)?;

        Ok(CodecReport {
            bytes_in: source.len() as u64,
            bytes_out: decoded.len() as u64,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage {
            working_bytes: 64 * 1024,
            ..MemoryUsage::default()
        }
    }

    fn capabilities(&self) -> crate::codec::traits::CodecCapabilities {
        crate::codec::traits::CodecCapabilities {
            compress_speed_mb_per_sec: 500,
            decompress_speed_mb_per_sec: 2_000,
            typical_ratio_pct: 45,
            min_useful_bytes: 64,
            peak_memory_mib: 4,
            parallelizable: false,
        }
    }
}
