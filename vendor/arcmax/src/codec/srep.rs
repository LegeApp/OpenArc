use std::io::{Cursor, Read, Write};

use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::Result;
use crate::srep::SrepConfig;

#[derive(Debug, Clone)]
pub struct SrepCodec {
    options: SrepConfig,
}

impl SrepCodec {
    pub fn new(options: SrepConfig) -> Self {
        Self { options }
    }

    pub fn options(&self) -> &SrepConfig {
        &self.options
    }
}

impl Codec for SrepCodec {
    fn name(&self) -> &'static str {
        "srep"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut input_data = Vec::new();
        input.read_to_end(&mut input_data)?;

        let mut compressed = Cursor::new(Vec::new());
        let report = crate::srep::compress(
            Cursor::new(input_data),
            &mut compressed,
            self.options.clone(),
        )?;
        let compressed = compressed.into_inner();
        output.write_all(&compressed)?;

        Ok(CodecReport {
            bytes_in: report.bytes_in,
            bytes_out: compressed.len() as u64,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut input_data = Vec::new();
        input.read_to_end(&mut input_data)?;

        let mut decoded = Cursor::new(Vec::new());
        let report = crate::srep::decompress(Cursor::new(input_data), &mut decoded)?;
        let decoded = decoded.into_inner();
        output.write_all(&decoded)?;

        Ok(CodecReport {
            bytes_in: 0,
            bytes_out: report.bytes_out,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage {
            working_bytes: (self.options.block_size + self.options.dict_size) as u64,
            ..MemoryUsage::default()
        }
    }
}
