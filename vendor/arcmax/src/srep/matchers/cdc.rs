//! Content-defined chunking (CDC) matchers for SREP `-m1` (fast) and `-m2` (ZPAQ).
//!
//! Ports `compress_cdc.cpp` from the FreeARC C++ source. The CDC matcher splits
//! input into variable-size chunks at content-defined boundaries and deduplicates
//! whole chunks against a per-run hash table indexed by chunk digest.
//!
//! ## Fast CDC (-m1)
//! Processes 3 parallel streams within each stripe (116 KiB), each using a rolling
//! CRC or polynomial hash. Points where `hash > maxhash` become chunk boundaries.
//! The 3-stream approach gives denser boundary coverage than a single stream.
//!
//! ## ZPAQ CDC (-m2)
//! Uses an order-1 context model. The rolling hash multiplier is different for
//! predicted vs. mispredicted bytes. On a boundary, the context model resets.

use std::collections::HashMap;

use siphasher::sip::SipHasher13;
use std::hash::Hasher;

use crate::srep::compress::block_sink::MatchSink;
use crate::srep::error::SrepError;
use crate::srep::format::StatCodec;
use crate::srep::hash::rolling::{CrcRollingHash, PolyRolling64, CRC32_CASTAGNOLI, PRIME_M3};

// --------------------------------------------------------------------------
// Constants (match the C++ source)
// --------------------------------------------------------------------------

/// Stripe size for parallel processing: 116 KiB.
/// 116 KiB + 2 VHASH tables fits in a 512 KiB per-core cache.
const STRIPE: usize = 116 * 1024;

/// Window size for the rolling hash (bytes).
const WINSIZE: usize = 48;

/// Minimum chunk size (= MINIMAL_MIN_MATCH in C++).
pub const MIN_CHUNK: usize = 16;

// --------------------------------------------------------------------------
// CDC mode
// --------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdcMode {
    Fast,
    Zpaq,
}

// --------------------------------------------------------------------------
// Boundary finding: fast CDC (3-stream polynomial/CRC rolling hash)
// --------------------------------------------------------------------------

/// Find chunk boundaries in `block` for fast CDC (`-m1`).
///
/// Appends boundary offsets to `out` (in sorted order within each stripe).
/// The final entry `block.len()` is always appended to mark the end.
pub fn find_chunks_fast(block: &[u8], avg_len: usize, out: &mut Vec<usize>) {
    // maxhash: probability ~1/avg_len that hash > maxhash for random u32.
    let avg = avg_len.max(1) as u32;
    let maxhash: u32 = u32::MAX - u32::MAX / avg;

    let mut ptr = 0usize;

    while ptr < block.len() {
        let stripe_end = (ptr + STRIPE).min(block.len());
        let stripe = &block[ptr..stripe_end];
        let piece = stripe.len() / 3;

        if piece >= WINSIZE {
            // 3-stream mode: each stream processes `piece` bytes.
            let mut marks: Vec<usize> = Vec::new();
            for stream in 0..3 {
                let start = stream * piece;
                let end = ((stream + 1) * piece).min(stripe.len());
                if end <= start + WINSIZE {
                    continue;
                }
                let sub = &stripe[start..end];
                let mut hash = CrcRollingHash::new(WINSIZE, CRC32_CASTAGNOLI);
                hash.move_to(sub);
                let mut last = start;
                for i in (start + WINSIZE)..end {
                    hash.update(stripe[i - WINSIZE], stripe[i]);
                    if hash.value() > maxhash && i - last >= MIN_CHUNK {
                        marks.push(ptr + i);
                        last = i;
                    }
                }
            }
            marks.sort_unstable();
            out.extend_from_slice(&marks);
        } else {
            // Sequential fallback for the last short stripe.
            if stripe.len() > WINSIZE {
                let mut hash = CrcRollingHash::new(WINSIZE, CRC32_CASTAGNOLI);
                hash.move_to(stripe);
                let mut last = 0usize;
                for i in WINSIZE..stripe.len() {
                    hash.update(stripe[i - WINSIZE], stripe[i]);
                    if hash.value() > maxhash && i - last >= MIN_CHUNK {
                        out.push(ptr + i);
                        last = i;
                    }
                }
            }
        }

        ptr = stripe_end;
    }

    // Always mark the end of the block.
    if out.last().copied() != Some(block.len()) {
        out.push(block.len());
    }
}

// --------------------------------------------------------------------------
// Boundary finding: ZPAQ CDC (order-1 context model)
// --------------------------------------------------------------------------

