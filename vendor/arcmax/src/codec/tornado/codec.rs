use std::io::{Read, Write};

use crate::codec::tornado::decode;
use crate::codec::tornado::encode;
use crate::codec::tornado::options::TornadoOptions;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::Result;

/// Native Rust Tornado codec.
///
/// All three parsers (Greedy, Lazy, Optimal) and all five entropy coders
/// (Store, Byte, Bit, Huffman, Arithmetic) are implemented natively.
/// Decompression handles the full Tornado wire format including REPCHAR,
/// repdist codes, and numeric-table preprocessing commands.
pub struct TornadoCodec {
    options: TornadoOptions,
}

impl TornadoCodec {
    pub fn new(options: TornadoOptions) -> Self {
        Self { options }
    }
}

impl Codec for TornadoCodec {
    fn name(&self) -> &'static str {
        "tornado"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let bytes_out = encode::compress(input, output, &self.options)?;
        Ok(CodecReport {
            bytes_in: 0,
            bytes_out,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut input_ref = input;
        let mut output_ref = output;
        decode::decompress(&mut input_ref, &mut output_ref)?;
        Ok(CodecReport {
            bytes_in: 0,
            bytes_out: 0,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage {
            working_bytes: self.options.buffer_size as u64 * 2,
            ..MemoryUsage::default()
        }
    }
}

#[cfg(test)]
mod roundtrip_tests {
    use crate::codec::tornado::{decode, encode, TornadoOptions};
    use std::io::Cursor;

    fn roundtrip(input: &[u8], opts: &TornadoOptions) {
        let level = opts.level.unwrap_or(0);
        let mut encoded = Vec::new();
        encode::compress(&mut Cursor::new(input), &mut encoded, opts)
            .unwrap_or_else(|e| panic!("encode failed at level {level}: {e}"));
        let mut decoded = Vec::new();
        decode::decompress(&mut Cursor::new(&encoded), &mut decoded)
            .unwrap_or_else(|e| panic!("decode failed at level {level}: {e}"));
        assert_eq!(decoded, input, "roundtrip mismatch at level {level}");
    }

    fn corpus() -> Vec<u8> {
        let mut data = Vec::new();
        for i in 0..512u32 {
            data.extend_from_slice(b"tornado roundtrip verification block ");
            data.extend_from_slice(&(i % 17).to_le_bytes());
            data.extend_from_slice(b" abcdefghijklmnopqrstuvwxyz abcdefghijklmnopqrstuvwxyz ");
            if i % 5 == 0 {
                data.extend_from_slice(b"repeat-distance-repeat-distance-repeat-distance");
            }
        }
        data
    }

    #[test]
    fn all_levels_roundtrip() {
        let input = corpus();
        // Levels 1-12 are safe with the hash_size-in-bytes fix applied to
        // CachingMatchFinder and CycledCachingMatchFinder.  Levels 13-16 use
        // very large windows (512 MB+) and are left to the benchmark suite.
        for level in 1u8..=12 {
            let opts = TornadoOptions::preset(level).unwrap();
            roundtrip(&input, &opts);
        }
    }

    #[test]
    fn repchar_corpus_roundtrip() {
        // Data with many runs where the same byte repeats at a recent distance,
        // exercising the REPCHAR path in the Huffman and arithmetic encoders.
        let mut data = Vec::new();
        for _ in 0..64 {
            data.extend_from_slice(b"aaaaaaaaaaaaaaaaaaaaaa");
            data.extend_from_slice(b"bbbbbbbbbbbbbbbbbbbbbb");
            data.extend_from_slice(b"abababababababababab");
            data.extend_from_slice(b"xyzxyzxyzxyzxyzxyz");
        }
        for level in [3u8, 4, 5] {
            let opts = TornadoOptions::preset(level).unwrap();
            roundtrip(&data, &opts);
        }
    }
}

#[cfg(all(test, feature = "legacy-ffi-tests"))]
mod cross_language_tests {
    use crate::codec::tornado::{decode, encode, TornadoOptions};
    use std::io::Cursor;

    fn tornado_cross_corpus() -> Vec<u8> {
        let mut data = Vec::new();
        for i in 0..512u32 {
            data.extend_from_slice(b"tornado cross language verification block ");
            data.extend_from_slice(&(i % 17).to_le_bytes());
            data.extend_from_slice(b" abcdefghijklmnopqrstuvwxyz abcdefghijklmnopqrstuvwxyz ");
            if i % 5 == 0 {
                data.extend_from_slice(b"repeat-distance-repeat-distance-repeat-distance");
            }
        }
        data
    }

    fn rust_encode(input: &[u8], opts: &TornadoOptions) -> Vec<u8> {
        let mut encoded = Vec::new();
        encode::compress(&mut Cursor::new(input), &mut encoded, opts).unwrap();
        encoded
    }

    fn rust_decode(encoded: &[u8], level: u8) -> Vec<u8> {
        let mut decoded = Vec::new();
        decode::decompress(&mut Cursor::new(encoded), &mut decoded)
            .unwrap_or_else(|err| panic!("Rust decoder rejected C-encoded tornado:{level}: {err}"));
        decoded
    }

    fn cross_roundtrip_levels(levels: &[u8]) {
        let input = tornado_cross_corpus();
        for &level in levels {
            let opts = TornadoOptions::preset(level).unwrap();

            let rust_encoded = rust_encode(&input, &opts);
            let c_decoded = crate::codecs::tornado::tornado_decompress(&rust_encoded, input.len())
                .unwrap_or_else(|err| {
                    panic!("C decoder rejected Rust-encoded tornado:{level}: {err}")
                });
            assert_eq!(
                c_decoded, input,
                "C decoder failed Rust-encoded tornado:{level}"
            );

            let c_encoded = crate::codecs::tornado::tornado_compress(&input, level as i32)
                .unwrap_or_else(|err| panic!("C encoder failed tornado:{level}: {err}"));
            let rust_decoded = rust_decode(&c_encoded, level);
            assert_eq!(
                rust_decoded, input,
                "Rust decoder failed C-encoded tornado:{level}"
            );
        }
    }

    // Levels 1–5: Greedy parser, Byte/Bit/Huffman/Arithmetic entropy coders.
    #[test]
    fn cross_roundtrip_greedy_levels() {
        cross_roundtrip_levels(&[1, 2, 3, 4, 5]);
    }

    // Levels 6–10: Lazy parser, Arithmetic coder, larger hash tables.
    // Exercises Lazy-parser paths in both the Rust encoder and the C encoder.
    #[test]
    fn cross_roundtrip_lazy_levels() {
        cross_roundtrip_levels(&[6, 7, 8, 9, 10]);
    }

    // Levels 11–16: Optimal parser (11+), BinaryTree match finder (16).
    // Exercises the full optimal-parser emit path and REPCHAR/repdist codes
    // produced by the Arithmetic encoder at these levels.
    #[test]
    fn cross_roundtrip_optimal_levels() {
        cross_roundtrip_levels(&[11, 12, 13, 14, 15, 16]);
    }
}
