use std::io::{Read, Write};

use crate::codec::rep::decode::rep_decompress;
use crate::codec::rep::encode::rep_compress;
use crate::codec::rep::options::RepOptions;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::Result;

pub struct RepCodec {
    opts: RepOptions,
}

impl RepCodec {
    pub fn new(opts: RepOptions) -> Self {
        Self { opts }
    }
}

impl Codec for RepCodec {
    fn name(&self) -> &'static str {
        "rep"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let bytes_out = rep_compress(input, output, &self.opts)?;
        Ok(CodecReport {
            bytes_in: 0,
            bytes_out,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let bytes_out = rep_decompress(input, output)?;
        Ok(CodecReport {
            bytes_in: 0,
            bytes_out,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage {
            read_bytes: 0,
            write_bytes: 0,
            working_bytes: (self.opts.block_size as u64).saturating_mul(2),
        }
    }
}
