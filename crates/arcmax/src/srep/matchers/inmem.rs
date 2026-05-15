//! In-memory sliding dictionary matcher for SREP `-m0`.
//!
//! This is the first safe scalar port of `compress_inmem.cpp`: it keeps a
//! bounded ring dictionary of already emitted bytes, maps rolling hashes to
//! logical offsets, verifies candidate bytes before recording a match, and
//! rejects stale hash entries through `RingDict::physical_index`.

use crate::srep::compress::block_sink::MatchSink;
use crate::srep::error::SrepError;
use crate::srep::hash::rolling::{PolyRolling64, PRIME_M3};

#[derive(Debug, Clone, Copy)]
pub struct PreparedEntry {
    pub hash_index: usize,
    pub offset: usize,
}

#[derive(Debug, Default, Clone)]
pub struct PreparedBlock {
    pub entries: Vec<PreparedEntry>,
}

#[derive(Debug, Clone, Copy)]
pub enum RingWindow<'a> {
    Contiguous(&'a [u8]),
    Split(&'a [u8], &'a [u8]),
}

impl<'a> RingWindow<'a> {
    pub fn len(self) -> usize {
        match self {
            RingWindow::Contiguous(a) => a.len(),
            RingWindow::Split(a, b) => a.len() + b.len(),
        }
    }
}

/// Fixed-size logical ring dictionary.
///
/// `logical_start` is the absolute offset represented by `buf[0]` when the
/// ring has not wrapped. After wrap, `physical_index` maps absolute positions
/// through `write_pos`.
#[derive(Debug, Clone)]
pub struct RingDict {
    buf: Vec<u8>,
    logical_start: u64,
    len: usize,
    write_pos: usize,
}

impl RingDict {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0; capacity],
            logical_start: 0,
            len: 0,
            write_pos: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    pub fn logical_start(&self) -> u64 {
        self.logical_start
    }

    pub fn logical_end(&self) -> u64 {
        self.logical_start + self.len as u64
    }

    pub fn physical_index(&self, logical_pos: u64) -> Option<usize> {
        if self.capacity() == 0 {
            return None;
        }
        if logical_pos < self.logical_start || logical_pos >= self.logical_end() {
            return None;
        }

        let rel = (logical_pos - self.logical_start) as usize;
        if self.len < self.capacity() {
            Some(rel)
        } else {
            Some((self.write_pos + rel) % self.capacity())
        }
    }

    pub fn byte_at(&self, logical_pos: u64) -> Option<u8> {
        self.physical_index(logical_pos).map(|idx| self.buf[idx])
    }

    pub fn window(&self, logical_pos: u64, len: usize) -> Option<RingWindow<'_>> {
        if len == 0 {
            return Some(RingWindow::Contiguous(&[]));
        }
        let start = self.physical_index(logical_pos)?;
        let end_pos = logical_pos.checked_add(len as u64)?.checked_sub(1)?;
        self.physical_index(end_pos)?;

        if start + len <= self.capacity() {
            Some(RingWindow::Contiguous(&self.buf[start..start + len]))
        } else {
            let first = &self.buf[start..];
            let second_len = len - first.len();
            Some(RingWindow::Split(first, &self.buf[..second_len]))
        }
    }

    pub fn push_block(&mut self, block: &[u8]) {
        if self.capacity() == 0 || block.is_empty() {
            return;
        }

        for &b in block {
            if self.len == self.capacity() {
                self.logical_start += 1;
            } else {
                self.len += 1;
            }
            self.buf[self.write_pos] = b;
            self.write_pos = (self.write_pos + 1) % self.capacity();
        }
    }
}

pub struct InMemDeduplicator {
    chunk_len: usize,
    min_match: usize,
    hash_mask: usize,
    hash_arr: Vec<Option<u64>>,
    dict: RingDict,
}

impl InMemDeduplicator {
    pub fn new(chunk_len: usize, min_match: usize, dict_size: usize) -> Result<Self, SrepError> {
        if chunk_len == 0 {
            return Err(SrepError::Format("chunk_len must be non-zero"));
        }
        if min_match < chunk_len {
            return Err(SrepError::Format("min_match must be at least chunk_len"));
        }
        if dict_size == 0 {
            return Err(SrepError::Format("dict_size must be non-zero for -m0"));
        }

        let slots = (dict_size / chunk_len).max(1024).next_power_of_two();
        Ok(Self {
            chunk_len,
            min_match,
            hash_mask: slots - 1,
            hash_arr: vec![None; slots],
            dict: RingDict::new(dict_size),
        })
    }

    pub fn prepare_block(&self, block: &[u8]) -> PreparedBlock {
        let mut entries = Vec::new();
        let full_regions = block.len() / self.chunk_len;
        if full_regions <= 1 {
            return PreparedBlock { entries };
        }

        let mut hasher = PolyRolling64::new(self.chunk_len, PRIME_M3);
        hasher.move_to(block);

        let mut pos = 0usize;
        for region in 0..full_regions - 1 {
            let region_start = region * self.chunk_len;
            debug_assert_eq!(pos, region_start);
            let mut max_hash = hasher.value();
            let mut max_offset = region_start;

            for step in 0..self.chunk_len {
                if hasher.value() > max_hash {
                    max_hash = hasher.value();
                    max_offset = region_start + step;
                }

                if pos + self.chunk_len >= block.len() {
                    break;
                }
                hasher.update(block[pos], block[pos + self.chunk_len]);
                pos += 1;
            }

            entries.push(PreparedEntry {
                hash_index: self.hash_index(max_hash),
                offset: max_offset,
            });
        }

        PreparedBlock { entries }
    }

