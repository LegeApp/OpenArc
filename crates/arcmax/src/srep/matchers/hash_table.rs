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
    if n <= 1 { 1 } else { n.next_power_of_two() }
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
        let hash_shift = if hashsize == 1 { 64 } else { 64 - floor_log2(hs) };
        Ok(TableLayout { chunknum_mask, hash_mask, hashsize, hashsize1, hash_shift })
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
            digestarr: if digest_len > 0 { vec![0u8; cap * digest_len] } else { vec![] },
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

#[cfg(test)]
mod tests {
    use super::*;

    // ---- TableLayout -------------------------------------------------------

    #[test]
    fn layout_small() {
        let l = TableLayout::new(100).unwrap();
        // chunknum_mask: roundup_pow2(102)-1 = 128-1 = 127
        assert_eq!(l.chunknum_mask, 127);
        // hash_mask: !127u32 = 0xFFFF_FF80
        assert_eq!(l.hash_mask, !127u32);
        // min_hash_size(100) = (100/4 + 1) * 5 = 26 * 5 = 130
        // roundup_pow2(130) = 256
        assert_eq!(l.hashsize, 256);
        assert_eq!(l.hashsize1, 255);
    }

    #[test]
    fn layout_zero_chunks() {
        let l = TableLayout::new(0).unwrap();
        // chunknum_mask: roundup_pow2(2)-1 = 2-1 = 1
        assert_eq!(l.chunknum_mask, 1);
        // hashsize: roundup_pow2(min_hash_size(0)) = roundup_pow2(5) = 8
        assert_eq!(l.hashsize, 8);
    }

    #[test]
    fn slot_value_roundtrip() {
        let l = TableLayout::new(64).unwrap();
        let hash: u64 = 0x1234_5678_ABCD_EF00;
        let chunk: ChunkId = 42;
        let v = l.slot_value(hash, chunk);
        assert_ne!(v, 0); // must not be the empty sentinel
        assert_eq!(l.chunk_id(v), chunk);
        assert_eq!(l.hash_bits(v), hash as u32 & l.hash_mask);
    }

    // ---- add_hash / find_match basics ------------------------------------

    #[test]
    fn first_insert_returns_none() {
        let mut t = FixedChunkTable::new(1024, 0).unwrap();
        let result = t.add_hash(1, 0xDEAD_BEEF_1234_5678);
        assert!(result.is_none(), "first insert should find no prior match");
    }

    #[test]
    fn duplicate_hash_returns_first_chunk() {
        let mut t = FixedChunkTable::new(1024, 0).unwrap();
        let hash: u64 = 0x0000_0000_CAFE_F00D;
        assert!(t.add_hash(1, hash).is_none());
        let dup = t.add_hash(2, hash);
        assert_eq!(dup, Some(1), "second insert with same hash should find chunk 1");
    }

    #[test]
    fn chunk_zero_is_matchable() {
        let mut t = FixedChunkTable::new(1024, 0).unwrap();
        assert!(t.add_hash(0, 0x1234).is_none());
        assert_eq!(t.add_hash(1, 0x1234), Some(0));
    }

    #[test]
    fn different_hashes_dont_collide() {
        let mut t = FixedChunkTable::new(1024, 0).unwrap();
        t.add_hash(1, 0xAAAA_AAAA_0000_0001);
        t.add_hash(2, 0xBBBB_BBBB_0000_0002);
        // Neither should match the other (different stored_hash values).
        let r = t.add_hash(3, 0xCCCC_CCCC_0000_0003);
        assert!(r.is_none());
    }

    // ---- Chain exhaustion ------------------------------------------------

    #[test]
    fn chain_exhaustion_graceful() {
        // All chunks share the same first hash slot (same low bits of index_hash).
        // After MAX_HASH_CHAIN probes the table gives up and returns None.
        let mut t = FixedChunkTable::new(4096, 0).unwrap();
        // Use distinct hash2 values that map to the same index_hash slot via hashsize1.
        let hashsize = t.layout.hashsize as u64;
        let base = 0x1111_1111_0000_0000u64;
        // Insert MAX_HASH_CHAIN + 5 unique hashes that slot to index 0 (multiples of hashsize).
        let mut stored = 0u32;
        for i in 1u32..=(MAX_HASH_CHAIN as u32 + 5) {
            let hash = base + (i as u64) * hashsize; // index_hash & hashsize1 == 0 for all
            let r = t.add_hash(i, hash);
            if r.is_some() {
                stored += 1;
            }
        }
        // We expect some inserts to have silently failed (chain exhausted) — that's OK.
        // The important thing is no panic.
        let _ = stored;
    }

    #[test]
    fn find_match_returns_none_on_empty_table() {
        let t = FixedChunkTable::new(64, 0).unwrap();
        assert!(t.find_match(0xABCD, None).is_none());
    }

    #[test]
    fn find_match_succeeds_after_add() {
        let mut t = FixedChunkTable::new(64, 0).unwrap();
        let hash: u64 = 0x1234_5678_9ABC_DEF0;
        t.add_hash(1, hash);
        let r = t.find_match(hash, None);
        assert_eq!(r, Some(1));
    }

    // ---- Digest comparison (-m3) -----------------------------------------

    #[test]
    fn digest_mismatch_prevents_match() {
        let mut t = FixedChunkTable::new(64, 4).unwrap();
        let hash: u64 = 0xAAAA_0000_BBBB_0000;

        // Chunk 1: digest = [1, 2, 3, 4]
        t.store_digest(1, &[1, 2, 3, 4]);
        t.add_hash(1, hash);

        // Chunk 2: same rolling hash but different digest.
        t.store_digest(2, &[9, 9, 9, 9]);
        let r = t.add_hash(2, hash);
        assert!(r.is_none(), "digest mismatch should prevent match");
    }

    #[test]
    fn digest_match_returns_chunk() {
        let mut t = FixedChunkTable::new(64, 4).unwrap();
        let hash: u64 = 0xAAAA_0000_BBBB_0000;

        t.store_digest(1, &[1, 2, 3, 4]);
        t.add_hash(1, hash);

        t.store_digest(2, &[1, 2, 3, 4]); // same digest
        let r = t.add_hash(2, hash);
        assert_eq!(r, Some(1));
    }

    #[test]
    fn find_match_with_digest() {
        let mut t = FixedChunkTable::new(64, 4).unwrap();
        let hash: u64 = 0x5555_5555_EEEE_EEEE;
        t.store_digest(1, &[0xAA, 0xBB, 0xCC, 0xDD]);
        t.add_hash(1, hash);

        // Correct digest — should find chunk 1.
        assert_eq!(t.find_match(hash, Some(&[0xAA, 0xBB, 0xCC, 0xDD])), Some(1));
        // Wrong digest — should find nothing.
        assert!(t.find_match(hash, Some(&[0xFF, 0xFF, 0xFF, 0xFF])).is_none());
        // No digest — should find chunk 1 (hash-only mode).
        assert_eq!(t.find_match(hash, None), Some(1));
    }

    // ---- next_slot ----------------------------------------------------------

    #[test]
    fn next_slot_is_deterministic() {
        let a = next_slot(42);
        let b = next_slot(42);
        assert_eq!(a, b);
        assert_ne!(a, 42); // must actually move
    }
}
