use std::io::{Read, Write};

use crate::codec::tornado::decode::{IMPOSSIBLE_DIST, IMPOSSIBLE_LEN};
use crate::codec::tornado::huffman::{
    CODES, DC_BASE, DC_EXTRA, LC2_BASE, LC2_EXTRA, LEN_CODES, REPCHAR, REPDIST_CODES,
};
use crate::error::{ArcError, Result};

// ── Range coder constants ──────────────────────────────────────────────────────

const RANGE_BITS: u32 = 14;
const RANGE: u32 = 1 << RANGE_BITS; // 16384
const INDEXES: usize = 2048;

// ── TRangeDecoder ─────────────────────────────────────────────────────────────

/// Shindler's range coder decoder (mirrors `TRangeDecoder` in `EntropyCoder.cpp`).
///
/// `range` and `buffer` are u32; arithmetic wraps as in C `unsigned int`.
struct TRangeDecoder<R: Read> {
    reader: R,
    range: u32,  // current range; always ≥ 2^24 after normalization
    buffer: u32, // current code buffer
}

impl<R: Read> TRangeDecoder<R> {
    fn new(mut reader: R) -> Result<Self> {
        // Read 5 bytes big-endian; the first byte "primes" the pipeline and
        // is discarded once all 5 bytes shift through the 32-bit buffer.
        let mut buffer = 0u32;
        for _ in 0..5 {
            buffer = buffer.wrapping_shl(8) | read_byte(&mut reader)? as u32;
        }
        Ok(Self {
            reader,
            range: 0xFFFF_FFFF,
            buffer,
        })
    }

    /// Decode next symbol: returns `buffer / (range >>= bits)`.
    ///
    /// Matches `GetCount(bits)` — caller must call `update` immediately after.
    fn get_count(&mut self, bits: u32) -> Result<u32> {
        self.range >>= bits;
        if self.range == 0 {
            return Err(ArcError::Codec {
                codec: "tornado",
                message: "range coder: range underflow".to_string(),
            });
        }
        let count = self.buffer / self.range;
        if count >= (1 << bits) {
            return Err(ArcError::Codec {
                codec: "tornado",
                message: format!("range coder data error: count {count} >= 1<<{bits}"),
            });
        }
        Ok(count)
    }

    /// Update buffer/range after decoding a symbol with cumulative `cum` and
    /// frequency `cnt`, then renormalize to keep range ≥ 2^24.
    ///
    /// Mirrors `Update(cum, cnt)`.
    fn update(&mut self, cum: u32, cnt: u32) -> Result<()> {
        self.buffer = self.buffer.wrapping_sub(cum.wrapping_mul(self.range));
        self.range = self.range.wrapping_mul(cnt);
        while self.range < (1 << 24) {
            self.range <<= 8;
            self.buffer = self.buffer.wrapping_shl(8) | read_byte(&mut self.reader)? as u32;
        }
        Ok(())
    }
}

fn read_byte<R: Read>(r: &mut R) -> Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b).map_err(|e| ArcError::Codec {
        codec: "tornado",
        message: e.to_string(),
    })?;
    Ok(b[0])
}

// ── TCounter ──────────────────────────────────────────────────────────────────

/// Semi-adaptive frequency counter (mirrors `TCounter<Decoder>` in
/// `EntropyCoder.cpp`).
///
/// Tracks symbol frequencies and rebuilds `cum`/`cnt`/`index` when
/// `remainder` reaches 0.  The `index` table allows O(1) initial symbol
/// lookup followed by a short linear scan.
struct TCounter {
    n: usize,
    remainder: u32,
    cnt: Vec<u32>,
    cum: Vec<u32>,
    livecnt: Vec<u32>,
    /// `index[k]` = first symbol whose range covers counts ≥ k * (RANGE/INDEXES).
    index: Vec<usize>,
}

