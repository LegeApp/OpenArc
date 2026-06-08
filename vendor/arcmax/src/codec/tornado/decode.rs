use std::io::Read;
use std::io::Write;

use crate::codec::tornado::arith::Lz77ArithDecoder;
use crate::codec::tornado::format::{EncodingMethod, TornadoHeader};
use crate::codec::tornado::huffman::{BitDecoder, Lz77HuffDecoder};
use crate::codec::tornado::lz77::LzWindow;
use crate::codec::tornado::options::TornadoOptions;
use crate::codec::tornado::table::undiff_table;
use crate::error::{ArcError, Result};

// Matches C `IMPOSSIBLE_LEN` / `IMPOSSIBLE_DIST` = INT_MAX / 2.
pub(super) const IMPOSSIBLE_LEN: u32 = (i32::MAX / 2) as u32;
pub(super) const IMPOSSIBLE_DIST: u32 = (i32::MAX / 2) as u32;

/// Entry point for Tornado decompression.
///
/// Reads the 6-byte [`TornadoHeader`] from `input`, validates the encoding
/// method, then dispatches to the appropriate decoder. Returns
/// [`ArcError::UnsupportedCodec`] for any encoding method that does not yet
/// have a native Rust decoder.
pub fn decompress<R: Read, W: Write>(input: &mut R, output: &mut W) -> Result<()> {
    let header = TornadoHeader::read_from(input)?;
    match header.encoding {
        EncodingMethod::Storing | EncodingMethod::ByteCoder => {
            decompress_bytecoder(input, output, &header)
        }
        EncodingMethod::BitCoder => decompress_bitcoder(input, output, &header),
        EncodingMethod::HuffmanCoder => decompress_huffcoder(input, output, &header),
        EncodingMethod::ArithmeticCoder => decompress_aricoder(input, output, &header),
    }
}

/// Decompress a `BYTECODER` (or `STORING`) stream.
///
/// Both encoding methods write the same flag-word / literal / match byte
/// stream via `LZ77_ByteCoder`; only the header byte differs. The stream
/// ends with an EOF sentinel match (`IMPOSSIBLE_LEN`, `IMPOSSIBLE_DIST`).
fn decompress_bytecoder<R: Read, W: Write>(
    input: &mut R,
    output: &mut W,
    header: &TornadoHeader,
) -> Result<()> {
    let mut dec = ByteDecoder::new(input);
    let mut window = LzWindow::new(header.buf_size as usize);
    let minlen = header.min_match_len as u32;

    loop {
        let kind = dec.advance()?;

        if kind == 0 {
            let byte = dec.read_u8()?;
            window.push_literal(byte, output)?;
        } else {
            let (len, dist) = dec.decode_match(minlen, kind)?;
            if len == IMPOSSIBLE_LEN && dist == IMPOSSIBLE_DIST {
                break;
            }
            window.copy_match(dist as usize, len as usize, output)?;
        }
    }

    Ok(())
}

/// Low-level byte-stream decoder for the `LZ77_ByteDecoder` wire format.
///
/// Mirrors the C++ `LZ77_ByteDecoder` struct. A 32-bit flag word groups
/// 16 symbol kinds (2 bits each). `flagpos` counts down from 16; when it
/// reaches 0 a new flag word is read.  `flagpos` initialises to 1 so the
/// very first `advance()` call loads the first flag word immediately.
struct ByteDecoder<R: Read> {
    reader: R,
    flags: u32,
    flagpos: u32,
    /// Distance set by `decode_match`; read back by the caller.
    dist: u32,
}

