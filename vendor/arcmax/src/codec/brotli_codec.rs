//! Brotli codec — pure-Rust `brotli` crate.
//!
//! Brotli frames are self-describing (no external size header needed). We use
//! the stream API and let the encoder/decoder negotiate the boundary.

use std::io::{Read, Write};

use crate::codec::brotli::BrotliOptions;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

pub struct BrotliCodec {
    options: BrotliOptions,
}

impl BrotliCodec {
    pub fn new(options: BrotliOptions) -> Self {
        Self { options }
    }
}

impl Codec for BrotliCodec {
    fn name(&self) -> &'static str {
        "brotli"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        let bytes_in = input.read_to_end(&mut source)? as u64;

        let mut encoded = Vec::with_capacity(source.len() / 4 + 16);
        let mut writer = brotli::CompressorWriter::new(
            &mut encoded,
            4096, // internal buffer size
            self.options.quality,
            self.options.lgwin,
        );
        writer.write_all(&source).map_err(|e| ArcError::Codec {
            codec: "brotli",
            message: format!("encode failed: {e}"),
        })?;
        drop(writer);

        let bytes_out = encoded.len() as u64;
        output.write_all(&encoded)?;
        Ok(CodecReport {
            bytes_in,
            bytes_out,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut compressed = Vec::new();
        let bytes_in = input.read_to_end(&mut compressed)? as u64;

        let mut decoded = Vec::new();
        let mut reader = brotli::Decompressor::new(&compressed[..], 4096);
        reader
            .read_to_end(&mut decoded)
            .map_err(|e| ArcError::Codec {
                codec: "brotli",
                message: format!("decode failed: {e}"),
            })?;

        let bytes_out = decoded.len() as u64;
        output.write_all(&decoded)?;
        Ok(CodecReport {
            bytes_in,
            bytes_out,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage::default()
    }
}
