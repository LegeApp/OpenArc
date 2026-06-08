use std::io::{Read, Write};

use crate::codec::framing;
use crate::codec::lzma::LzmaOptions;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

/// Transitional FFI-backed LZMA/LZMA2 codec.
///
/// Wraps `freearc_lzma2_compress` / `freearc_lzma2_decompress` behind the
/// `Codec` trait with size-prepend framing.
pub struct LzmaFfiCodec {
    options: LzmaOptions,
}

impl LzmaFfiCodec {
    pub fn new(options: LzmaOptions) -> Self {
        Self { options }
    }
}

impl Codec for LzmaFfiCodec {
    fn name(&self) -> &'static str {
        "lzma-ffi"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        input.read_to_end(&mut source)?;
        let uncompressed_len = source.len();

        let level = self.options.level.unwrap_or(5) as i32;
        let compressed = crate::codecs::lzma2::lzma2_compress(
            &source,
            level,
            self.options.dict_size,
            self.options.lc,
            self.options.lp,
            self.options.pb,
        )
        .map_err(|e| ArcError::Codec {
            codec: "lzma",
            message: e.to_string(),
        })?;

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

        let decompressed = crate::codecs::lzma2::lzma2_decompress(
            &compressed,
            expected_len,
            self.options.dict_size,
            self.options.lc,
            self.options.lp,
            self.options.pb,
        )
        .map_err(|e| ArcError::Codec {
            codec: "lzma",
            message: e.to_string(),
        })?;

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
}

#[cfg(all(test, feature = "legacy-ffi-tests"))]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn lzma_ffi_codec_roundtrip() {
        let input = b"LZMA FFI codec roundtrip payload ".repeat(256);
        let mut codec = LzmaFfiCodec::new(LzmaOptions::default());

        let mut encoded = Vec::new();
        codec
            .compress(&mut Cursor::new(&input), &mut encoded)
            .unwrap();

        let mut decoded = Vec::new();
        codec
            .decompress(&mut Cursor::new(&encoded), &mut decoded)
            .unwrap();
        assert_eq!(decoded, input);
    }
}
