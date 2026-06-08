use std::io::{Read, Write};

use crate::error::{ArcError, Result};

// ── Constants mirroring LZ77_Coder.cpp / EntropyCoder.cpp ─────────────────────

// elements(extra_lbits2) = 16 length codes
pub const LEN_CODES: usize = 16;
// elements(extra_dbits) + REPDIST_CODES = 32 + 4 = 36 distance codes
pub const DIST_CODES: usize = 36;
pub const REPDIST_CODES: usize = 4;
// Combined alphabet: 0..255 literals, 256..EOB_CODE-1 match codes, then specials
pub const EOB_CODE: usize = 256 + LEN_CODES * DIST_CODES;
pub const REPCHAR: usize = EOB_CODE + 1;
pub const CODES: usize = EOB_CODE + 10;

// Length VLE for lc2 (extra_lbits2 = {0,0,0,0,0,0,0,1,1,2,2,3,3,4,8,30})
pub const LC2_EXTRA: [u32; LEN_CODES] = [0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 8, 30];
pub const LC2_BASE: [u32; LEN_CODES] = [0, 1, 2, 3, 4, 5, 6, 7, 9, 11, 15, 19, 27, 35, 51, 307];

// Distance VLE for dc (same as BitCoder — reused from decode.rs constants)
pub const DC_EXTRA: [u32; 32] = [
    4, 4, 5, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16, 17,
    18, 19, 21, 23, 30,
];
pub const DC_BASE: [u32; 32] = [
    0, 16, 32, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 2048, 3072, 4096, 6144, 8192,
    12288, 16384, 24576, 32768, 49152, 65536, 98304, 131_072, 196_608, 327_680, 589_824, 1_114_112,
    3_211_264, 11_599_872,
];

// ── Huffman tree ───────────────────────────────────────────────────────────────

const MAXHUF: usize = 2048;
const FAST_BITS: u32 = 11;
const HUFBLOCK: usize = 5000;
const HUFBLOCK_INIT: usize = HUFBLOCK / 4;

/// A semi-adaptive Huffman tree (mirrors `HuffmanTree` in `EntropyCoder.cpp`).
///
/// Symbols start with frequency 1 and are rescaled periodically. The decoder
/// table (`fast_index` + `index`) is rebuilt after every `HUFBLOCK` symbols.
pub struct HuffmanTree {
    n: usize,
    pub counter: Vec<u32>,
    /// Bit depth for each symbol (used by both encoder and decoder).
    pub bits: Vec<u8>,
    /// Canonical code word for each symbol (encoder side; LSB = first bit sent).
    pub codes: Vec<u32>,
    pub maxbits: u32,
    maxbits_so_far: u32,
    /// Direct decode table for codes ≤ FAST_BITS bits (2048 entries).
    /// Negative → long code, must use `index[]`.
    fast_index: Vec<i32>,
    /// Full decode table for codes > FAST_BITS bits (1 << maxbits entries).
    index: Vec<u16>,
}

impl HuffmanTree {
    pub fn new(n: usize) -> Self {
        assert!(n <= MAXHUF, "HuffmanTree: n={n} exceeds MAXHUF={MAXHUF}");
        let mut tree = Self {
            n,
            counter: vec![1; n],
            bits: vec![0; n],
            codes: vec![0; n],
            maxbits: 0,
            maxbits_so_far: 0,
            fast_index: vec![0; 1 << FAST_BITS],
            index: Vec::new(),
        };
        tree.build_tree(0);
        tree
    }

    pub fn inc(&mut self, s: usize) {
        self.counter[s] += 1;
    }

    /// Decode one symbol given the `maxbits` low bits of the bit buffer.
    #[inline]
    pub fn decode(&self, code: u32) -> u32 {
        let fast = self.fast_index[(code & ((1 << FAST_BITS) - 1)) as usize];
        if fast >= 0 {
            fast as u32
        } else {
            self.index[code as usize] as u32
        }
    }

