//! Fixed-chunk hash table — the core data structure for `-m3`/`-m4`/`-m5` matching.
//!
//! Ports `HashTable` from srep.cpp (lines 776–1084). Key design points:
//!
//! * Slot value 0 is the empty sentinel. Stored slot chunk fields are encoded
//!   as `chunk_id + 1`, so real chunk 0 remains matchable.
//! * Each slot packs high-order hash bits (for quick rejection) and a chunk ID
//!   into a single `u32`. The split is dynamic: `chunknum_mask` = next power-of-2
//!   covering `total_chunks + 2` minus 1; `hash_mask = !chunknum_mask`.
//! * `hashsize` = next power-of-2 covering `(total_chunks / 4 + 1) * 5` (load
//!   factor ≤ 80%).
//! * Probing: linear for the first 4 steps, then a multiplicative jump every 4th
//!   step. Chain limit = 12 iterations.
//! * `hasharr[chunk_id]` holds the high 32 bits of the 64-bit rolling hash for
//!   that chunk (quick second-level rejection before digest comparison).
//! * `digestarr` is an optional flat byte array of `digest_len` bytes per chunk,
//!   used by `-m3` to confirm matches without re-reading the source file.

use crate::srep::error::SrepError;
use crate::srep::types::ChunkId;

/// Maximum number of occupied slots to probe before giving up.
const MAX_HASH_CHAIN: usize = 12;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn roundup_pow2(n: u64) -> u64 {
    if n <= 1 {
        1
    } else {
        n.next_power_of_two()
    }
}

/// Integer floor-log2 of n (n must be ≥ 1).
fn floor_log2(n: u64) -> u32 {
    debug_assert!(n >= 1);
    63 - n.leading_zeros()
}

fn min_hash_size(n: u64) -> u64 {
    (n / 4 + 1) * 5
}

/// Pseudo-random jump to a new probe slot (C++ `next_hash_slot` macro).
/// All arithmetic wraps — this models C unsigned 64-bit overflow.
#[inline]
fn next_slot(h: u64) -> u64 {
    h.wrapping_mul(123_456_791)
        .wrapping_add(h >> 16)
        .wrapping_add(462_782_923)
}

// ---------------------------------------------------------------------------
// TableLayout
// ---------------------------------------------------------------------------

/// Packing/unpacking parameters computed once from `total_chunks`.
///
/// Mirrors the C++ fields `chunknum_mask`, `hash_mask`, `hashsize`, `hashsize1`,
/// `hash_shift` in `HashTable`.
#[derive(Debug, Clone, Copy)]
pub struct TableLayout {
    /// Low-bit mask selecting the chunk-ID field from a slot value.
    pub chunknum_mask: u32,
    /// High-bit mask selecting the hash-fingerprint field from a slot value.
    pub hash_mask: u32,
    /// Number of slots in `chunkarr` (always a power of two).
    pub hashsize: usize,
    /// `hashsize - 1` — used with `& hashsize1` to wrap probe positions.
    pub hashsize1: u64,
    /// `64 - floor_log2(hashsize)` — informational; not used in probing itself.
    pub hash_shift: u32,
}

impl TableLayout {
    /// Compute layout for at most `total_chunks` distinct chunks.
    pub fn new(total_chunks: u64) -> Result<Self, SrepError> {
        if total_chunks > u32::MAX as u64 - 2 {
            return Err(SrepError::Overflow("total_chunks exceeds ChunkId range"));
        }
        let chunknum_mask = roundup_pow2(total_chunks + 2) as u32 - 1;
        let hash_mask = !chunknum_mask;
        let hs = roundup_pow2(min_hash_size(total_chunks));
        if hs > usize::MAX as u64 {
            return Err(SrepError::Overflow("hashsize exceeds usize"));
        }
        let hashsize = hs as usize;
        let hashsize1 = hs - 1;
        let hash_shift = if hashsize == 1 {
            64
        } else {
            64 - floor_log2(hs)
        };
        Ok(TableLayout {
            chunknum_mask,
            hash_mask,
            hashsize,
            hashsize1,
            hash_shift,
        })
    }