impl TCounter {
    fn new(n: usize) -> Self {
        // Distribute RANGE evenly: first (RANGE%n) symbols get +1.
        let base = RANGE / n as u32;
        let extra = (RANGE % n as u32) as usize;
        let mut c = Self {
            n,
            remainder: 0,
            cnt: vec![0u32; n],
            cum: vec![0u32; n],
            livecnt: (0..n)
                .map(|s| base + if s < extra { 1 } else { 0 })
                .collect(),
            index: vec![0usize; INDEXES],
        };
        c.rescale();
        c
    }

    /// Count symbol `s` and trigger a rescale when the block is full.
    fn inc(&mut self, s: usize) {
        self.livecnt[s] += 1;
        self.remainder -= 1;
        if self.remainder == 0 {
            self.rescale();
        }
    }

    /// Rebuild `cnt`, `cum`, `index`; decay `livecnt`; reset `remainder`.
    ///
    /// Mirrors `TCounter::Rescale()`.
    fn rescale(&mut self) {
        let mut total = 0u32;
        self.remainder = RANGE;
        let mut ind = 0usize;
        let step = RANGE / INDEXES as u32; // = 8
        for s in 0..self.n {
            self.cnt[s] = self.livecnt[s];
            self.cum[s] = total;
            total += self.cnt[s];

            // Decay: decrease livecnt toward a floor of 1.
            let lc = self.livecnt[s];
            self.livecnt[s] -= if lc > 1 && lc < 6 { 1 } else { lc / 6 };
            self.remainder -= self.livecnt[s];

            // Fill index slots where this symbol's range ends (≥ ind*step).
            while ind < INDEXES && self.cum[s] + self.cnt[s] - 1 >= step * ind as u32 {
                self.index[ind] = s;
                ind += 1;
            }
        }
    }

    /// Find the symbol whose range contains `count`.
    ///
    /// Uses `index` for an O(1) starting hint, then a short linear scan.
    fn decode(&self, count: u32) -> usize {
        let step = RANGE / INDEXES as u32;
        let mut s = self.index[(count / step) as usize];
        while self.cum[s] + self.cnt[s] - 1 < count {
            s += 1;
        }
        s
    }
}

// ── ArithDecoder ──────────────────────────────────────────────────────────────

/// Semi-adaptive arithmetic coder decoder (mirrors `ArithDecoder<EOB_CODE>` in
/// `EntropyCoder.cpp`).
///
/// `decode()` returns the next symbol from the LZ77 alphabet (0..CODES).
/// `getbits(n)` reads `n` raw bits using the range coder itself.
pub struct ArithDecoder<R: Read> {
    rd: TRangeDecoder<R>,
    c: TCounter,
}

impl<R: Read> ArithDecoder<R> {
    pub fn new(reader: R) -> Result<Self> {
        // TCounter is initialised with the full LZ77 symbol alphabet (CODES
        // entries); CODES is imported from huffman.rs which owns it.
        let n = crate::codec::tornado::huffman::CODES;
        Ok(Self {
            rd: TRangeDecoder::new(reader)?,
            c: TCounter::new(n),
        })
    }

    /// Decode one LZ77 symbol.
    pub fn decode(&mut self) -> Result<u32> {
        let count = self.rd.get_count(RANGE_BITS)?;
        let x = self.c.decode(count);
        self.rd.update(self.c.cum[x], self.c.cnt[x])?;
        self.c.inc(x);
        Ok(x as u32)
    }

    /// Read `n` raw bits via the range coder (≡ `getbits(n)` in C++).
    ///
    /// For n > 24 the read is split into two GetCount calls (≤ 15 bits each)
    /// to stay within the 32-bit arithmetic bounds.
    pub fn getbits(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Ok(0);
        }
        if n <= 24 {
            let x = self.rd.get_count(n)?;
            self.rd.update(x, 1)?;
            Ok(x)
        } else {
            // n > 24: split as 15 + (n-15), mirroring C++ getbits.
            let x1 = self.rd.get_count(15)?;
            self.rd.update(x1, 1)?;
            let x2 = self.rd.get_count(n - 15)?;
            self.rd.update(x2, 1)?;
            Ok((x2 << 15) | x1)
        }
    }
}