    /// Rebuild the decode table from current counters, then rescale counters.
    ///
    /// This mirrors `HuffmanTree::build_tree(rescale_mode)` in `EntropyCoder.cpp`.
    pub fn build_tree(&mut self, rescale_mode: u32) {
        let n = self.n;

        // ── Phase 1: sort all n symbols stably by counter (ascending) ──────────
        //
        // The C code uses a hybrid counting-sort for small counters plus
        // qsort for large ones. We use a stable sort — same output order,
        // slightly simpler.
        let mut sorted: Vec<u32> = (0..n as u32).collect();
        sorted.sort_by(|&a, &b| {
            let ca = self.counter[a as usize];
            let cb = self.counter[b as usize];
            ca.cmp(&cb).then(a.cmp(&b)) // stable: tie-break by symbol index
        });
        let b = n; // all symbols have counter ≥ 1

        // ── Phase 2: Huffman tree construction ─────────────────────────────────
        //
        // `buf` stores both original leaf nodes (indices 0..b-1) and combined
        // internal nodes (indices b+2..). `u32::MAX` fences mark end of each list.

        // We store (cnt, left, right, bits, code) per node.
        // Using parallel arrays to avoid the struct-of-arrays vs array-of-structs
        // friction with the C-style index arithmetic.
        let buf_size = 2 * MAXHUF + 8;
        let mut buf_cnt = vec![0u32; buf_size];
        let mut buf_left = vec![0u16; buf_size];
        let mut buf_right = vec![0u16; buf_size];
        let mut buf_bits = vec![0u8; buf_size];
        let mut buf_code = vec![0u32; buf_size];

        // Fill leaf nodes
        for (i, &sym) in sorted.iter().enumerate() {
            buf_cnt[i] = self.counter[sym as usize];
            buf_left[i] = sym as u16; // for leaf nodes, left = symbol index
        }
        // Fence
        for i in 0..4 {
            buf_cnt[b + i] = u32::MAX;
        }

        let mut p1 = 0usize; // next original node
        let mut p2 = b + 2; // next combined node (starts past fence region)
        let mut p3 = b + 2; // next slot for new combined node

        // Degenerate case: 0 or 1 symbols → assign depth 0
        if b <= 1 {
            if b == 1 {
                buf_bits[0] = 0;
                buf_code[0] = 0;
                self.maxbits = 0;
            }
        } else {
            while !(p1 == b && p3 - p2 == 1) {
                let (lchild, rchild, new_cnt) = if buf_cnt[p1 + 1] < buf_cnt[p2] {
                    // Both smallest are original
                    let c = buf_cnt[p1].saturating_add(buf_cnt[p1 + 1]);
                    let l = p1;
                    let r = p1 + 1;
                    p1 += 2;
                    (l, r, c)
                } else if buf_cnt[p1] > buf_cnt[p2 + 1] {
                    // Both smallest are combined
                    let c = buf_cnt[p2].saturating_add(buf_cnt[p2 + 1]);
                    let l = p2;
                    let r = p2 + 1;
                    p2 += 2;
                    (l, r, c)
                } else {
                    // One from each list
                    let c = buf_cnt[p1].saturating_add(buf_cnt[p2]);
                    let l = p1;
                    let r = p2;
                    p1 += 1;
                    p2 += 1;
                    (l, r, c)
                };
                buf_cnt[p3] = new_cnt;
                buf_left[p3] = lchild as u16;
                buf_right[p3] = rchild as u16;
                p3 += 1;
            }

            // ── Phase 3: propagate bit-depths from root down ────────────────────
            let root = p2; // last combined node = root
            buf_bits[root] = 0;
            buf_code[root] = 0;
            for i in (b + 2..=root).rev() {
                let l = buf_left[i] as usize;
                let r = buf_right[i] as usize;
                let parent_bits = buf_bits[i];
                let parent_code = buf_code[i];
                buf_bits[l] = parent_bits + 1;
                buf_code[l] = parent_code;
                buf_bits[r] = parent_bits + 1;
                buf_code[r] = parent_code | (1 << parent_bits);
            }

            // maxbits = bit count of the rarest symbol (buf[0] = lowest counter)
            self.maxbits = buf_bits[0] as u32;
        }

        // ── Phase 4: build decoder lookup tables ───────────────────────────────
        if self.maxbits > self.maxbits_so_far {
            self.maxbits_so_far = self.maxbits;
            self.index.resize(1 << self.maxbits, 0);
        }

        for i in 0..b {
            let s = buf_left[i] as usize; // symbol
            let sbits = buf_bits[i] as u32;
            let scode = buf_code[i];
            self.bits[s] = sbits as u8;
            self.codes[s] = scode;

            if sbits <= FAST_BITS {
                let fill = 1u32 << (FAST_BITS - sbits);
                for j in 0..fill {
                    self.fast_index[(scode + (j << sbits)) as usize] = s as i32;
                }
            } else {
                // Mark the short-prefix entry as "long code"
                self.fast_index[(scode & ((1 << FAST_BITS) - 1)) as usize] = -1;
                let fill = 1u32 << (self.maxbits - sbits);
                for j in 0..fill {
                    self.index[(scode + (j << sbits)) as usize] = s as u16;
                }
            }
        }

        // ── Phase 5: rescale counters ──────────────────────────────────────────
        let factor: u32 = match rescale_mode {
            0 => 2,
            1 => 3,
            2 => 4,
            3 => 6,
            4 => 8,
            5 => 10,
            6 => 12,
            _ => 16,
        };
        for s in 0..n {
            let c = self.counter[s];
            self.counter[s] = if c > 1 && c < factor {
                c - 1
            } else {
                c - c / factor
            };
        }
    }
}