impl<R: Read> ByteDecoder<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            flags: 0,
            flagpos: 1,
            dist: 0,
        }
    }

    // ---- primitive readers ------------------------------------------------

    fn read_u8(&mut self) -> Result<u8> {
        let mut b = [0u8; 1];
        self.reader.read_exact(&mut b).map_err(byte_io_err)?;
        Ok(b[0])
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        let mut b = [0u8; 2];
        self.reader.read_exact(&mut b).map_err(byte_io_err)?;
        Ok(u16::from_le_bytes(b))
    }

    fn read_u24_le(&mut self) -> Result<u32> {
        let mut b = [0u8; 3];
        self.reader.read_exact(&mut b).map_err(byte_io_err)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], 0]))
    }

    fn read_u32_le(&mut self) -> Result<u32> {
        let mut b = [0u8; 4];
        self.reader.read_exact(&mut b).map_err(byte_io_err)?;
        Ok(u32::from_le_bytes(b))
    }

    // ---- flag-word management --------------------------------------------

    /// Advance to the next symbol; returns its kind (0=literal, 1..3=match).
    fn advance(&mut self) -> Result<u32> {
        self.flagpos -= 1;
        if self.flagpos != 0 {
            self.flags >>= 2;
        } else {
            self.flagpos = 16;
            self.flags = self.read_u32_le()?;
        }
        Ok(self.flags & 3)
    }

    // ---- match decoding --------------------------------------------------

    /// Decode match length and distance from the byte stream.
    ///
    /// `kind` must be 1 (short), 2 (medium), or 3 (long).
    /// Returns `(len, dist)` where `len = minlen + raw_len`.
    fn decode_match(&mut self, minlen: u32, kind: u32) -> Result<(u32, u32)> {
        let raw_len = match kind {
            1 => {
                // 16-bit: upper 4 bits = len-MINLEN, lower 12 = dist
                let x = self.read_u16_le()? as u32;
                self.dist = x & 0xFFF;
                x >> 12
            }
            2 => {
                // 24-bit: upper 6 bits = len-MINLEN, lower 18 = dist
                let x = self.read_u24_le()?;
                self.dist = x & 0x3_FFFF;
                x >> 18
            }
            3 => {
                // Variable: optional 1-byte dist-high prefix (255), optional
                // 3+1 byte len prefix (254), then u32 packed as [len_low, dist[0..2]].
                let mut first = self.read_u8()? as u32;
                if first == 255 {
                    self.dist = (self.read_u8()? as u32) << 24;
                    first = self.read_u8()? as u32;
                } else {
                    self.dist = 0;
                }
                let raw = if first == 254 {
                    let hi = self.read_u24_le()? << 8;
                    hi + self.read_u8()? as u32
                } else {
                    first
                };
                self.dist += self.read_u24_le()?;
                raw
            }
            _ => unreachable!(),
        };
        Ok((minlen.wrapping_add(raw_len), self.dist))
    }
}

// ── BitCoder decoder ─────────────────────────────────────────────────────────
//
// Wire format: 9-bit symbols packed LSB-first into LE u32 words.
// x < 256  → literal byte x
// x >= 256 → match: bits[7:5] = lcode, bits[4:0] = dcode; then lbits + dbits
//             extra bits for exact length and distance.
//
// Tables derived from LZ77_Coder.cpp constants:
//   extra_lbits[8] = {0,0,0,1,2,4,8,30}
//   extra_dbits[32] = {4,4,5,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13,14,14,15,15,16,17,18,19,21,23,30}

// Length VLE: base value and extra-bit count for each of the 8 length codes.
const LC_EXTRA: [u32; 8] = [0, 0, 0, 1, 2, 4, 8, 30];
const LC_BASE: [u32; 8] = [0, 1, 2, 3, 5, 9, 25, 281];

// Distance VLE: base value (actual byte offset) and extra-bit count for each
// of the 32 distance codes.  Precomputed from the 3-phase DistanceCoder init.
const DC_EXTRA: [u32; 32] = [
    4, 4, 5, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16, 17,
    18, 19, 21, 23, 30,
];
const DC_BASE: [u32; 32] = [
    0, 16, 32, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 2048, 3072, 4096, 6144, 8192,
    12288, 16384, 24576, 32768, 49152, 65536, 98304, 131_072, 196_608, 327_680, 589_824, 1_114_112,
    3_211_264, 11_599_872,
];

