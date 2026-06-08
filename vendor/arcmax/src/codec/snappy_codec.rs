//! Snappy codec — pure-Rust `snap` crate.
//!
//! We use Snappy's **frame** format (`.sz`) so streams are self-describing
//! across block boundaries.  The raw block format requires the caller to
//! preserve the uncompressed size out-of-band, which doesn't fit the `Codec`
//! trait contract (compressed bytes must be self-sufficient).

use std::io::{Read, Write};

use crate::codec::snappy::SnappyOptions;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

pub struct SnappyCodec {
    _options: SnappyOptions,
}

impl SnappyCodec {
    pub fn new(options: SnappyOptions) -> Self {
        Self { _options: options }
    }
}

impl Codec for SnappyCodec {
    fn name(&self) -> &'static str {
        "snappy"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        let bytes_in = input.read_to_end(&mut source)? as u64;

        let mut encoded = Vec::with_capacity(source.len() / 2 + 32);
        let mut writer = snap::write::FrameEncoder::new(&mut encoded);
        writer.write_all(&source).map_err(|e| ArcError::Codec {
            codec: "snappy",
            message: format!("encode failed: {e}"),
        })?;
        writer.into_inner().map_err(|e| ArcError::Codec {
            codec: "snappy",
            message: format!("encode flush failed: {e}"),
        })?;

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
        let mut reader = snap::read::FrameDecoder::new(&compressed[..]);
        reader
            .read_to_end(&mut decoded)
            .map_err(|e| ArcError::Codec {
                codec: "snappy",
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