// ── Huffman bit-stream decoder ─────────────────────────────────────────────────

/// Semi-adaptive Huffman decoder (mirrors `HuffmanDecoder<EOB_CODE>` from
/// `EntropyCoder.cpp`).
///
/// Wraps a `BitDecoder` reader and a `HuffmanTree`. Transparently handles EOB
/// codes by rebuilding the tree and reading the next symbol.
pub struct HuffmanDecoder<R: Read> {
    pub tree: HuffmanTree,
    pub reader: BitDecoder<R>,
    pub remainder: usize, // symbols until next mandatory rebuild
}

impl<R: Read> HuffmanDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            tree: HuffmanTree::new(CODES),
            reader: BitDecoder::new(reader),
            remainder: HUFBLOCK_INIT,
        }
    }

    /// Decode one symbol, transparently skipping EOB (tree-rebuild) tokens.
    pub fn decode_sym(&mut self) -> Result<u32> {
        loop {
            // Mandatory periodic rebuild
            if self.remainder == 0 {
                self.tree.build_tree(3); // rescale_mode=3 (same as encoder)
                self.remainder = HUFBLOCK;
            }
            self.remainder -= 1;

            // needbits peeks without consuming; dump then consumes exactly
            // the code's length — matching EntropyCoder.cpp's pattern.
            let bits = self.reader.needbits(self.tree.maxbits)?;
            let x = self.tree.decode(bits);
            self.reader.dump(self.tree.bits[x as usize] as u32);

            if x as usize == EOB_CODE {
                let rm = self.reader.getbits(3)?;
                self.tree.build_tree(rm);
                // don't decrement remainder (rebuild just happened)
                continue;
            }

            self.tree.inc(x as usize);
            return Ok(x);
        }
    }

    /// Read `n` raw bits from the underlying bit stream.
    pub fn getbits(&mut self, n: u32) -> Result<u32> {
        self.reader.getbits(n)
    }
}

// ── Bit decoder (shared between BitCoder and HuffmanDecoder) ──────────────────

/// LSB-first bit stream decoder (mirrors `InputBitStream` in `EntropyCoder.cpp`).
pub struct BitDecoder<R: Read> {
    pub reader: R,
    bitbuf: u64,
    bitcount: u32,
}