fn decompress_bitcoder<R: Read, W: Write>(
    input: &mut R,
    output: &mut W,
    header: &TornadoHeader,
) -> Result<()> {
    let mut dec = BitDecoder::new(input);
    let mut window = LzWindow::new(header.buf_size as usize);
    let minlen = header.min_match_len as u32;

    loop {
        let x = dec.getbits(9)?;
        if x < 256 {
            window.push_literal(x as u8, output)?;
        } else {
            let lcode = (x >> 5) - 8;
            let dcode = (x & 31) as usize;
            let lbits = LC_EXTRA[lcode as usize];
            let lbase = LC_BASE[lcode as usize];
            let dbits = DC_EXTRA[dcode];
            let dbase = DC_BASE[dcode];
            let len = minlen + lbase + dec.getbits(lbits)?;
            let dist = dbase + dec.getbits(dbits)?;
            if len == IMPOSSIBLE_LEN && dist == IMPOSSIBLE_DIST {
                break;
            }
            window.copy_match(dist as usize, len as usize, output)?;
        }
    }

    Ok(())
}

fn decompress_huffcoder<R: Read, W: Write>(
    input: &mut R,
    output: &mut W,
    header: &TornadoHeader,
) -> Result<()> {
    let mut dec = Lz77HuffDecoder::new(input);
    let mut window = LzWindow::new(header.buf_size as usize);
    let minlen = header.min_match_len as u32;

    // Collect decompressed bytes and table regions for undiff post-processing.
    let mut buf: Vec<u8> = Vec::new();
    let mut tables: Vec<(usize, usize, usize)> = Vec::new(); // (start, type_n, items)

    loop {
        if dec.is_literal()? {
            window.push_literal(dec.getchar(), &mut buf)?;
        } else {
            let len = dec.getlen(minlen)?;
            let dist = dec.getdist()?;
            if len == IMPOSSIBLE_LEN && dist == IMPOSSIBLE_DIST {
                break;
            }
            if len > IMPOSSIBLE_LEN {
                // Table command: type_n = len - IMPOSSIBLE_LEN, items = dist.
                let type_n = (len - IMPOSSIBLE_LEN) as usize;
                tables.push((buf.len(), type_n, dist as usize));
            } else {
                window.copy_match(dist as usize, len as usize, &mut buf)?;
            }
        }
    }

    for (start, n, items) in &tables {
        let end = start + n * items;
        if end <= buf.len() {
            undiff_table(&mut buf, *start, *n, *items);
        }
    }

    output.write_all(&buf)?;
    Ok(())
}

fn decompress_aricoder<R: Read, W: Write>(
    input: &mut R,
    output: &mut W,
    header: &TornadoHeader,
) -> Result<()> {
    let mut dec = Lz77ArithDecoder::new(input)?;
    let mut window = LzWindow::new(header.buf_size as usize);
    let minlen = header.min_match_len as u32;

    let mut buf: Vec<u8> = Vec::new();
    let mut tables: Vec<(usize, usize, usize)> = Vec::new(); // (start, type_n, items)

    loop {
        if dec.is_literal()? {
            window.push_literal(dec.getchar(), &mut buf)?;
        } else {
            let len = dec.getlen(minlen)?;
            let dist = dec.getdist()?;
            if len == IMPOSSIBLE_LEN && dist == IMPOSSIBLE_DIST {
                break;
            }
            if len > IMPOSSIBLE_LEN {
                let type_n = (len - IMPOSSIBLE_LEN) as usize;
                tables.push((buf.len(), type_n, dist as usize));
            } else {
                window.copy_match(dist as usize, len as usize, &mut buf)?;
            }
        }
    }

    for (start, n, items) in &tables {
        let end = start + n * items;
        if end <= buf.len() {
            undiff_table(&mut buf, *start, *n, *items);
        }
    }

    output.write_all(&buf)?;
    Ok(())
}

#[inline]
fn byte_io_err(e: std::io::Error) -> ArcError {
    ArcError::Codec {
        codec: "tornado",
        message: e.to_string(),
    }
}

