use std::io::{Read, Write};

use super::super::internal::PPMD_BIN_SCALE;
use super::super::Error;

const K_TOP_VALUE: u32 = 1 << 24;
const K_BOT_VALUE: u32 = 1 << 15;

// Subbotin carryless range decoder. Compatible with FreeArc PPMdH streams.
// Differs from the 7-zip range coder in three ways:
//  1. No sentinel byte on init — just 4 code bytes.
//  2. `low` tracks the interval start; `code` = code_actual - low (relative).
//  3. Normalization uses the carry-less condition on (low ^ (low+range)).
pub(crate) struct RangeDecoder<R: Read> {
    pub(crate) range: u32,
    pub(crate) code: u32,
    pub(crate) low: u32,
    pub(crate) reader: R,
}

impl<R: Read> RangeDecoder<R> {
    pub(crate) fn new(reader: R) -> super::super::Result<Self> {
        let mut dec = Self {
            range: 0xFFFFFFFF,
            code: 0,
            low: 0,
            reader,
        };

        for _ in 0..4 {
            dec.code = dec.code << 8 | dec.read_byte().map_err(Error::IoError)?;
        }

        Ok(dec)
    }

    #[inline(always)]
    pub(crate) fn read_byte(&mut self) -> Result<u32, std::io::Error> {
        let mut buf = [0u8];
        self.reader.read_exact(&mut buf)?;
        Ok(buf[0] as u32)
    }

    #[inline(always)]
    pub(crate) fn get_threshold(&mut self, total: u32) -> u32 {
        self.range /= total;
        self.code / self.range
    }

    // Symbol is "bit 0" (low half): new range = size.
    #[inline(always)]
    pub(crate) fn decode_bit_0(&mut self, size: u32) -> Result<(), std::io::Error> {
        self.range = size;
        self.normalize_remote()?;
        Ok(())
    }

    // Symbol is "bit 1" (high half): low advances by size.
    #[inline(always)]
    pub(crate) fn decode_bit_1(&mut self, size: u32) {
        self.low = self.low.wrapping_add(size);
        self.code = self.code.wrapping_sub(size);
        self.range = (self.range & !(PPMD_BIN_SCALE - 1)).wrapping_sub(size);
    }

    #[inline(always)]
    pub(crate) fn decode(&mut self, start: u32, size: u32) {
        let s = start.wrapping_mul(self.range);
        self.low = self.low.wrapping_add(s);
        self.code = self.code.wrapping_sub(s);
        self.range = self.range.wrapping_mul(size);
    }

    #[inline(always)]
    pub(crate) fn decode_final(&mut self, start: u32, size: u32) -> Result<(), std::io::Error> {
        self.decode(start, size);
        self.normalize_remote()
    }

    #[inline(always)]
    pub(crate) fn normalize_remote(&mut self) -> Result<(), std::io::Error> {
        while self.low ^ self.low.wrapping_add(self.range) < K_TOP_VALUE
            || self.range < K_BOT_VALUE && {
                self.range = 0u32.wrapping_sub(self.low) & (K_BOT_VALUE - 1);
                true
            }
        {
            self.code = self.code << 8 | self.read_byte()?;
            self.range <<= 8;
            self.low <<= 8;
        }
        Ok(())
    }
}

// Subbotin carryless range encoder. Uses u32 `low` (no carry-cache), 4-byte flush.
pub(crate) struct RangeEncoder<W: Write> {
    pub(crate) range: u32,
    pub(crate) low: u32,
    pub(crate) writer: W,
}

impl<W: Write> RangeEncoder<W> {
    pub(crate) fn new(writer: W) -> Self {
        Self {
            range: 0xFFFFFFFF,
            low: 0,
            writer,
        }
    }

    #[inline(always)]
    pub(crate) fn write_byte(&mut self, byte: u8) -> Result<(), std::io::Error> {
        self.writer.write_all(&[byte])
    }

    // Symbol is "bit 0" (low half): new range = bound.
    #[inline(always)]
    pub(crate) fn encode_bit_0(&mut self, bound: u32) -> Result<(), std::io::Error> {
        self.range = bound;
        self.normalize_remote()?;
        Ok(())
    }

    // Symbol is "bit 1" (high half): low advances by bound.
    #[inline(always)]
    pub(crate) fn encode_bit_1(&mut self, bound: u32) -> Result<(), std::io::Error> {
        self.low = self.low.wrapping_add(bound);
        self.range = (self.range & !(PPMD_BIN_SCALE - 1)).wrapping_sub(bound);
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn encode(&mut self, start: u32, size: u32) {
        let s = start.wrapping_mul(self.range);
        self.low = self.low.wrapping_add(s);
        self.range = self.range.wrapping_mul(size);
    }

    #[inline(always)]
    pub(crate) fn encode_final(&mut self, start: u32, freq: u32) -> Result<(), std::io::Error> {
        self.encode(start, freq);
        self.normalize_remote()
    }

    #[inline(always)]
    pub(crate) fn normalize_remote(&mut self) -> Result<(), std::io::Error> {
        while self.low ^ self.low.wrapping_add(self.range) < K_TOP_VALUE
            || self.range < K_BOT_VALUE && {
                self.range = 0u32.wrapping_sub(self.low) & (K_BOT_VALUE - 1);
                true
            }
        {
            self.write_byte((self.low >> 24) as u8)?;
            self.range <<= 8;
            self.low <<= 8;
        }
        Ok(())
    }

    pub(crate) fn flush(&mut self) -> Result<(), std::io::Error> {
        for _ in 0..4 {
            self.write_byte((self.low >> 24) as u8)?;
            self.low <<= 8;
        }
        self.writer.flush()
    }
}