// ── Lz77ArithDecoder ──────────────────────────────────────────────────────────

/// `LZ77_Decoder<ArithDecoder<EOB_CODE>>` (mirrors `LZ77_Decoder` in
/// `LZ77_Coder.cpp` with `ArithDecoder` as the entropy backend).
///
/// Adds repeat-previous-distance codes and REPCHAR on top of the base
/// arithmetic symbol stream. Unlike the Huffman variant, there is no
/// explicit EOB_CODE token — the TCounter rescales automatically via `Inc`.
pub struct Lz77ArithDecoder<R: Read> {
    dec: ArithDecoder<R>,
    prevdists: [u32; 128],
    prevdist_pos: usize,
    x: u32,
}

impl<R: Read> Lz77ArithDecoder<R> {
    pub fn new(reader: R) -> Result<Self> {
        Ok(Self {
            dec: ArithDecoder::new(reader)?,
            prevdists: [0u32; 128],
            prevdist_pos: REPDIST_CODES,
            x: 0,
        })
    }

    /// Decode next symbol; returns `true` if it is a literal byte.
    pub fn is_literal(&mut self) -> Result<bool> {
        self.x = self.dec.decode()?;
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
        let decoded = if len > 100 {
            if len <= 104 {
                len - 100 + IMPOSSIBLE_LEN
            } else {
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
            return Ok(self.prevdist_last());
        }
        let dcode_raw = (self.x - 256) as usize / LEN_CODES;
        if dcode_raw < REPDIST_CODES {
            let n = dcode_raw + 1;
            let dist = self.prevdist_nth(n);
            self.prevdist_promote(n);
            return Ok(dist);
        }
        let dcode = dcode_raw - REPDIST_CODES;
        let dbits = DC_EXTRA[dcode];
        let dbase = DC_BASE[dcode];
        let extra = self.dec.getbits(dbits)?;
        let dist = dbase + extra + 1;
        self.prevdist_push(dist);
        Ok(dist)
    }

    // ── prevdist ring-buffer helpers ───────────────────────────────────────────

    fn prevdist_last(&self) -> u32 {
        self.prevdists[self.prevdist_pos - 1]
    }

    fn prevdist_nth(&self, n: usize) -> u32 {
        self.prevdists[self.prevdist_pos - n]
    }

    fn prevdist_promote(&mut self, n: usize) {
        let idx = self.prevdist_pos - n;
        let dist = self.prevdists[idx];
        for i in idx..self.prevdist_pos - 1 {
            self.prevdists[i] = self.prevdists[i + 1];
        }
        self.prevdists[self.prevdist_pos - 1] = dist;
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
}

// ════════════════════════════════════════════════════════════════════════════
// Encoder side
// ════════════════════════════════════════════════════════════════════════════

fn write_byte<W: Write>(w: &mut W, b: u8) -> Result<()> {
    w.write_all(&[b]).map_err(|e| ArcError::Codec {
        codec: "tornado",
        message: e.to_string(),
    })
}

// ── TRangeEncoder ─────────────────────────────────────────────────────────────

/// Shindler's range coder encoder (mirrors `TRangeCoder` in `EntropyCoder.cpp`).
///
/// `low` is a 64-bit register: bits 0–31 are the working range accumulator;
/// bits 32+ hold the carry byte when an interval crosses a 32-bit boundary.
/// `buffer` holds the pending output byte and `help` counts pending 0xFF bytes
/// for carry-propagation.
pub(super) struct TRangeEncoder<W: Write> {
    writer: W,
    low: u64,
    range: u32,
    buffer: u32,
    help: u32,
}

impl<W: Write> TRangeEncoder<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            low: 0,
            range: 0xFFFF_FFFF,
            buffer: 0,
            help: 0,
        }
    }

    /// Carry-propagation flush step (mirrors `ShiftLow`).
    fn shift_low(&mut self) -> Result<()> {
        let low32 = self.low as u32;
        if (low32 ^ 0xFF00_0000) >= (1 << 24) {
            // Top byte of low32 is NOT 0xFF — safe to emit the buffered byte.
            let carry = (self.low >> 32) as u32; // 0 or 1
            write_byte(&mut self.writer, (self.buffer.wrapping_add(carry)) as u8)?;
            let fill = carry.wrapping_add(255) as u8; // 0x00 on carry, 0xFF otherwise
            for _ in 0..self.help {
                write_byte(&mut self.writer, fill)?;
            }
            self.help = 0;
            self.buffer = low32 >> 24;
        } else {
            // Top byte is 0xFF — defer; it may be modified by a future carry.
            self.help += 1;
        }
        // Mirror C++ `low = static_cast<unsigned int>(low) << 8`:
        // truncate to 32 bits first, then shift — top 8 bits are consumed/emitted.
        self.low = low32.wrapping_shl(8) as u64;
        Ok(())
    }

    /// Encode an interval: `Encode(cum, cnt, bits)` — mirrors `TRangeCoder::Encode`.
    ///
    /// Encodes a symbol with cumulative frequency `cum`, frequency `cnt`,
    /// and total range `1 << bits`.
    pub fn encode(&mut self, cum: u32, cnt: u32, bits: u32) -> Result<()> {
        self.range >>= bits;
        self.low += (cum as u64) * (self.range as u64);
        self.range = self.range.wrapping_mul(cnt);
        while self.range < (1 << 24) {
            self.range <<= 8;
            self.shift_low()?;
        }
        Ok(())
    }

    /// Flush all pending bytes (call once when done encoding).
    pub fn finish(mut self) -> Result<W> {
        for _ in 0..5 {
            self.shift_low()?;
        }
        Ok(self.writer)
    }
}