impl<R: Read> BitDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            bitbuf: 0,
            bitcount: 0,
        }
    }

    fn refill(&mut self) -> Result<()> {
        let mut b = [0u8; 4];
        let mut read = 0usize;
        while read < b.len() {
            match self.reader.read(&mut b[read..]) {
                Ok(0) if read == 0 => {
                    return Err(ArcError::Codec {
                        codec: "tornado",
                        message: "unexpected end of bitstream".to_string(),
                    });
                }
                Ok(0) => break,
                Ok(n) => read += n,
                Err(e) => {
                    return Err(ArcError::Codec {
                        codec: "tornado",
                        message: e.to_string(),
                    });
                }
            }
        }
        self.bitbuf |= (u32::from_le_bytes(b) as u64) << self.bitcount;
        self.bitcount += 32;
        Ok(())
    }

    /// Read n bits from the LSB end of the stream (n ≤ 32).
    pub fn getbits(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Ok(0);
        }
        if self.bitcount <= 32 {
            self.refill()?;
        }
        let val = (self.bitbuf & ((1u64 << n) - 1)) as u32;
        self.bitbuf >>= n;
        self.bitcount -= n;
        Ok(val)
    }

    /// Discard `n` bits already peeked with `needbits`.
    pub fn dump(&mut self, n: u32) {
        self.bitbuf >>= n;
        self.bitcount = self.bitcount.saturating_sub(n);
    }

    /// Ensure at least `n` bits are in the buffer and return the low `n` bits
    /// WITHOUT consuming them.
    pub fn needbits(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Ok(0);
        }
        if self.bitcount <= 32 {
            self.refill()?;
        }
        Ok((self.bitbuf & ((1u64 << n) - 1)) as u32)
    }
}

// ── LZ77 + Huffman decoder ─────────────────────────────────────────────────────

/// Matches `LZ77_Decoder<HuffmanDecoder<EOB_CODE>>` in `LZ77_Coder.cpp`.
///
/// Adds repeat-previous-distance codes and REPCHAR on top of the base Huffman
/// symbol stream. The internal `x` field holds the most-recently decoded code.
pub struct Lz77HuffDecoder<R: Read> {
    pub dec: HuffmanDecoder<R>,
    /// Ring buffer of recent match distances (mirrors `prevdists[]`).
    prevdists: [u32; 128],
    prevdist_pos: usize, // points one past the last written entry
    x: u32,              // last decoded symbol
}

