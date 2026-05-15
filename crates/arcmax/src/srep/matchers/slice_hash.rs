//! `SliceHash` — bloom-like filter for eliminating false match candidates.
//!
//! Ports `SliceHash` from srep.cpp (lines 681–773). Purpose: before doing an
//! expensive byte-level match extension, verify that the slices *around* the
//! candidate match position have consistent 4-bit hashes — if they don't, the
//! match cannot extend to `min_match` bytes and we skip it (~90% false-positive
//! elimination in `-m5` mode).
//!
//! ## Memory layout
//!
//! One `u32` entry per `chunk_len`-byte chunk in the file. The entry packs 8
//! 4-bit sub-hashes (one per `slice_size = chunk_len / 8` bytes), bit-packed
//! from LSB to MSB.
//!
//! ## Disabled state
//!
//! When `check_slices ≤ 0` (i.e., the filter wouldn't reject anything), the
//! filter is allocated with zero entries and `check()` always returns `true`.
//! This happens when `io_accelerator < 0` or `min_match ≤ chunk_len`.

/// 4 bits per sub-slice, 8 sub-slices per chunk entry → one `u32` per chunk.
const BITS: usize = 4;
const ONES: u32 = (1 << BITS) - 1; // 0x0F
const SLICES_PER_ENTRY: usize = u32::BITS as usize / BITS; // 8

/// Compute a 4-bit hash of `data`. Ports `SliceHash::hash()`.
fn slice_hash4(data: &[u8]) -> u32 {
    let mut h: u32 = 111_222_341;
    for &b in data {
        h = h.wrapping_mul(123_456_791).wrapping_add(b as u32);
    }
    (h.wrapping_mul(123_456_791)) >> (u32::BITS as usize - BITS) // top 4 bits
}

/// Per-chunk bloom-like slice filter.
pub struct SliceHash {
    /// One packed entry per chunk. Empty when the filter is disabled.
    entries: Vec<u32>,
    /// Bytes per sub-slice: `chunk_len / SLICES_PER_ENTRY`.
    slice_size: usize,
    /// How many consecutive matching sub-slices are required to pass.
    check_slices: usize,
    /// `chunk_len` in bytes.
    chunk_len: usize,
}

impl SliceHash {
    /// Create a filter for `total_chunks` chunks.
    ///
    /// * `chunk_len` — `L` (must be divisible by `SLICES_PER_ENTRY = 8`)
    /// * `min_match` — minimum required match length in bytes
    /// * `io_accelerator` — higher values reduce `check_slices` (≥ 0 normally;
    ///   negative disables the filter entirely)
    pub fn new(
        total_chunks: u64,
        chunk_len: usize,
        min_match: usize,
        io_accelerator: i32,
    ) -> Self {
        debug_assert!(chunk_len >= SLICES_PER_ENTRY, "chunk_len must be >= 8");
        debug_assert_eq!(chunk_len % SLICES_PER_ENTRY, 0, "chunk_len must be divisible by 8");

        let slice_size = chunk_len / SLICES_PER_ENTRY;
        let raw_check_slices = if min_match > chunk_len {
            ((min_match - chunk_len) / slice_size) as i32 - io_accelerator
        } else {
            0
        };

        let effective_check = raw_check_slices.max(0) as usize;
        let entries = if effective_check == 0 || io_accelerator < 0 {
            vec![] // disabled
        } else {
            vec![0u32; total_chunks as usize]
        };

        SliceHash { entries, slice_size, check_slices: effective_check, chunk_len }
    }

    /// Returns `true` when the filter is disabled (always passes).
    pub fn is_disabled(&self) -> bool {
        self.entries.is_empty()
    }

    /// Compute and store the slice-hash entry for every `chunk_len`-byte chunk
    /// that starts within `block[..size]`.
    ///
    /// `block_offset` is the absolute file offset of `block[0]`.
    pub fn prepare_buffer(&mut self, block_offset: u64, block: &[u8]) {
        if self.entries.is_empty() {
            return;
        }
        let mut chunk_id = (block_offset / self.chunk_len as u64) as usize;
        let mut pos = 0usize;
        while pos + self.chunk_len <= block.len() {
            let mut entry: u32 = 0;
            for i in 0..SLICES_PER_ENTRY {
                let sl = &block[pos + i * self.slice_size..pos + (i + 1) * self.slice_size];
                entry |= slice_hash4(sl) << (i * BITS);
            }
            if chunk_id < self.entries.len() {
                self.entries[chunk_id] = entry;
            }
            chunk_id += 1;
            pos += self.chunk_len;
        }
    }