// ── TCounterEncoder ───────────────────────────────────────────────────────────

/// Semi-adaptive frequency counter for the encoder side.
///
/// Tracks `cnt`/`cum` arrays for interval encoding and decays `livecnt`
/// after each block of `RANGE` events, exactly mirroring `TCounter<Encoder>`.
struct TCounterEncoder {
    n: usize,
    remainder: u32,
    cnt: Vec<u32>,
    cum: Vec<u32>,
    livecnt: Vec<u32>,
}

impl TCounterEncoder {
    fn new(n: usize) -> Self {
        let base = RANGE / n as u32;
        let extra = (RANGE % n as u32) as usize;
        let mut c = Self {
            n,
            remainder: 0,
            cnt: vec![0u32; n],
            cum: vec![0u32; n],
            livecnt: (0..n)
                .map(|s| base + if s < extra { 1 } else { 0 })
                .collect(),
        };
        c.rescale();
        c
    }

    fn rescale(&mut self) {
        let mut total = 0u32;
        self.remainder = RANGE;
        for s in 0..self.n {
            self.cnt[s] = self.livecnt[s];
            self.cum[s] = total;
            total += self.cnt[s];
            let lc = self.livecnt[s];
            self.livecnt[s] -= if lc > 1 && lc < 6 { 1 } else { lc / 6 };
            self.remainder -= self.livecnt[s];
        }
    }

    fn inc(&mut self, s: usize) {
        self.livecnt[s] += 1;
        self.remainder -= 1;
        if self.remainder == 0 {
            self.rescale();
        }
    }
}

// ── ArithEncoder ──────────────────────────────────────────────────────────────

/// Semi-adaptive arithmetic symbol encoder (mirrors `ArithCoder<EOB_CODE>`).
///
/// `encode_symbol(x)` encodes one LZ77 alphabet symbol and updates statistics.
/// `putbits(n, val)` encodes `n` raw bits uniformly.
pub(super) struct ArithEncoder<W: Write> {
    enc: TRangeEncoder<W>,
    c: TCounterEncoder,
}

