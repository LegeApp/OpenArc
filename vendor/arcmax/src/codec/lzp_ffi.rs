use std::io::{Read, Write};

use crate::codec::framing;
use crate::codec::lzp::LzpOptions;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

/// Transitional FFI-backed LZP codec.
///
/// Wraps `freearc_lzp_compress` / `freearc_lzp_decompress` behind the
/// `Codec` trait with size-prepend framing.
pub struct LzpFfiCodec {
    options: LzpOptions,
}

impl LzpFfiCodec {
    pub fn new(options: LzpOptions) -> Self {
        Self { options }
    }
}

impl Codec for LzpFfiCodec {
    fn name(&self) -> &'static str {
        "lzp-ffi"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        input.read_to_end(&mut source)?;
        let uncompressed_len = source.len();

        let compressed = crate::codecs::lzp::lzp_compress(
            &source,
            self.options.min_match as i32,
            self.options.hash_size_log as i32,
        )
        .map_err(|e| ArcError::Codec {
            codec: "lzp",
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

        // The FFI lzp_decompress convenience function uses fixed defaults.
        // Pass options explicitly via the raw FFI function used by that module.
        let decompressed = lzp_decompress_with_opts(
            &compressed,
            expected_len,
            self.options.min_match as i32,
            self.options.hash_size_log as i32,
        )
        .map_err(|e| ArcError::Codec {
            codec: "lzp",
            message: e.to_string(),
        })?;

        output.write_all(&decompressed)?;

        Ok(CodecReport {
            bytes_in,
            bytes_out: decompressed.len() as u64,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        let hash_bytes = 1usize << self.options.hash_size_log;
        MemoryUsage {
            working_bytes: hash_bytes as u64,
            ..MemoryUsage::default()
        }
    }
}

// Call the underlying FFI function with explicit options rather than the
// convenience wrapper that hardcodes defaults.
fn lzp_decompress_with_opts(
    input: &[u8],
    expected_size: usize,
    min_match_len: i32,
    hash_size_log: i32,
) -> anyhow::Result<Vec<u8>> {
    use anyhow::anyhow;

    extern "C" {
        fn freearc_lzp_decompress(
            input: *const u8,
            input_size: i32,
            output: *mut u8,
            output_size: i32,
            min_match_len: i32,
            hash_size: i32,
        ) -> i32;
    }

    let mut output = vec![0u8; expected_size];
    let result = unsafe {
        freearc_lzp_decompress(
            input.as_ptr(),
            input.len() as i32,
            output.as_mut_ptr(),
            expected_size as i32,
            min_match_len,
            hash_size_log,
        )
    };

    if result < 0 {
        return Err(anyhow!("LZP decompression error code: {result}"));
    }
    let actual = result as usize;
    if actual > output.len() {
        return Err(anyhow!("LZP returned {actual} > expected {expected_size}"));
    }
    output.truncate(actual);
    Ok(output)
}

#[cfg(all(test, feature = "legacy-ffi-tests"))]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn lzp_ffi_codec_roundtrip() {
        let input = b"LZP FFI codec roundtrip payload ".repeat(256);
        let mut codec = LzpFfiCodec::new(LzpOptions::default());

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
