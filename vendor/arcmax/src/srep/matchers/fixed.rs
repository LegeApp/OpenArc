//! Fixed-block matcher for `-m3`, `-m4`, and `-m5`.
//!
//! Implements the hot inner loop from `srep.cpp` `compress<ACCELERATOR>` (lines
//! ~2560–2850): for every L-byte chunk in the current block, compute a rolling
//! polynomial hash, probe the `FixedChunkTable`, and record any confirmed match
//! in the provided `MatchSink`.
//!
//! ## Chunk ID derivation
//!
//! chunk_id is always computed as `(block_start + i) / L`. Chunk 0 is a valid
//! source; `FixedChunkTable` keeps slot value 0 reserved by storing chunk ids
//! as `chunk_id + 1` in the packed slot word.
//!
//! ## block_start alignment
//!
//! `block_start` MUST be a multiple of `chunk_len`. The encoder loop ensures
//! this by rounding `block_size` down to a multiple of L before reading blocks.
//! A debug_assert enforces this invariant at the top of `compress_block`.

use crate::srep::compress::block_sink::MatchSink;
use crate::srep::config::{HashKind, Method};
use crate::srep::error::SrepError;
use crate::srep::hash::digest::compute_block_digest;
use crate::srep::hash::rolling::{PolyRolling64, PRIME_M3};
use crate::srep::matchers::{FixedChunkTable, SliceHash};
use crate::srep::types::{ChunkId, Offset};
use std::io::{Read, Seek, SeekFrom};

/// All state for a fixed-block (-m3/-m4/-m5) compression pass.
///
/// Allocated once per compression run; `compress_block` is called for each input block.
pub struct FixedMatcher {
    table: FixedChunkTable,
    slice_hash: SliceHash,
    hasher: PolyRolling64,
    chunk_len: usize,
    min_match: usize,
    method: Method,
    /// Block-integrity hash kind (for per-chunk digest storage in -m3).
    hash_kind: HashKind,
    /// Seed for keyed hashes (SipHash etc.).
    seed: Vec<u8>,
    /// Digest length in bytes (0 when HashKind::None).
    digest_len: usize,
}

impl FixedMatcher {
    /// Construct a matcher for a file of `file_size` bytes.
    ///
    /// * `chunk_len` (`L`) — must be a power of two and at least 8
    /// * `min_match` — minimum useful match length in bytes
    /// * `method` — `M3`, `M4`, or `M5`
    /// * `hash_kind` / `seed` — block-integrity hash (drives chunk digest storage for -m3)
    pub fn new(
        file_size: u64,
        chunk_len: usize,
        min_match: usize,
        method: Method,
        hash_kind: HashKind,
        seed: Vec<u8>,
        io_accelerator: i32,
    ) -> Result<Self, SrepError> {
        let total_chunks = file_size / chunk_len as u64 + 1;
        let digest_len = if method == Method::M3 {
            hash_kind.digest_len()
        } else {
            0
        };
        let table = FixedChunkTable::new(total_chunks, digest_len)?;

        let slice_hash = SliceHash::new(total_chunks, chunk_len, min_match, io_accelerator);

        let hasher = PolyRolling64::new(chunk_len, PRIME_M3);

        Ok(FixedMatcher {
            table,
            slice_hash,
            hasher,
            chunk_len,
            min_match,
            method,
            hash_kind,
            seed,
            digest_len,
        })
    }