impl<R: Read> Lz77HuffDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            dec: HuffmanDecoder::new(reader),
            prevdists: [0u32; 128],
            prevdist_pos: REPDIST_CODES,
            x: 0,
        }
    }

    /// Decode next symbol; returns `true` if it is a literal or REPCHAR (≡
    /// copy-one-byte-at-previous-distance, handled as a 1-byte match).
    pub fn is_literal(&mut self) -> Result<bool> {
        self.x = self.dec.decode_sym()?;
        Ok(self.x < 256)
    }

    /// Return the literal byte after `is_literal()` returned `true`.
    pub fn getchar(&self) -> u8 {
        self.x as u8
    }

    /// Decode match length. Must be called immediately after `is_literal()`
    /// returned `false`, before `getdist()`.
    pub fn getlen(&mut self, minlen: u32) -> Result<u32> {
        if self.x as usize == REPCHAR {
            return Ok(1);
        }
        let lcode = (self.x - 256) as usize % LEN_CODES;
        let lbits = LC2_EXTRA[lcode];
        let lbase = LC2_BASE[lcode];
        let extra = self.dec.getbits(lbits)?;
        let len = lbase + extra;
        // Map special len values to IMPOSSIBLE_LEN sentinel or table codes
        let decoded = if len > 100 {
            if len <= 104 {
                // Table-type code
                len - 100 + super::decode::IMPOSSIBLE_LEN
            } else {
                // Very long match
                len - 4 + minlen
            }
        } else {
            len + minlen
        };
        Ok(decoded)
    }

    /// Decode match distance. Must be called immediately after `getlen()`.
    pub fn getdist(&mut self) -> Result<u32> {
        if self.x as usize == REPCHAR {
            // Use the most-recent previous distance (no update)
            return Ok(self.prevdist_last());
        }

        let dcode_raw = ((self.x - 256) as usize) / LEN_CODES;

        if dcode_raw < REPDIST_CODES {
            // Repeat-previous-distance code: rotate recent distances
            let n = dcode_raw + 1;
            let dist = self.prevdist_nth(n);
            self.prevdist_promote(n);
            return Ok(dist);
        }

        let dcode = dcode_raw - REPDIST_CODES;
        let dbits = DC_EXTRA[dcode];
        let dbase = DC_BASE[dcode];
        let extra = self.dec.getbits(dbits)?;
        let dist = dbase + extra + 1; // +1: C uses 0-based internal, 1-based external

        self.prevdist_push(dist);
        Ok(dist)
    }

    // ── prevdist ring-buffer helpers ───────────────────────────────────────────

    fn prevdist_last(&self) -> u32 {
        self.prevdists[self.prevdist_pos - 1]
    }

    /// Return the Nth most-recent distance (1 = most recent).
    fn prevdist_nth(&self, n: usize) -> u32 {
        self.prevdists[self.prevdist_pos - n]
    }

    /// Bring the Nth entry to the front (rotate entries after it forward by 1).
    fn prevdist_promote(&mut self, n: usize) {
        let idx = self.prevdist_pos - n;
        let dist = self.prevdists[idx];
        for i in idx..self.prevdist_pos - 1 {
            self.prevdists[i] = self.prevdists[i + 1];
        }
        self.prevdists[self.prevdist_pos - 1] = dist;
    }

    fn prevdist_push(&mut self, dist: u32) {
        // If the buffer is nearly full, compact: keep last REPDIST_CODES-1 entries
        if self.prevdist_pos == self.prevdists.len() {
            let keep = REPDIST_CODES - 1;
            let src = self.prevdist_pos - keep;
            for i in 0..keep {
                self.prevdists[i] = self.prevdists[src + i];
            }
            self.prevdist_pos = keep;
        }
        self.prevdists[self.prevdist_pos] = dist;
        self.prevdist_pos += 1;
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Huffman encoder
// ════════════════════════════════════════════════════════════════════════════

use crate::codec::tornado::decode::{IMPOSSIBLE_DIST, IMPOSSIBLE_LEN};

// ── HuffBitWriter ─────────────────────────────────────────────────────────────
// LSB-first bit packing into LE 32-bit words — symmetric with BitDecoder.

struct HuffBitWriter<W: Write> {
    writer: W,
    bitbuf: u64,
    bitcount: u32,
}

impl<W: Write> HuffBitWriter<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            bitbuf: 0,
            bitcount: 0,
        }
    }

    fn write_bits(&mut self, bits: u32, value: u32) -> Result<()> {
        if bits == 0 {
            return Ok(());
        }
        let mask = if bits == 32 {
            u32::MAX
        } else {
            (1u32 << bits) - 1
        };
        self.bitbuf |= ((value & mask) as u64) << self.bitcount;
        self.bitcount += bits;
        while self.bitcount >= 32 {
            let word = (self.bitbuf as u32).to_le_bytes();
            self.writer.write_all(&word).map_err(huff_io_err)?;
            self.bitbuf >>= 32;
            self.bitcount -= 32;
        }
        Ok(())
    }

    fn flush(mut self) -> Result<W> {
        if self.bitcount > 0 {
            // Pad the partial word to 32 bits before writing.
            let word = (self.bitbuf as u32).to_le_bytes();
            self.writer.write_all(&word).map_err(huff_io_err)?;
        }
        // The BitDecoder refills 4 bytes at a time before consuming bits.
        // Write extra zero words so a refill during the last symbol's extra-bits
        // read never hits EOF.  These bytes are never decoded as symbols.
        self.writer.write_all(&[0u8; 8]).map_err(huff_io_err)?;
        Ok(self.writer)
    }
}

fn huff_io_err(e: std::io::Error) -> ArcError {
    ArcError::Codec {
        codec: "tornado",
        message: e.to_string(),
    }
}

// ── VLE lookup ────────────────────────────────────────────────────────────────

fn huff_vle_code(value: u32, base: &[u32], extra: &[u32]) -> Option<(usize, u32)> {
    for (c, (&b, &bits)) in base.iter().zip(extra.iter()).enumerate() {
        let span = if bits >= 32 {
            u32::MAX
        } else {
            (1u32 << bits).wrapping_sub(1)
        };
        if value >= b && value <= b.saturating_add(span) {
            return Some((c, value - b));
        }
    }
    None
}

// ── Lz77HuffEncoder ───────────────────────────────────────────────────────────

/// LZ77 command encoder backed by semi-adaptive Huffman coding.
///
/// Mirrors `LZ77_Coder<HuffmanDecoder<EOB_CODE>>` from `LZ77_Coder.cpp`.
///
/// The encoder and decoder share a self-synchronized mandatory rebuild
/// protocol: both count `HUFBLOCK_INIT` symbols initially, then `HUFBLOCK`
/// symbols per block, rebuilding with `rescale_mode = 3` on each boundary.
/// No explicit EOB token is emitted — rebuilds are implicit from the count.
///
/// Supports REPCHAR and repdist optimisations: matches at recently-used
/// distances use compact repdist codes; same-byte literals at the previous
/// distance use the single REPCHAR symbol.
pub struct Lz77HuffEncoder<W: Write> {
    tree: HuffmanTree,
    writer: HuffBitWriter<W>,
    remainder: usize,
    prevdists: [u32; 128],
    prevdist_pos: usize,
}

