use std::io::{Read, Write};

use crate::codec::framing;
use crate::codec::grzip::GrzipOptions;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

/// Transitional FFI-backed GRZip codec.
///
/// Wraps `freearc_grzip_compress` / `freearc_grzip_decompress` behind the
/// `Codec` trait with size-prepend framing.
pub struct GrzipFfiCodec {
    options: GrzipOptions,
}

impl GrzipFfiCodec {
    pub fn new(options: GrzipOptions) -> Self {
        Self { options }
    }
}

impl Codec for GrzipFfiCodec {
    fn name(&self) -> &'static str {
        "grzip-ffi"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        input.read_to_end(&mut source)?;
        let uncompressed_len = source.len();

        let compressed =
            crate::codecs::grzip::grzip_compress(&source, self.options.mode).map_err(|e| {
                ArcError::Codec {
                    codec: "grzip",
                    message: e.to_string(),
                }
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

        let decompressed = crate::codecs::grzip::grzip_decompress(&compressed, expected_len)
            .map_err(|e| ArcError::Codec {
                codec: "grzip",
                message: e.to_string(),
            })?;

        output.write_all(&decompressed)?;

        Ok(CodecReport {
            bytes_in,
            bytes_out: decompressed.len() as u64,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        // GRZip max block 8 MB; allocates roughly 5× for BWT transform.
        MemoryUsage {
            working_bytes: 8 * 1024 * 1024 * 5,
            ..MemoryUsage::default()
        }
    }
}

#[cfg(all(test, feature = "legacy-ffi-tests"))]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn grzip_ffi_codec_roundtrip() {
        let input = b"GRZip FFI codec roundtrip payload ".repeat(128);
        let mut codec = GrzipFfiCodec::new(GrzipOptions::default());

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