    /// Prepare block-level state: store chunk digests (-m3) and slice-hash entries.
    ///
    /// Must be called before `compress_block` for the same block.
    pub fn prepare_block(&mut self, block_start: u64, block: &[u8]) -> Result<(), SrepError> {
        let l = self.chunk_len;

        // Store per-chunk digests for -m3 match confirmation.
        // Digest computation is independent per chunk → parallelize with rayon.
        if self.method == Method::M3 && self.digest_len > 0 {
            use rayon::prelude::*;

            // Gather (chunk_id, byte_offset) pairs for chunks that need a digest.
            let positions: Vec<(u32, usize)> = (0..block.len())
                .step_by(l)
                .filter_map(|pos| {
                    if pos + l > block.len() {
                        return None;
                    }
                    let chunk_id = ((block_start + pos as u64) / l as u64) as u32;
                    if chunk_id != 0 {
                        Some((chunk_id, pos))
                    } else {
                        None
                    }
                })
                .collect();

            // Compute all digests in parallel.
            let digests: Vec<(u32, Result<Vec<u8>, SrepError>)> = positions
                .into_par_iter()
                .map(|(chunk_id, pos)| {
                    let digest =
                        compute_block_digest(self.hash_kind, &self.seed, &block[pos..pos + l]);
                    (chunk_id, digest)
                })
                .collect();

            // Write results serially (each chunk_id is unique — no contention).
            for (chunk_id, digest) in digests {
                self.table.store_digest(chunk_id, &digest?);
            }
        }

        // Fill slice-hash bloom filter entries.
        self.slice_hash.prepare_buffer(block_start, block);

        Ok(())
    }

    /// Find and emit all matches in `block`, writing stats/literals into `sink`.
    ///
    /// `block_start` is the absolute file offset of `block[0]`. It MUST be a
    /// multiple of `chunk_len`.
    pub fn compress_block(
        &mut self,
        block_start: u64,
        block: &[u8],
        sink: &mut MatchSink<'_>,
    ) -> Result<u64, SrepError> {
        self.compress_block_with_source::<std::io::Empty>(block_start, block, sink, None)
    }

    /// Variant used by `-m4/-m5` to confirm cross-block candidates by reading
    /// the original input at the source offset.
    pub fn compress_block_with_source<R>(
        &mut self,
        block_start: u64,
        block: &[u8],
        sink: &mut MatchSink<'_>,
        mut source: Option<&mut R>,
    ) -> Result<u64, SrepError>
    where
        R: Read + Seek,
    {
        debug_assert!(
            block_start % self.chunk_len as u64 == 0,
            "block_start not L-aligned"
        );

        let l = self.chunk_len;
        let mut matches_found: u64 = 0;
        let mut next_emit = 0usize; // first position not yet claimed by a match

        // Only process complete L-aligned chunks.
        let usable = (block.len() / l) * l;

        let mut i = 0usize;
        while i < usable {
            // Compute chunk_id from absolute position — never use a counter.
            let chunk_id: ChunkId = ((block_start + i as u64) / l as u64) as u32;

            // Compute rolling hash for block[i..i+l].
            self.hasher.move_to(&block[i..i + l]);
            let hash2: u64 = self.hasher.value();

            // SliceHash pre-filter: skip positions unlikely to match.
            if !self.slice_hash.check(chunk_id, block, i) {
                i += l;
                continue;
            }

            // Look for a prior chunk with the same rolling hash.
            // For -m3: store_digest was called in prepare_block, so add_hash uses digest comparison.
            let old_id = self.table.add_hash(chunk_id, hash2);

            if let Some(old_chunk) = old_id {
                let old_offset: Offset = old_chunk as u64 * l as u64;
                let dest_offset = block_start + i as u64;
                if old_offset >= dest_offset {
                    i += l;
                    continue;
                }

                // For same-block matches, byte-verify to guard against hash collisions.
                // Cross-block matches trust the stored digest (or stored hash for -m4/-m5).
                let mut same_block_old_pos = None;
                if old_offset >= block_start {
                    let old_pos = (old_offset - block_start) as usize;
                    if old_pos + l > block.len() || block[i..i + l] != block[old_pos..old_pos + l] {
                        i += l;
                        continue;
                    }
                    same_block_old_pos = Some(old_pos);
                } else if matches!(self.method, Method::M4 | Method::M5) {
                    let Some(src) = source.as_deref_mut() else {
                        i += l;
                        continue;
                    };
                    if !verify_source_seed(src, old_offset, &block[i..i + l])? {
                        i += l;
                        continue;
                    }
                }

                // Extend same-block matches forward within the current block.
                // Cross-block byte extension needs random-access input and remains
                // a later -m4/-m5 enhancement; the confirmed seed stays valid.
                let mut match_len = l + same_block_old_pos
                    .map(|old_pos| common_prefix_overlap(block, old_pos + l, i + l))
                    .unwrap_or_else(|| {
                        if self.method == Method::M3 {
                            self.digest_chunk_extension(old_chunk, chunk_id, block.len() - i)
                        } else if matches!(self.method, Method::M4 | Method::M5) {
                            source
                                .as_deref_mut()
                                .map(|src| {
                                    source_common_prefix(
                                        src,
                                        old_offset + l as u64,
                                        &block[i + l..],
                                    )
                                    .unwrap_or(0)
                                })
                                .unwrap_or(0)
                        } else {
                            0
                        }
                    });

                if sink.round_matches() {
                    match_len -= match_len % l;
                }

                if match_len >= self.min_match && i >= next_emit {
                    sink.record_match(i, old_offset, match_len as u32, block_start)?;
                    next_emit = i + match_len;
                    matches_found += 1;
                }
            }

            i += l;
        }

        Ok(matches_found)
    }
}