    /// Pack hash fingerprint and chunk ID into one slot word.
    ///
    /// Only the low 32 bits of `index_hash` are used (matching C++ `Chunk(hash)`).
    #[inline]
    pub fn slot_value(&self, index_hash: u64, chunk: ChunkId) -> u32 {
        (index_hash as u32 & self.hash_mask) | chunk.wrapping_add(1)
    }

    /// Extract the hash-fingerprint bits from a slot word.
    #[inline]
    pub fn hash_bits(&self, value: u32) -> u32 {
        value & self.hash_mask
    }

    /// Extract the chunk ID from a slot word.
    #[inline]
    pub fn chunk_id(&self, value: u32) -> ChunkId {
        (value & self.chunknum_mask).wrapping_sub(1)
    }

    /// Map a probe value `h` to a slot index (wraps within `hashsize`).
    #[inline]
    pub fn hash_index(&self, h: u64) -> usize {
        (h & self.hashsize1) as usize
    }
}

// ---------------------------------------------------------------------------
// FixedChunkTable
// ---------------------------------------------------------------------------

/// Hash table for fixed-size (`L`-byte) chunk deduplication.
///
/// Stores up to `total_chunks` entries across `hashsize` slots using the
/// hybrid linear/multiplicative probe scheme from the original SREP.
pub struct FixedChunkTable {
    layout: TableLayout,
    /// Slot array — 0 means empty. Non-zero = `slot_value(hash_bits | chunk_id)`.
    chunkarr: Vec<u32>,
    /// Stored hash (high 32 bits of rolling hash) indexed by chunk ID.
    hasharr: Vec<u32>,
    /// Optional flat digest storage: `digest_len` bytes per chunk.
    digest_len: usize,
    digestarr: Vec<u8>,
}

impl FixedChunkTable {
    /// Allocate a table for at most `total_chunks` chunks.
    ///
    /// Pass `digest_len > 0` for `-m3` (precomputed digest comparison).
    /// Pass `digest_len = 0` for `-m4`/`-m5` (hash-only comparison).
    pub fn new(total_chunks: u64, digest_len: usize) -> Result<Self, SrepError> {
        let layout = TableLayout::new(total_chunks)?;
        let cap = total_chunks as usize + 2; // +2 mirrors C++ `total_chunks+2` guard
        Ok(FixedChunkTable {
            chunkarr: vec![0u32; layout.hashsize],
            hasharr: vec![0u32; cap],
            digest_len,
            digestarr: if digest_len > 0 {
                vec![0u8; cap * digest_len]
            } else {
                vec![]
            },
            layout,
        })
    }

    /// Store the precomputed digest for `chunk_id` (called during block prep for `-m3`).
    ///
    /// Must be called before `add_hash(chunk_id, ...)`.
    pub fn store_digest(&mut self, chunk_id: ChunkId, digest: &[u8]) {
        debug_assert_eq!(digest.len(), self.digest_len);
        debug_assert!(chunk_id as usize + 1 < self.hasharr.len());
        let start = chunk_id as usize * self.digest_len;
        self.digestarr[start..start + self.digest_len].copy_from_slice(digest);
    }

    fn digest_slice(&self, chunk_id: ChunkId) -> &[u8] {
        let start = chunk_id as usize * self.digest_len;
        &self.digestarr[start..start + self.digest_len]
    }

    /// Return the stored digest for `chunk_id`, or `None` if out of range or
    /// digest storage is disabled.
    pub fn digest_for_chunk(&self, chunk_id: ChunkId) -> Option<&[u8]> {
        if self.digest_len == 0 {
            return None;
        }
        let start = chunk_id as usize * self.digest_len;
        let end = start + self.digest_len;
        if end <= self.digestarr.len() {
            Some(&self.digestarr[start..end])
        } else {
            None
        }
    }