impl<W: Write> Lz77HuffEncoder<W> {
    pub fn new(writer: W) -> Self {
        Self {
            tree: HuffmanTree::new(CODES),
            writer: HuffBitWriter::new(writer),
            remainder: HUFBLOCK_INIT,
            prevdists: [0u32; 128],
            prevdist_pos: REPDIST_CODES,
        }
    }

    /// Most-recent match distance (0 if no match has been emitted yet).
    pub fn prevdist_last(&self) -> u32 {
        self.prevdists[self.prevdist_pos - 1]
    }

    fn prevdist_nth(&self, n: usize) -> u32 {
        self.prevdists[self.prevdist_pos - n]
    }

    fn prevdist_push(&mut self, dist: u32) {
        if self.prevdist_pos == self.prevdists.len() {
            let keep = REPDIST_CODES - 1;
            let src = self.prevdist_pos - keep;
            for i in 0..keep {
                self.prevdists[i] = self.prevdists[src + i];
            }
            self.prevdist_pos = keep;
        }
        self.prevdists[self.prevdist_pos] = dist;
        self.prevdist_pos += 1;
    }

    fn prevdist_promote(&mut self, n: usize) {
        let idx = self.prevdist_pos - n;
        let dist = self.prevdists[idx];
        for i in idx..self.prevdist_pos - 1 {
            self.prevdists[i] = self.prevdists[i + 1];
        }
        self.prevdists[self.prevdist_pos - 1] = dist;
    }

    /// Look up `dist` in the recent-distance ring; return Some(n) (1-based) if found.
    fn find_repdist(&self, dist: u32) -> Option<usize> {
        for n in 1..=REPDIST_CODES {
            if self.prevdist_nth(n) == dist {
                return Some(n);
            }
        }
        None
    }

    /// Emit the REPCHAR symbol (1-byte match at the previous distance).
    pub fn encode_repchar(&mut self) -> Result<()> {
        self.emit_sym(REPCHAR)
        // No ring update: REPCHAR does not change the prevdist ring.
    }

    /// Emit one symbol, updating the tree counter and triggering mandatory
    /// rebuilds exactly as the decoder does.
    fn emit_sym(&mut self, sym: usize) -> Result<()> {
        if self.remainder == 0 {
            self.tree.build_tree(3);
            self.remainder = HUFBLOCK;
        }
        self.remainder -= 1;

        let bits = self.tree.bits[sym] as u32;
        let code = self.tree.codes[sym];
        self.writer.write_bits(bits, code)?;

        self.tree.inc(sym);
        Ok(())
    }

    /// Encode one literal byte.
    pub fn encode_literal(&mut self, b: u8) -> Result<()> {
        self.emit_sym(b as usize)
    }

    /// Encode an LZ match of `len` bytes at `dist` positions back (1-based).
    ///
    /// `minlen` is the stream minimum-match length from the header.
    pub fn encode_match(&mut self, len: usize, dist: usize, minlen: u32) -> Result<()> {
        let mut raw_len = (len as u32).wrapping_sub(minlen);
        if raw_len > 100 && raw_len <= IMPOSSIBLE_LEN {
            raw_len = raw_len.wrapping_add(4);
        }

        let (lcode, len_extra) =
            huff_vle_code(raw_len, &LC2_BASE, &LC2_EXTRA).ok_or_else(|| ArcError::Codec {
                codec: "tornado",
                message: format!("huff encoder: length {len} out of range"),
            })?;

        if let Some(n) = self.find_repdist(dist as u32) {
            // Repdist: encode using the MRU slot index, no extra distance bits.
            let symbol = 256 + (n - 1) * LEN_CODES + lcode;
            self.emit_sym(symbol)?;
            self.writer.write_bits(LC2_EXTRA[lcode], len_extra)?;
            self.prevdist_promote(n);
        } else {
            // Full distance code: offset past the REPDIST slots.
            let dist_0 = dist as u32 - 1;
            let (dcode, dist_extra) =
                huff_vle_code(dist_0, &DC_BASE, &DC_EXTRA).ok_or_else(|| ArcError::Codec {
                    codec: "tornado",
                    message: format!("huff encoder: distance {dist} out of range"),
                })?;
            let dcode_full = dcode + REPDIST_CODES;
            let symbol = 256 + dcode_full * LEN_CODES + lcode;
            self.emit_sym(symbol)?;
            self.writer.write_bits(LC2_EXTRA[lcode], len_extra)?;
            self.writer.write_bits(DC_EXTRA[dcode], dist_extra)?;
            self.prevdist_push(dist as u32);
        }
        Ok(())
    }