impl FixedMatcher {
    /// Extend an `-m3` cross-block match in whole chunks using precomputed
    /// per-chunk digests. The seed chunk was already confirmed by `add_hash`,
    /// so this returns only extra bytes after that first chunk.
    fn digest_chunk_extension(
        &self,
        old_chunk: ChunkId,
        cur_chunk: ChunkId,
        max_len: usize,
    ) -> usize {
        if self.digest_len == 0 || max_len <= self.chunk_len {
            return 0;
        }

        let max_extra_chunks = max_len / self.chunk_len - 1;
        let mut extra_chunks = 0usize;
        for step in 1..=max_extra_chunks {
            let Some(old_digest) = self.table.digest_for_chunk(old_chunk + step as u32) else {
                break;
            };
            let Some(cur_digest) = self.table.digest_for_chunk(cur_chunk + step as u32) else {
                break;
            };
            if old_digest != cur_digest {
                break;
            }
            extra_chunks += 1;
        }

        extra_chunks * self.chunk_len
    }
}

/// Count equal bytes at two positions in the same block, with LZ-style overlap
/// semantics. This is intentionally bytewise and safe; it mirrors the decoder's
/// forward copy behavior for same-block repeated runs.
#[inline]
fn common_prefix_overlap(block: &[u8], mut old_pos: usize, mut new_pos: usize) -> usize {
    let mut len = 0usize;
    while new_pos < block.len() && block[old_pos] == block[new_pos] {
        len += 1;
        old_pos += 1;
        new_pos += 1;
    }
    len
}

fn verify_source_seed<R: Read + Seek>(
    source: &mut R,
    offset: u64,
    expected: &[u8],
) -> Result<bool, SrepError> {
    let mut buf = vec![0u8; expected.len()];
    source
        .seek(SeekFrom::Start(offset))
        .map_err(SrepError::Io)?;
    source.read_exact(&mut buf).map_err(SrepError::Io)?;
    Ok(buf == expected)
}

fn source_common_prefix<R: Read + Seek>(
    source: &mut R,
    offset: u64,
    expected: &[u8],
) -> Result<usize, SrepError> {
    if expected.is_empty() {
        return Ok(0);
    }
    let mut buf = vec![0u8; expected.len()];
    source
        .seek(SeekFrom::Start(offset))
        .map_err(SrepError::Io)?;
    let n = source.read(&mut buf).map_err(SrepError::Io)?;
    Ok(buf[..n]
        .iter()
        .zip(expected.iter())
        .take_while(|(a, b)| a == b)
        .count())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