/// A decoder instance built from parsed header metadata.
///
/// Holds the window and any decoder-specific state. Produced by
/// [`TornadoDecoder::from_header`] once the header has been parsed.
pub struct TornadoDecoder {
    pub header: TornadoHeader,
    window: LzWindow,
    #[allow(dead_code)]
    options: TornadoOptions,
}

impl TornadoDecoder {
    /// Build a decoder from an already-parsed header and the original method options.
    pub fn from_header(header: TornadoHeader, options: TornadoOptions) -> Self {
        let window = LzWindow::new(header.buf_size as usize);
        Self {
            header,
            window,
            options,
        }
    }

    /// The buffer/dictionary size negotiated at compression time.
    pub fn buf_size(&self) -> u32 {
        self.header.buf_size
    }

    /// The minimum match length negotiated at compression time.
    pub fn min_match_len(&self) -> u8 {
        self.header.min_match_len
    }

    /// Bytes written to the output so far.
    pub fn produced(&self) -> u64 {
        self.window.produced()
    }
}

// ---- helpers for tests -------------------------------------------------------

/// Build a minimal ByteCoder/Storing stream that decodes to `data`.
///
/// Encodes every byte as a literal, then appends the IMPOSSIBLE_LEN/DIST
/// EOF sentinel. Used only in unit tests.
#[cfg(test)]
fn build_bytecoder_literals(data: &[u8], minlen: u8) -> Vec<u8> {
    // Flag word groups 16 symbols; literal kind = 0, long match kind = 3.
    // Symbol N (1-based) uses flagbit = 1 << (2*(N-1 % 16)).
    // We need len(data)+1 symbols: N literals + 1 EOF sentinel.

    let total_syms = data.len() + 1; // +1 for EOF
    let mut flag_words: Vec<u32> = vec![0u32; (total_syms + 15) / 16];

    // Mark the EOF sentinel symbol as kind=3.
    let eof_sym_idx = data.len(); // 0-based
    let word_idx = eof_sym_idx / 16;
    let bit_pos = (eof_sym_idx % 16) * 2;
    flag_words[word_idx] |= 3u32 << bit_pos;

    // Build the payload bytes
    let mut out = Vec::new();
    let mut sym_in_word = 0usize;
    let mut word_idx_cur = 0usize;

    for &b in data {
        let _ = (sym_in_word, word_idx_cur); // unused here; flag encoding above
        out.push(b);
        sym_in_word += 1;
        if sym_in_word == 16 {
            sym_in_word = 0;
            word_idx_cur += 1;
        }
    }

    // Build the EOF sentinel bytes (kind=3 long match for IMPOSSIBLE values)
    let imp_len = IMPOSSIBLE_LEN;
    let imp_dist = IMPOSSIBLE_DIST;
    let mut len_reduced = imp_len - minlen as u32;
    let mut eof_bytes = Vec::new();

    // dist >= 2^24 → write 255 + high byte
    eof_bytes.push(255u8);
    eof_bytes.push((imp_dist >> 24) as u8);

    // len >= 254 → write 254 + put24(len >> 8) + remaining in final put32
    eof_bytes.push(254u8);
    let len_hi = len_reduced >> 8;
    eof_bytes.extend_from_slice(&(len_hi as u32).to_le_bytes()[..3]); // put24
    len_reduced &= 0xFF;

    // put32(len_low + (dist_low24 << 8)) — only low 24 bits of dist used here
    let packed = len_reduced | ((imp_dist & 0xFF_FFFF) << 8);
    eof_bytes.extend_from_slice(&packed.to_le_bytes());

    // Now interleave flag words and payload. Each flag word precedes its 16 symbols.
    let mut stream = Vec::new();
    for (wi, &fw) in flag_words.iter().enumerate() {
        stream.extend_from_slice(&fw.to_le_bytes());
        let start = wi * 16;
        let end = ((wi + 1) * 16).min(data.len());
        stream.extend_from_slice(&out[start..end]);
        // If this word contains the EOF sentinel symbol, append EOF bytes
        let eof_word = eof_sym_idx / 16;
        if wi == eof_word {
            stream.extend_from_slice(&eof_bytes);
        }
    }
    stream
}