impl<W: Write> ArithEncoder<W> {
    pub fn new(writer: W, n: usize) -> Self {
        Self {
            enc: TRangeEncoder::new(writer),
            c: TCounterEncoder::new(n),
        }
    }

    /// Encode one symbol from the alphabet `[0, n)`.
    pub fn encode_symbol(&mut self, x: usize) -> Result<()> {
        self.enc.encode(self.c.cum[x], self.c.cnt[x], RANGE_BITS)?;
        self.c.inc(x);
        Ok(())
    }

    /// Encode `n` raw bits with a flat distribution (≡ `putlowerbits(n, val)`).
    ///
    /// Splits into at most two ≤15-bit calls so `range >> bits` never underflows.
    pub fn putbits(&mut self, n: u32, val: u32) -> Result<()> {
        if n == 0 {
            return Ok(());
        }
        if n <= 24 {
            let masked = val & ((1u32 << n).wrapping_sub(1));
            self.enc.encode(masked, 1, n)
        } else {
            // n > 24: split lower 15 bits then upper (n-15) bits.
            let lo = val & 0x7FFF;
            self.enc.encode(lo, 1, 15)?;
            let hi_bits = n - 15;
            let hi_mask = (1u32 << hi_bits).wrapping_sub(1);
            let hi = (val >> 15) & hi_mask;
            self.enc.encode(hi, 1, hi_bits)
        }
    }

    /// Flush the range coder and return the underlying writer.
    pub fn finish(self) -> Result<W> {
        self.enc.finish()
    }
}

// ── VLE helpers for Lz77ArithEncoder ─────────────────────────────────────────