    /// Insert `chunk_id` into the table using rolling hash `hash2`.
    ///
    /// Returns the ID of a previously stored chunk whose digest (or stored hash)
    /// matches, or `None` if no duplicate was found.
    ///
    /// `chunk_id` may be 0. The on-slot representation stores `chunk_id + 1`
    /// so 0 remains reserved for empty slots.
    pub fn add_hash(&mut self, chunk_id: ChunkId, hash2: u64) -> Option<ChunkId> {
        let index_hash: u64 = hash2; // index_hash(hash2) = hash2
        let stored_hash: u32 = (hash2 >> 32) as u32; // stored_hash(hash2) = hash2 >> 32

        // Store this chunk's quick-reject hash.
        self.hasharr[chunk_id as usize] = stored_hash;

        let layout = self.layout;
        // The fingerprint we'll look for in existing slots.
        let target_fp = layout.hash_bits(layout.slot_value(index_hash, 0));

        let mut h = index_hash;
        let mut limit = MAX_HASH_CHAIN;
        let mut found: Option<ChunkId> = None;

        loop {
            let slot_idx = layout.hash_index(h);
            let value = self.chunkarr[slot_idx];

            if value == 0 {
                break; // empty slot — stop
            }
            limit -= 1;
            if limit == 0 {
                break; // chain exhausted
            }

            if layout.hash_bits(value) == target_fp {
                let old_id = layout.chunk_id(value);
                // Quick second-level check: stored hash comparison.
                if self.hasharr[old_id as usize] == stored_hash {
                    // Full check: digest comparison for -m3, or assume match for -m4/-m5.
                    if self.digest_len == 0
                        || self.digest_slice(old_id) == self.digest_slice(chunk_id)
                    {
                        found = Some(old_id);
                        break;
                    }
                }
            }

            h = h.wrapping_add(1);
            if (limit & 3) == 0 {
                h = next_slot(h);
            }
        }

        // Write this chunk into the slot where the probe stopped.
        let slot_idx = layout.hash_index(h);
        self.chunkarr[slot_idx] = layout.slot_value(index_hash, chunk_id);

        found
    }

    /// Scan the table for a chunk matching `hash2`.
    ///
    /// When `cur_digest` is `Some`, confirms candidates by digest comparison.
    /// When `None`, the first stored-hash-matching candidate is returned (for -m4/-m5).
    ///
    /// Does not modify the table.
    pub fn find_match(&self, hash2: u64, cur_digest: Option<&[u8]>) -> Option<ChunkId> {
        let index_hash: u64 = hash2;
        let stored_hash: u32 = (hash2 >> 32) as u32;
        let layout = self.layout;
        let target_fp = layout.hash_bits(layout.slot_value(index_hash, 0));

        let mut h = index_hash;
        let mut limit = MAX_HASH_CHAIN;

        loop {
            let slot_idx = layout.hash_index(h);
            let value = self.chunkarr[slot_idx];

            if value == 0 {
                return None;
            }
            limit -= 1;
            if limit == 0 {
                return None;
            }

            if layout.hash_bits(value) == target_fp {
                let old_id = layout.chunk_id(value);
                if self.hasharr[old_id as usize] == stored_hash {
                    match cur_digest {
                        None => return Some(old_id),
                        Some(dig) if self.digest_slice(old_id) == dig => return Some(old_id),
                        _ => {}
                    }
                }
            }

            h = h.wrapping_add(1);
            if (limit & 3) == 0 {
                h = next_slot(h);
            }
        }
    }

    /// Prefetch the first slot for `hash2` — call one iteration before the matching
    /// call to pipeline memory latency.
    #[inline]
    pub fn prefetch(&self, hash2: u64) {
        let slot_idx = self.layout.hash_index(hash2);
        // Safe to ignore the result — prefetch is a hint only.
        let _ = self.chunkarr.get(slot_idx);
    }

    /// Layout parameters (public for Stage G integration).
    pub fn layout(&self) -> TableLayout {
        self.layout
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