/// Find chunk boundaries in `block` for ZPAQ CDC (`-m2`).
///
/// Uses an order-1 context model: the rolling hash multiplier differs depending
/// on whether the current byte matches the predicted byte for that context.
pub fn find_chunks_zpaq(block: &[u8], avg_len: usize, out: &mut Vec<usize>) {
    let avg = avg_len.max(1) as u32;
    let maxhash: u32 = u32::MAX - u32::MAX / avg;

    let mut hash: u32 = 0;
    let mut c1: u8 = 0;
    let mut o1 = [0u8; 256];
    let mut lastp = 0usize;

    // Warm up by starting 8000 bytes before the block if there's preceding data.
    // Since we process whole blocks, we start from position 0 but model state
    // is accumulated. The C++ warms up from max(0, ptr-8000); for block-level
    // processing we start fresh (no warm-up penalty vs. C++ since there's no
    // cross-block state here).

    for (i, &c) in block.iter().enumerate() {
        let multiplier: u32 = if c != o1[c1 as usize] {
            271_828_182
        } else {
            314_159_265
        };
        hash = (hash.wrapping_add(c as u32).wrapping_add(1)).wrapping_mul(multiplier);
        o1[c1 as usize] = c;
        c1 = c;

        if hash > maxhash && i - lastp >= MIN_CHUNK {
            out.push(i);
            lastp = i;
            c1 = 0;
            hash = 0;
            o1 = [0u8; 256];
        }
    }

    if out.last().copied() != Some(block.len()) {
        out.push(block.len());
    }
}

// --------------------------------------------------------------------------
// Public entry point
// --------------------------------------------------------------------------

/// Find CDC chunk boundaries in `block`, appending them to `out`.
///
/// `avg_len` is the target average chunk length (= `config.chunk_len` for CDC modes).
/// The last entry appended is always `block.len()`.
pub fn find_chunks(mode: CdcMode, block: &[u8], avg_len: usize, out: &mut Vec<usize>) {
    match mode {
        CdcMode::Fast => find_chunks_fast(block, avg_len, out),
        CdcMode::Zpaq => find_chunks_zpaq(block, avg_len, out),
    }
}

// --------------------------------------------------------------------------
// CdcMatcher: chunk hash table and match finding
// --------------------------------------------------------------------------

/// Compute a 64-bit SipHash digest of a chunk.
fn chunk_digest(data: &[u8], seed: &[u8]) -> u64 {
    let k0 = if seed.len() >= 8 {
        u64::from_le_bytes(seed[..8].try_into().unwrap_or([0u8; 8]))
    } else {
        0xDEAD_BEEF_CAFE_BABEu64
    };
    let k1 = if seed.len() >= 16 {
        u64::from_le_bytes(seed[8..16].try_into().unwrap_or([0u8; 8]))
    } else {
        0x0102_0304_0506_0708u64
    };
    let mut h = SipHasher13::new_with_keys(k0, k1);
    h.write(data);
    h.finish()
}

/// CDC deduplicator: maps chunk digests to prior file positions.
///
/// Both `-m1` and `-m2` go through this structure. Chunks are compared by
/// digest; collision probability is negligible for typical archive sizes.
pub struct CdcMatcher {
    mode: CdcMode,
    avg_len: usize,
    min_match: usize,
    seed: Vec<u8>,
    /// digest → (file_offset, chunk_len)
    table: HashMap<u64, (u64, usize)>,
}

impl CdcMatcher {
    pub fn new(mode: CdcMode, avg_len: usize, min_match: usize, seed: Vec<u8>) -> Self {
        CdcMatcher {
            mode,
            avg_len,
            min_match: min_match.max(MIN_CHUNK),
            seed,
            table: HashMap::new(),
        }
    }

    /// Compress one block using CDC matching.
    ///
    /// Finds chunk boundaries, looks up each chunk in the table, records matches
    /// or advances the literal cursor. Newly-seen chunks are registered.
    pub fn compress_block(
        &mut self,
        block_start: u64,
        block: &[u8],
        sink: &mut MatchSink<'_>,
    ) -> Result<u64, SrepError> {
        let mut boundaries: Vec<usize> = Vec::new();
        find_chunks(self.mode, block, self.avg_len, &mut boundaries);

        let mut chunk_start = 0usize;
        let mut hits = 0u64;

        for &boundary in &boundaries {
            let chunk = &block[chunk_start..boundary];
            if chunk.len() < self.min_match {
                chunk_start = boundary;
                continue;
            }

            let digest = chunk_digest(chunk, &self.seed);

            if let Some(&(src_offset, src_len)) = self.table.get(&digest) {
                // Match found: confirm lengths match (digest collision guard).
                if src_len == chunk.len() {
                    let dest = chunk_start;
                    let len = chunk.len() as u32;
                    sink.record_match(dest, src_offset, len, block_start)?;
                    hits += 1;
                }
            }
            // Register this chunk (whether matched or not — latest wins).
            let file_offset = block_start + chunk_start as u64;
            self.table.insert(digest, (file_offset, chunk.len()));

            chunk_start = boundary;
        }

        Ok(hits)
    }
}

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------