    /// Encode the EOF sentinel (bypasses repdist ring — IMPOSSIBLE_DIST is not a real distance).
    pub fn encode_eof(&mut self, minlen: u32) -> Result<()> {
        self.encode_match_raw(IMPOSSIBLE_LEN, IMPOSSIBLE_DIST, minlen)
    }

    /// Encode a table preprocessing command.
    ///
    /// `type_n` is the bytes-per-element (1..4, typically 2 or 4).
    /// `items` is the element count.  Wire encoding: `raw_len = 100 + type_n`,
    /// `dist_0 = items - 1`; decoder maps `raw_len 101..104` to
    /// `IMPOSSIBLE_LEN + type_n` so it can invoke `undiff_table`.
    /// Table commands use full distance coding and do not update the prevdist ring.
    pub fn encode_table(&mut self, type_n: usize, items: usize) -> Result<()> {
        let raw_len = 100 + type_n as u32;
        let dist_0 = items as u32 - 1;

        let (lcode, len_extra) =
            huff_vle_code(raw_len, &LC2_BASE, &LC2_EXTRA).ok_or_else(|| ArcError::Codec {
                codec: "tornado",
                message: format!("huff encoder: table type {type_n} out of LC2 range"),
            })?;
        let (dcode, dist_extra) =
            huff_vle_code(dist_0, &DC_BASE, &DC_EXTRA).ok_or_else(|| ArcError::Codec {
                codec: "tornado",
                message: format!("huff encoder: table items {items} out of DC range"),
            })?;

        let dcode_full = dcode + REPDIST_CODES;
        let symbol = 256 + dcode_full * LEN_CODES + lcode;
        self.emit_sym(symbol)?;
        self.writer.write_bits(LC2_EXTRA[lcode], len_extra)?;
        self.writer.write_bits(DC_EXTRA[dcode], dist_extra)?;
        Ok(())
    }

    /// Raw match encoder: full distance code, no repdist optimisation.
    /// Used by encode_eof and encode_table where the dist is not a real LZ distance.
    fn encode_match_raw(&mut self, len: u32, dist: u32, minlen: u32) -> Result<()> {
        let mut raw_len = len.wrapping_sub(minlen);
        if raw_len > 100 && raw_len <= IMPOSSIBLE_LEN {
            raw_len = raw_len.wrapping_add(4);
        }
        let dist_0 = dist - 1;
        let (lcode, len_extra) =
            huff_vle_code(raw_len, &LC2_BASE, &LC2_EXTRA).ok_or_else(|| ArcError::Codec {
                codec: "tornado",
                message: format!("huff encoder: eof/raw length out of range"),
            })?;
        let (dcode, dist_extra) =
            huff_vle_code(dist_0, &DC_BASE, &DC_EXTRA).ok_or_else(|| ArcError::Codec {
                codec: "tornado",
                message: format!("huff encoder: eof/raw distance out of range"),
            })?;
        let dcode_full = dcode + REPDIST_CODES;
        let symbol = 256 + dcode_full * LEN_CODES + lcode;
        self.emit_sym(symbol)?;
        self.writer.write_bits(LC2_EXTRA[lcode], len_extra)?;
        self.writer.write_bits(DC_EXTRA[dcode], dist_extra)?;
        Ok(())
    }

    /// Flush remaining bits and return the underlying writer.
    pub fn finish(self) -> Result<W> {
        self.writer.flush()
    }
}