    pub fn compress_block(
        &mut self,
        block_start: u64,
        block: &[u8],
        prepared: &PreparedBlock,
        sink: &mut MatchSink<'_>,
    ) -> Result<u64, SrepError> {
        let mut matches = 0u64;
        let mut next_emit = 0usize;

        for entry in &prepared.entries {
            if entry.offset < next_emit {
                continue;
            }

            let Some(src) = self.hash_arr[entry.hash_index] else {
                continue;
            };

            if self.dict.physical_index(src).is_none() {
                continue;
            }

            let back = self.match_back(src, block, entry.offset, next_emit);
            let match_src = src - back as u64;
            let match_dest = entry.offset - back;
            let len = back + self.match_forward(src, block, entry.offset);
            if len >= self.min_match {
                sink.record_match(match_dest, match_src, len as u32, block_start)?;
                next_emit = match_dest + len;
                matches += 1;
            }
        }

        Ok(matches)
    }

    /// Commit the current block into the dictionary after it has been encoded.
    pub fn commit_block(&mut self, block_start: u64, block: &[u8], prepared: &PreparedBlock) {
        for entry in &prepared.entries {
            let logical = block_start + entry.offset as u64;
            self.hash_arr[entry.hash_index] = Some(logical);
        }
        self.dict.push_block(block);
    }

    pub fn dictionary_range(&self) -> std::ops::Range<u64> {
        self.dict.logical_start()..self.dict.logical_end()
    }

    fn hash_index(&self, hash: u64) -> usize {
        hash as usize & self.hash_mask
    }

    fn match_forward(&self, src: u64, block: &[u8], dest: usize) -> usize {
        let mut len = 0usize;
        while dest + len < block.len() {
            match self.dict.byte_at(src + len as u64) {
                Some(b) if b == block[dest + len] => len += 1,
                _ => break,
            }
        }
        len
    }

    fn match_back(&self, src: u64, block: &[u8], dest: usize, lower_dest: usize) -> usize {
        let mut len = 0usize;
        while dest > lower_dest + len && src > len as u64 {
            let src_pos = src - len as u64 - 1;
            let dest_pos = dest - len - 1;
            match self.dict.byte_at(src_pos) {
                Some(b) if b == block[dest_pos] => len += 1,
                _ => break,
            }
        }
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::srep::format::StatCodec;

    fn sink(block: &[u8]) -> MatchSink<'_> {
        MatchSink::new(
            block,
            StatCodec {
                round_matches: false,
                chunk_len: 16,
            },
        )
    }

    #[test]
    fn ring_rejects_stale_positions() {
        let mut ring = RingDict::new(4);
        ring.push_block(b"abcd");
        assert_eq!(ring.byte_at(0), Some(b'a'));
        ring.push_block(b"ef");
        assert_eq!(ring.byte_at(0), None);
        assert_eq!(ring.byte_at(2), Some(b'c'));
        assert_eq!(ring.byte_at(5), Some(b'f'));
    }

    #[test]
    fn finds_previous_block_match() {
        let mut d = InMemDeduplicator::new(16, 16, 1024).unwrap();
        let block0 = b"0123456789abcdef0123456789abcdef".to_vec();
        let prep0 = d.prepare_block(&block0);
        d.commit_block(0, &block0, &prep0);

        let block1 = block0.clone();
        let prep1 = d.prepare_block(&block1);
        let mut sink = sink(&block1);
        let matches = d.compress_block(block0.len() as u64, &block1, &prep1, &mut sink).unwrap();
        assert!(matches > 0);
        let (stats, literals) = sink.finish();
        assert!(!stats.is_empty());
        assert!(literals.len() < block1.len());
    }

    #[test]
    fn prepare_block_stores_one_entry_per_full_region_after_first() {
        let d = InMemDeduplicator::new(16, 16, 1024).unwrap();
        let block = vec![0u8; 64];
        let prepared = d.prepare_block(&block);
        assert_eq!(prepared.entries.len(), 3);
        assert!(prepared.entries.iter().all(|entry| entry.offset < 48));
    }

    #[test]
    fn ring_window_can_span_wrap_boundary() {
        let mut ring = RingDict::new(6);
        ring.push_block(b"abcdef");
        ring.push_block(b"gh");
        let window = ring.window(4, 4).unwrap();
        assert_eq!(window.len(), 4);
        match window {
            RingWindow::Split(a, b) => {
                assert_eq!(a, b"ef");
                assert_eq!(b, b"gh");
            }
            RingWindow::Contiguous(bytes) => panic!("expected split window, got {bytes:?}"),
        }
    }
}
