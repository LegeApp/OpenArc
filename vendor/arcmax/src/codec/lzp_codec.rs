use std::io::{Read, Write};

use crate::codec::framing;
use crate::codec::lzp::LzpOptions;
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

const MATCH_FLAG: u8 = 0xF2;
const RUN_FLAG: u8 = 0xF3;
const XOR_FLAG: u8 = 0xFF ^ RUN_FLAG; // 0x0C

/// Native LZP context-based literal suppressor.
///
/// LZP is a prefilter that replaces repeated byte sequences found via a
/// context-hash table with short match tokens. Compatible with the FFI LZP
/// codec; uses the same size-prepend framing as `LzpFfiCodec`.
///
/// When the encoder's output would equal or exceed the input length, it returns
/// `ArcError::Codec` with `message == "not compressible"`. Callers may detect
/// this and fall back to storing the raw bytes.
pub struct LzpCodec {
    options: LzpOptions,
}

impl LzpCodec {
    pub fn new(options: LzpOptions) -> Self {
        Self { options }
    }
}

impl Codec for LzpCodec {
    fn name(&self) -> &'static str {
        "lzp"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        input.read_to_end(&mut source)?;
        let uncompressed_len = source.len();

        let compressed = lzp_encode(&source, self.options.min_match, self.options.hash_size_log)?;

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

        let decompressed = lzp_decode(
            &compressed,
            expected_len,
            self.options.min_match,
            self.options.hash_size_log,
        )?;

        output.write_all(&decompressed)?;

        Ok(CodecReport {
            bytes_in,
            bytes_out: decompressed.len() as u64,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        let hash_bytes = (1usize << self.options.hash_size_log) * 4;
        MemoryUsage {
            working_bytes: hash_bytes as u64,
            ..MemoryUsage::default()
        }
    }
}

/// Encode `input` with LZP. Returns compressed bytes, or `ArcError::Codec`
/// with message "not compressible" if compressed length >= input length.
pub fn lzp_encode(input: &[u8], min_match: usize, hash_size_log: u8) -> Result<Vec<u8>> {
    if input.len() < 4 {
        return Ok(input.to_vec());
    }

    let mask = ((1u32 << hash_size_log) - 1) as usize;
    // Table stores absolute input positions (1-based: 0 == no entry).
    let mut table = vec![0u32; mask + 1];

    let mut out = Vec::with_capacity(input.len());
    // First 4 bytes verbatim.
    out.extend_from_slice(&input[..4]);

    let mut ctx = u32::from_be_bytes([input[0], input[1], input[2], input[3]]);
    let mut in_pos: usize = 4;

    while in_pos < input.len() {
        if out.len() >= input.len() {
            return Err(ArcError::Codec {
                codec: "lzp",
                message: "not compressible".into(),
            });
        }

        let hash = hash_ctx(ctx, mask);
        let hist = table[hash] as usize;
        table[hash] = in_pos as u32;

        if hist != 0 {
            // Count matching bytes from hist and in_pos.
            let match_len = count_match(input, in_pos, hist);

            if match_len >= min_match {
                // Encode match.
                in_pos += match_len;
                ctx = read_ctx(input, in_pos);

                let mut run = match_len - min_match + 1;
                out.push(MATCH_FLAG);
                loop {
                    if run > 254 {
                        out.push(RUN_FLAG);
                        if out.len() >= input.len() {
                            return Err(ArcError::Codec {
                                codec: "lzp",
                                message: "not compressible".into(),
                            });
                        }
                        run -= 255;
                    } else {
                        out.push((run as u8) ^ XOR_FLAG);
                        break;
                    }
                }
                continue;
            }

            // No match: literal, but escape MATCH_FLAG so decoder can tell.
            let ch = input[in_pos];
            in_pos += 1;
            ctx = (ctx << 8) | ch as u32;
            out.push(ch);
            if ch == MATCH_FLAG {
                out.push(XOR_FLAG);
            }
        } else {
            // No history entry: literal copied verbatim (no escape needed).
            let ch = input[in_pos];
            in_pos += 1;
            ctx = (ctx << 8) | ch as u32;
            out.push(ch);
        }
    }

    Ok(out)
}

/// Decode `input` (produced by `lzp_encode`) back to its original form.
pub fn lzp_decode(
    input: &[u8],
    expected_len: usize,
    min_match: usize,
    hash_size_log: u8,
) -> Result<Vec<u8>> {
    if input.len() < 4 {
        return Ok(input.to_vec());
    }

    let mask = ((1u32 << hash_size_log) - 1) as usize;
    // Table stores absolute output positions (0 == no entry).
    let mut table = vec![0u32; mask + 1];

    let mut out = Vec::with_capacity(expected_len);
    out.extend_from_slice(&input[..4]);

    let mut ctx = u32::from_be_bytes([input[0], input[1], input[2], input[3]]);
    let mut in_pos: usize = 4;

    while in_pos < input.len() {
        let hash = hash_ctx(ctx, mask);
        let hist = table[hash] as usize;
        table[hash] = out.len() as u32;

        if hist != 0 {
            // History exists: MATCH_FLAG is a match token; any other byte is literal.
            let byte = input[in_pos];
            in_pos += 1;

            if byte != MATCH_FLAG {
                ctx = (ctx << 8) | byte as u32;
                out.push(byte);
            } else {
                // Decode run-length.
                let mut count: usize = 0;
                loop {
                    if in_pos >= input.len() {
                        return Err(ArcError::Codec {
                            codec: "lzp",
                            message: "truncated match run-length".into(),
                        });
                    }
                    let b = input[in_pos];
                    in_pos += 1;
                    count += (b ^ XOR_FLAG) as usize;
                    if b != RUN_FLAG {
                        break;
                    }
                }
                if count == 0 {
                    // Escaped MATCH_FLAG literal.
                    ctx = (ctx << 8) | MATCH_FLAG as u32;
                    out.push(MATCH_FLAG);
                } else {
                    let copy_len = count + min_match - 1;
                    for i in 0..copy_len {
                        let src = hist + i;
                        if src >= out.len() {
                            return Err(ArcError::Codec {
                                codec: "lzp",
                                message: "match references future output".into(),
                            });
                        }
                        let b = out[src];
                        out.push(b);
                    }
                    ctx = read_ctx(&out, out.len());
                }
            }
        } else {
            // No history: byte is verbatim (no MATCH_FLAG ambiguity).
            let byte = input[in_pos];
            in_pos += 1;
            ctx = (ctx << 8) | byte as u32;
            out.push(byte);
        }
    }

    Ok(out)
}

#[inline]
fn hash_ctx(ctx: u32, mask: usize) -> usize {
    (((ctx >> 15) ^ ctx ^ (ctx >> 3)) as usize) & mask
}

#[inline]
fn count_match(input: &[u8], a: usize, b: usize) -> usize {
    input[a..]
        .iter()
        .zip(input[b..].iter())
        .take_while(|(x, y)| x == y)
        .count()
}

/// Read the context (big-endian u32 of the last 4 bytes) at `pos`.
#[inline]
fn read_ctx(buf: &[u8], pos: usize) -> u32 {
    if pos < 4 {
        return 0;
    }
    u32::from_be_bytes([buf[pos - 4], buf[pos - 3], buf[pos - 2], buf[pos - 1]])
}
