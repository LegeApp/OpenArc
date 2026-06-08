use std::io::{Read, Write};

use crate::codec::dict::decode::dict_decompress;
use crate::codec::dict::encode::dict_compress;
use crate::codec::dict::options::DictOptions;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::Result;

pub struct DictCodec {
    opts: DictOptions,
}

impl DictCodec {
    pub fn new(opts: DictOptions) -> Self {
        Self { opts }
    }
}

impl Codec for DictCodec {
    fn name(&self) -> &'static str {
        "dict"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let bytes_out = dict_compress(input, output, &self.opts)?;
        Ok(CodecReport {
            bytes_in: 0,
            bytes_out,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let bytes_out = dict_decompress(input, output)?;
        Ok(CodecReport {
            bytes_in: 0,
            bytes_out,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage {
            read_bytes: 0,
            write_bytes: 0,
            working_bytes: self.opts.block_size as u64,
        }
    }
}
