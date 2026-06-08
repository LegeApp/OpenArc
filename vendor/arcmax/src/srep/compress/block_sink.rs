use byteorder::{WriteBytesExt, LE};
use std::io::Write;

use crate::srep::error::SrepError;
use crate::srep::format::{BlockHeader, StatCodec};
use crate::srep::types::{Offset, Stat};

/// Accumulates matches found in a single block, then drains them as the on-disk
/// representation for one SREP block (stats + literals).
///
/// The literal-only encoder (Stage D) never calls `record_match`. Real matchers
/// (Stage G+) advance `last_match_end` by calling `record_match` for each hit,
/// then call `finish()` to drain the remaining tail literals.
pub struct MatchSink<'b> {
    block: &'b [u8],
    codec: StatCodec,
    pub stats: Vec<Stat>,
    pub literals: Vec<u8>,
    last_match_end: usize,
}

impl<'b> MatchSink<'b> {
    pub fn new(block: &'b [u8], codec: StatCodec) -> Self {
        MatchSink {
            block,
            codec,
            stats: Vec::new(),
            literals: Vec::new(),
            last_match_end: 0,
        }
    }

    pub fn round_matches(&self) -> bool {
        self.codec.round_matches
    }

    /// Extend the literal buffer with all bytes from `last_match_end` up to `end`.
    pub fn flush_literals_through(&mut self, end: usize) {
        if end > self.last_match_end {
            self.literals
                .extend_from_slice(&self.block[self.last_match_end..end]);
            self.last_match_end = end;
        }
    }

    /// Record a confirmed match starting at `dest_in_block` in the current block,
    /// referencing `len` bytes at global offset `src`.
    ///
    /// All bytes before `dest_in_block` that haven't been claimed by a previous
    /// match are first flushed to the literal buffer.
    pub fn record_match(
        &mut self,
        dest_in_block: usize,
        src: Offset,
        len: u32,
        block_start: u64,
    ) -> Result<(), SrepError> {
        // Save position before flush so lit_len reflects bytes between the
        // previous match end and this match start.
        let lits_start = self.last_match_end;
        self.flush_literals_through(dest_in_block);
        let lit_len = (dest_in_block - lits_start) as u32;
        let dest = block_start + dest_in_block as u64;
        let offset = dest.saturating_sub(src);
        self.codec.encode(&mut self.stats, lit_len, offset, len)?;
        self.last_match_end = dest_in_block + len as usize;
        Ok(())
    }

    /// Flush any remaining tail bytes and return `(stats, literals)`.
    pub fn finish(mut self) -> (Vec<Stat>, Vec<u8>) {
        let end = self.block.len();
        self.flush_literals_through(end);
        (self.stats, self.literals)
    }
}

/// Writes one fully-prepared block to `w` in IO_LZ format (stats inline, literals follow).
pub fn write_io_lz_block<W: Write>(
    w: &mut W,
    stats: &[Stat],
    literals: &[u8],
    original_len: u32,
    digest: &[u8],
) -> Result<(), SrepError> {
    let stat_bytes = (stats.len() * size_of::<u32>()) as u32;
    BlockHeader {
        literal_bytes: literals.len() as u32,
        original_len,
        stat_size: stat_bytes,
    }
    .write(w, digest)?;

    for &s in stats {
        w.write_u32::<LE>(s).map_err(SrepError::Io)?;
    }
    w.write_all(literals).map_err(SrepError::Io)?;
    Ok(())
}

/// Writes one fully-prepared block to `w` in INDEX_LZ format (stat_size=0, stats deferred).
pub fn write_index_lz_block<W: Write>(
    w: &mut W,
    literals: &[u8],
    original_len: u32,
    digest: &[u8],
) -> Result<(), SrepError> {
    BlockHeader {
        literal_bytes: literals.len() as u32,
        original_len,
        stat_size: 0,
    }
    .write(w, digest)?;
    w.write_all(literals).map_err(SrepError::Io)?;
    Ok(())
}