/// Find the VLE code index and extra-bit value for `value` using `base`/`extra`
/// tables. Mirrors the lookup in `encode_match`.
fn arith_vle_code(value: u32, base: &[u32], extra: &[u32]) -> Option<(usize, u32)> {
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

// ── Lz77ArithEncoder ──────────────────────────────────────────────────────────

/// LZ77 command encoder backed by arithmetic coding.
///
/// Mirrors `LZ77_Coder<ArithCoder<CODES>>` from `LZ77_Coder.cpp`.
/// Supports REPCHAR and repdist optimisations: matches at recently-used
/// distances use compact repdist codes; same-byte literals at the previous
/// distance use the single REPCHAR symbol.
pub struct Lz77ArithEncoder<W: Write> {
    enc: ArithEncoder<W>,
    prevdists: [u32; 128],
    prevdist_pos: usize,
}

impl<W: Write> Lz77ArithEncoder<W> {
    pub fn new(writer: W) -> Self {
        Self {
            enc: ArithEncoder::new(writer, CODES),
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
        self.enc.encode_symbol(REPCHAR)
    }

    /// Encode one literal byte.
    pub fn encode_literal(&mut self, b: u8) -> Result<()> {
        self.enc.encode_symbol(b as usize)
    }

    /// Encode an LZ match of `len` bytes at `dist` positions back (1-based).
    ///
    /// `minlen` is the header minimum-match length (= `MIN_MATCH` = 4).
    pub fn encode_match(&mut self, len: usize, dist: usize, minlen: u32) -> Result<()> {
        let mut raw_len = (len as u32).wrapping_sub(minlen);
        if raw_len > 100 && raw_len <= IMPOSSIBLE_LEN {
            raw_len = raw_len.wrapping_add(4);
        }

        let (lcode, len_extra) =
            arith_vle_code(raw_len, &LC2_BASE, &LC2_EXTRA).ok_or_else(|| ArcError::Codec {
                codec: "tornado",
                message: format!("arith encoder: length {len} exceeds LC2 range"),
            })?;

        if let Some(n) = self.find_repdist(dist as u32) {
            let symbol = 256 + (n - 1) * LEN_CODES + lcode;
            self.enc.encode_symbol(symbol)?;
            self.enc.putbits(LC2_EXTRA[lcode], len_extra)?;
            self.prevdist_promote(n);
        } else {
            let dist_0 = dist as u32 - 1;
            let (dcode, dist_extra) =
                arith_vle_code(dist_0, &DC_BASE, &DC_EXTRA).ok_or_else(|| ArcError::Codec {
                    codec: "tornado",
                    message: format!("arith encoder: distance {dist} exceeds DC range"),
                })?;
            let dcode_raw = dcode + REPDIST_CODES;
            let symbol = 256 + dcode_raw * LEN_CODES + lcode;
            self.enc.encode_symbol(symbol)?;
            self.enc.putbits(LC2_EXTRA[lcode], len_extra)?;
            self.enc.putbits(DC_EXTRA[dcode], dist_extra)?;
            self.prevdist_push(dist as u32);
        }
        Ok(())
    }

    /// Encode the EOF sentinel (bypasses repdist ring — IMPOSSIBLE_DIST is not a real distance).
    pub fn encode_eof(&mut self, minlen: u32) -> Result<()> {
        self.encode_match_raw(IMPOSSIBLE_LEN, IMPOSSIBLE_DIST, minlen)
    }

    /// Raw match encoder: full distance code, no repdist optimisation.
    /// Used by encode_eof where the dist is not a real LZ distance.
    fn encode_match_raw(&mut self, len: u32, dist: u32, minlen: u32) -> Result<()> {
        let mut raw_len = len.wrapping_sub(minlen);
        if raw_len > 100 && raw_len <= IMPOSSIBLE_LEN {
            raw_len = raw_len.wrapping_add(4);
        }
        let dist_0 = dist - 1;
        let (lcode, len_extra) =
            arith_vle_code(raw_len, &LC2_BASE, &LC2_EXTRA).ok_or_else(|| ArcError::Codec {
                codec: "tornado",
                message: format!("arith encoder: eof/raw length out of range"),
            })?;
        let (dcode, dist_extra) =
            arith_vle_code(dist_0, &DC_BASE, &DC_EXTRA).ok_or_else(|| ArcError::Codec {
                codec: "tornado",
                message: format!("arith encoder: eof/raw distance out of range"),
            })?;
        let dcode_full = dcode + REPDIST_CODES;
        let symbol = 256 + dcode_full * LEN_CODES + lcode;
        self.enc.encode_symbol(symbol)?;
        self.enc.putbits(LC2_EXTRA[lcode], len_extra)?;
        self.enc.putbits(DC_EXTRA[dcode], dist_extra)?;
        Ok(())
    }

    /// Encode a table preprocessing command.
    ///
    /// `type_n` bytes per element (1..4, typically 2 or 4), `items` elements.
    /// Wire: `raw_len = 100 + type_n`, `dist_0 = items - 1`.
    /// Table commands use full distance coding and do not update the prevdist ring.
    pub fn encode_table(&mut self, type_n: usize, items: usize) -> Result<()> {
        let raw_len = 100 + type_n as u32;
        let dist_0 = items as u32 - 1;

        let (lcode, len_extra) =
            arith_vle_code(raw_len, &LC2_BASE, &LC2_EXTRA).ok_or_else(|| ArcError::Codec {
                codec: "tornado",
                message: format!("arith encoder: table type {type_n} out of LC2 range"),
            })?;
        let (dcode, dist_extra) =
            arith_vle_code(dist_0, &DC_BASE, &DC_EXTRA).ok_or_else(|| ArcError::Codec {
                codec: "tornado",
                message: format!("arith encoder: table items {items} out of DC range"),
            })?;

        let dcode_raw = dcode + REPDIST_CODES;
        let symbol = 256 + dcode_raw * LEN_CODES + lcode;
        self.enc.encode_symbol(symbol)?;
        self.enc.putbits(LC2_EXTRA[lcode], len_extra)?;
        self.enc.putbits(DC_EXTRA[dcode], dist_extra)?;
        Ok(())
    }

    /// Flush the arithmetic coder and return the underlying writer.
    pub fn finish(self) -> Result<W> {
        self.enc.finish()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