    /// Return `true` if the match candidate at `chunk_id` *might* extend to
    /// `min_match` bytes; `false` when it is provably impossible.
    ///
    /// Arguments (mirroring C++ `SliceHash::check`):
    ///
    /// * `chunk_id` — the OLD chunk (candidate match source) in the global file
    /// * `block` — the current input block buffer
    /// * `pos_in_block` — byte offset `i` of the current position within `block`
    ///
    /// Returns `true` (passes) when:
    /// - The filter is disabled, **or**
    /// - There are not enough bytes around `pos_in_block` to check, **or**
    /// - At least `check_slices` consecutive sub-slices match on forward + backward
    ///   sides combined.
    pub fn check(&self, chunk_id: u32, block: &[u8], pos_in_block: usize) -> bool {
        if self.entries.is_empty() {
            return true;
        }
        let block_size = block.len();
        let l = self.chunk_len;

        // Boundary guard: need `L` bytes before and `2L` bytes after `pos_in_block`.
        if pos_in_block < l || block_size.saturating_sub(pos_in_block) < 2 * l {
            return true;
        }

        let fwd_id = chunk_id as usize + 1;
        // chunk_id == 0 would underflow; in practice, 0 is never stored (NOT_FOUND sentinel).
        if chunk_id == 0 {
            return true;
        }
        let bwd_id = chunk_id as usize - 1;

        // Forward check: slices starting at pos_in_block + L.
        let fwd_entry = if fwd_id < self.entries.len() { self.entries[fwd_id] } else { return true };
        let fwd_base = pos_in_block + l;
        let mut j = 0usize;
        loop {
            if j == self.check_slices {
                return true;
            }
            let slice_start = fwd_base + j * self.slice_size;
            let slice_end = slice_start + self.slice_size;
            if slice_end > block.len() {
                return true; // not enough data
            }
            let expected = (fwd_entry >> (j * BITS)) & ONES;
            if slice_hash4(&block[slice_start..slice_end]) != expected {
                break;
            }
            j += 1;
        }

        // Backward check: slices ending at pos_in_block, going backward.
        let bwd_entry = if bwd_id < self.entries.len() { self.entries[bwd_id] } else { return true };
        let mut k = 0usize;
        loop {
            if j + k == self.check_slices {
                return true;
            }
            // Sub-slice index from the END of bwd_entry: SLICES_PER_ENTRY-1, SLICES_PER_ENTRY-2, ...
            let slice_idx_in_entry = SLICES_PER_ENTRY - (k + 1);
            let slice_end = pos_in_block.saturating_sub(k * self.slice_size);
            let slice_start = slice_end.saturating_sub(self.slice_size);
            if slice_start == slice_end {
                return true; // underflow guard
            }
            let expected = (bwd_entry >> (slice_idx_in_entry * BITS)) & ONES;
            if slice_hash4(&block[slice_start..slice_end]) != expected {
                break;
            }
            k += 1;
        }

        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sh(total_chunks: u64, chunk_len: usize, min_match: usize, accel: i32) -> SliceHash {
        SliceHash::new(total_chunks, chunk_len, min_match, accel)
    }

    #[test]
    fn disabled_when_min_match_le_chunk_len() {
        let sh = make_sh(1000, 32, 32, 0); // min_match == chunk_len → check_slices = 0
        assert!(sh.is_disabled());
    }

    #[test]
    fn disabled_when_negative_accelerator() {
        let sh = make_sh(1000, 32, 128, -1);
        assert!(sh.is_disabled());
    }

    #[test]
    fn disabled_check_always_passes() {
        let sh = make_sh(100, 32, 32, 0);
        let block = vec![0u8; 256];
        assert!(sh.check(1, &block, 32));
        assert!(sh.check(5, &block, 100));
    }

    #[test]
    fn prepare_and_check_identical_data_passes() {
        // chunk_len=8, min_match=9 → slice_size=1, check_slices=1.
        // Use REPEATING data (period=8) so old chunk and new position have identical content
        // and surrounding slices also match.
        let total_chunks: u64 = 8;
        let chunk_len = 8;
        let mut sh = SliceHash::new(total_chunks, chunk_len, 9, 0);
        assert!(!sh.is_disabled(), "check_slices should be 1");

        // Build repeating data: [0,1,2,3,4,5,6,7, 0,1,2,3,4,5,6,7, ...]
        let data: Vec<u8> = (0u8..64).map(|i| i % 8).collect();
        sh.prepare_buffer(0, &data);

        // Old match: chunk_id=1 (bytes 8..16 = [0,1,2,3,4,5,6,7]).
        // Current position: pos_in_block=16 (bytes 16..24 = [0,1,2,3,4,5,6,7]).
        // Forward: entries[2] covers old bytes 16..24 = same [0..7] pattern.
        //   block[24..25] = block[24%8=0] → same hash.
        // Backward: entries[0] covers old bytes 0..8 = same [0..7] pattern.
        //   block[15..16] = 7, entries[0]'s last slice = hash(data[7..8]) = hash(7).
        // All slices match → check returns true.
        assert!(sh.check(1, &data, 16));
    }

    #[test]
    fn prepare_and_check_different_data_fails() {
        let total_chunks: u64 = 8;
        let chunk_len = 8;
        let min_match = 10; // check_slices = (10-8)/1 = 2
        let mut sh = SliceHash::new(total_chunks, chunk_len, min_match, 0);
        if sh.is_disabled() {
            return; // skip if disabled
        }

        // Prepare with a block of zeros.
        let old_data = vec![0u8; (total_chunks as usize) * chunk_len];
        sh.prepare_buffer(0, &old_data);

        // Now check against a block of 0xFF bytes.
        let new_data = vec![0xFFu8; (total_chunks as usize) * chunk_len];
        // Check chunk 2 as old match (enough boundary space at pos=16).
        let result = sh.check(2, &new_data, 16);
        // The hashes stored for chunk 3 (zero data) won't match 0xFF slices → false.
        assert!(!result);
    }

    #[test]
    fn slice_hash4_stable() {
        let a = slice_hash4(b"hello");
        let b = slice_hash4(b"hello");
        assert_eq!(a, b);
        assert_ne!(a, slice_hash4(b"world"));
        // Must fit in 4 bits.
        assert!(a <= ONES);
        assert!(slice_hash4(b"world") <= ONES);
    }

    #[test]
    fn boundary_guard_passes_at_edges() {
        let mut sh = SliceHash::new(16, 8, 16, 0);
        if sh.is_disabled() { return; }
        let block = vec![0u8; 64];
        sh.prepare_buffer(0, &block);
        // pos_in_block < chunk_len → boundary guard → true
        assert!(sh.check(0, &block, 4));
        // block_size - pos_in_block < 2*L → boundary guard → true
        assert!(sh.check(5, &block, 56));
    }
}
