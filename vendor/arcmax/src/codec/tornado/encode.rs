use std::io::{Read, Write};

use crate::codec::tornado::arith::Lz77ArithEncoder;
use crate::codec::tornado::decode::{IMPOSSIBLE_DIST, IMPOSSIBLE_LEN};
use crate::codec::tornado::format::{EncodingMethod, TornadoHeader};
use crate::codec::tornado::huffman::{Lz77HuffEncoder, DC_BASE, DC_EXTRA, LC2_BASE, LC2_EXTRA};
use crate::codec::tornado::options::{
    EntropyKind, Hash3Mode, MatchFinderKind, ParserKind, TornadoOptions,
};
use crate::codec::tornado::table::{detect_and_apply_tables, TableRegion};
use crate::error::{ArcError, Result};

// Length VLE tables (bit coder)
const LC_EXTRA: [u32; 8] = [0, 0, 0, 1, 2, 4, 8, 30];
const LC_BASE: [u32; 8] = [0, 1, 2, 3, 5, 9, 25, 281];

// Optimal parser block size (32 KB, matching FreeArc OPTIMAL_WINDOW).
const OPTIMAL_WINDOW: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Match {
    len: usize,
    dist: usize,
}

fn codec_err(msg: impl Into<String>) -> ArcError {
    ArcError::Codec {
        codec: "tornado",
        message: msg.into(),
    }
}

pub fn compress(
    input: &mut dyn Read,
    output: &mut dyn Write,
    opts: &TornadoOptions,
) -> Result<u64> {
    let mut source = Vec::new();
    input.read_to_end(&mut source)?;

    // Detect numeric tables, apply delta transform in-place, record regions.
    // Optimal parser is incompatible with table preprocessing (matches FreeArc
    // OptimalParser::support_tables = FALSE).
    let tables = if opts.find_tables
        && opts.parser != ParserKind::Optimal
        && matches!(opts.entropy, EntropyKind::Huffman | EntropyKind::Arithmetic)
    {
        detect_and_apply_tables(&mut source)
    } else {
        Vec::new()
    };

    let mf = build_match_finder(opts);
    let min_match = mf.min_match();

    let encoding = match opts.entropy {
        EntropyKind::Store => EncodingMethod::Storing,
        EntropyKind::Byte => EncodingMethod::ByteCoder,
        EntropyKind::Bit => EncodingMethod::BitCoder,
        EntropyKind::Huffman => EncodingMethod::HuffmanCoder,
        EntropyKind::Arithmetic => EncodingMethod::ArithmeticCoder,
    };

    let buffer_size = opts.buffer_size.max(source.len()).max(1);
    let header = TornadoHeader {
        encoding,
        min_match_len: min_match as u8,
        buf_size: u32::try_from(buffer_size)
            .map_err(|_| codec_err(format!("Tornado buffer too large: {buffer_size}")))?,
    };

    output.write_all(&header.to_bytes())?;
    let encoded = encode_lz_stream(&source, buffer_size, mf, opts, &tables)?;
    output.write_all(&encoded)?;

    Ok((TornadoHeader::SIZE + encoded.len()) as u64)
}

fn encode_lz_stream(
    input: &[u8],
    window_size: usize,
    mf: Box<dyn MatchFinder>,
    opts: &TornadoOptions,
    tables: &[TableRegion],
) -> Result<Vec<u8>> {
    let min_match = mf.min_match();
    match opts.entropy {
        EntropyKind::Store => encode_with_coder(
            input,
            window_size,
            ByteLzCoder::new(min_match),
            true,
            opts,
            mf,
            tables,
        ),
        EntropyKind::Byte => encode_with_coder(
            input,
            window_size,
            ByteLzCoder::new(min_match),
            false,
            opts,
            mf,
            tables,
        ),
        EntropyKind::Bit => encode_with_coder(
            input,
            window_size,
            BitLzCoder::new(min_match),
            false,
            opts,
            mf,
            tables,
        ),
        EntropyKind::Huffman => encode_with_coder(
            input,
            window_size,
            HuffLzCoder::new(min_match),
            false,
            opts,
            mf,
            tables,
        ),
        EntropyKind::Arithmetic => encode_with_coder(
            input,
            window_size,
            ArithLzCoder::new(min_match),
            false,
            opts,
            mf,
            tables,
        ),
    }
}

fn encode_with_coder<C: LzTokenCoder>(
    input: &[u8],
    window_size: usize,
    coder: C,
    literals_only: bool,
    opts: &TornadoOptions,
    mf: Box<dyn MatchFinder>,
    tables: &[TableRegion],
) -> Result<Vec<u8>> {
    match opts.parser {
        ParserKind::Greedy => run_encode(
            input,
            window_size,
            coder,
            literals_only,
            GreedyParser,
            mf,
            tables,
        ),
        ParserKind::Lazy => run_encode(
            input,
            window_size,
            coder,
            literals_only,
            LazyParser::new(),
            mf,
            tables,
        ),
        ParserKind::Optimal => {
            let min_match = mf.min_match();
            run_encode_optimal(
                input,
                window_size,
                coder,
                min_match,
                opts.fast_bytes,
                mf,
                tables,
            )
        }
    }
}

fn run_encode<C: LzTokenCoder, P: ParseStrategy>(
    input: &[u8],
    window_size: usize,
    mut coder: C,
    literals_only: bool,
    mut parser: P,
    mut mf: Box<dyn MatchFinder>,
    tables: &[TableRegion],
) -> Result<Vec<u8>> {
    let mut pos = 0usize;
    let mut table_idx = 0usize;

    while pos < input.len() {
        // Emit table commands for any region that starts exactly at this position.
        while table_idx < tables.len() && tables[table_idx].start == pos {
            let t = &tables[table_idx];
            coder.encode_table(t.n, t.items)?;
            table_idx += 1;
        }

        let m = if literals_only {
            None
        } else {
            parser.step(input, pos, window_size, &mut *mf)
        };

        if let Some(m) = m {
            coder.encode_match(m.len, m.dist)?;
            pos += m.len;
        } else {
            let b = input[pos];
            if let Some(pd) = coder.prevdist_last() {
                let pd = pd as usize;
                if pos >= pd && input[pos - pd] == b {
                    coder.encode_repchar()?;
                    pos += 1;
                    continue;
                }
            }
            coder.encode_literal(b)?;
            pos += 1;
        }
    }

    coder.encode_eof()?;
    Ok(coder.finish())
}

// ── MatchFinder trait ─────────────────────────────────────────────────────────

trait MatchFinder {
    fn min_match(&self) -> usize;
    fn find_best(&mut self, buf: &[u8], pos: usize, window: usize) -> Option<Match>;
    fn update(&mut self, buf: &[u8], pos: usize);

    /// Return all valid matches at `pos`, sorted by increasing length.
    /// `max_match` caps how far match_len extends — callers should pass the
    /// maximum length they will actually use to avoid O(n²) on repetitive data.
    /// The default delegates to `find_best`; override to expose all hash slots.
    fn find_all(&mut self, buf: &[u8], pos: usize, window: usize, max_match: usize) -> Vec<Match> {
        match self.find_best(buf, pos, window) {
            Some(m) => {
                let capped_len = m.len.min(max_match);
                if capped_len >= self.min_match() {
                    vec![Match {
                        len: capped_len,
                        dist: m.dist,
                    }]
                } else {
                    vec![]
                }
            }
            None => vec![],
        }
    }

    fn update_range(&mut self, buf: &[u8], pos: usize, len: usize) {
        let mm = self.min_match();
        for p in pos..pos.saturating_add(len) {
            if p + mm <= buf.len() {
                self.update(buf, p);
            }
        }
    }
}

// ── Parser strategies ─────────────────────────────────────────────────────────

trait ParseStrategy {
    fn step(
        &mut self,
        buf: &[u8],
        pos: usize,
        window: usize,
        mf: &mut dyn MatchFinder,
    ) -> Option<Match>;
}

struct GreedyParser;

impl ParseStrategy for GreedyParser {
    fn step(
        &mut self,
        buf: &[u8],
        pos: usize,
        window: usize,
        mf: &mut dyn MatchFinder,
    ) -> Option<Match> {
        if let Some(m) = mf.find_best(buf, pos, window) {
            mf.update_range(buf, pos, m.len);
            Some(m)
        } else {
            mf.update_range(buf, pos, 1);
            None
        }
    }
}

struct LazyParser {
    cached: Option<Match>,
}

impl LazyParser {
    fn new() -> Self {
        Self { cached: None }
    }
}

impl ParseStrategy for LazyParser {
    fn step(
        &mut self,
        buf: &[u8],
        pos: usize,
        window: usize,
        mf: &mut dyn MatchFinder,
    ) -> Option<Match> {
        let m = if let Some(c) = self.cached.take() {
            c
        } else {
            match mf.find_best(buf, pos, window) {
                Some(m) => m,
                None => {
                    mf.update_range(buf, pos, 1);
                    return None;
                }
            }
        };

        // Update hash for pos so pos+1 lookup sees it.
        mf.update_range(buf, pos, 1);

        // One-position lookahead: prefer a strictly longer match one byte ahead.
        if pos + 1 < buf.len() {
            if let Some(next) = mf.find_best(buf, pos + 1, window) {
                if next.len > m.len {
                    self.cached = Some(next);
                    return None;
                }
            }
        }

        // Use current match; update the interior positions.
        mf.update_range(buf, pos + 1, m.len.saturating_sub(1));
        Some(m)
    }
}

// ── SimpleHashMatcher ─────────────────────────────────────────────────────────
// Single entry per hash bucket. Used for MatchFinderKind::NonCaching.

struct SimpleHashMatcher {
    table: Vec<u32>,
    hash_shift: u32,
    hash_mask: usize,
}

impl SimpleHashMatcher {
    fn new(hash_size: usize) -> Self {
        let size = hash_size.next_power_of_two().max(16);
        let bits = size.trailing_zeros();
        Self {
            table: vec![u32::MAX; size],
            hash_shift: 32u32.saturating_sub(bits),
            hash_mask: size - 1,
        }
    }

    #[inline]
    fn hash(&self, buf: &[u8], pos: usize) -> usize {
        (read_u32_le(buf, pos).wrapping_mul(123_456_791) >> self.hash_shift) as usize
            & self.hash_mask
    }
}

impl MatchFinder for SimpleHashMatcher {
    fn min_match(&self) -> usize {
        4
    }

    fn find_best(&mut self, buf: &[u8], pos: usize, window: usize) -> Option<Match> {
        if pos + 4 > buf.len() {
            return None;
        }
        let h = self.hash(buf, pos);
        let raw = self.table[h];
        if raw == u32::MAX {
            return None;
        }
        let cand = raw as usize;
        if cand >= pos {
            return None;
        }
        let dist = pos - cand;
        if dist > window {
            return None;
        }
        let len = match_len(buf, cand, pos, buf.len() - pos);
        if len >= 4 && accept_len_dist(len, dist) {
            Some(Match { len, dist })
        } else {
            None
        }
    }

    fn update(&mut self, buf: &[u8], pos: usize) {
        if pos + 4 <= buf.len() {
            let h = self.hash(buf, pos);
            self.table[h] = pos as u32;
        }
    }
}

// ── CachingMatchFinder ────────────────────────────────────────────────────────
// row_width entries per hash bucket, front-insert (MRU order).

struct CachingMatchFinder {
    table: Vec<u32>,
    row_width: usize,
    hash_shift: u32,
    hash_mask: usize,
    min_len: usize,
}

impl CachingMatchFinder {
    fn new(hash_size: usize, row_width: usize, min_len: usize) -> Self {
        let rw = row_width.max(1);
        // hash_size is in bytes; each entry is 4 bytes (u32).
        let num_buckets = (hash_size / 4 / rw).next_power_of_two().max(16);
        let bits = num_buckets.trailing_zeros();
        Self {
            table: vec![u32::MAX; num_buckets * rw],
            row_width: rw,
            hash_shift: 32u32.saturating_sub(bits),
            hash_mask: num_buckets - 1,
            min_len,
        }
    }

    #[inline]
    fn hash(&self, buf: &[u8], pos: usize) -> usize {
        (read_u32_le(buf, pos).wrapping_mul(123_456_791) >> self.hash_shift) as usize
            & self.hash_mask
    }
}

impl MatchFinder for CachingMatchFinder {
    fn min_match(&self) -> usize {
        self.min_len
    }

    fn find_best(&mut self, buf: &[u8], pos: usize, window: usize) -> Option<Match> {
        if pos + self.min_len > buf.len() {
            return None;
        }
        let h = self.hash(buf, pos);
        let row_start = h * self.row_width;
        let max_len = buf.len() - pos;
        let mut best: Option<Match> = None;

        for i in 0..self.row_width {
            let raw = self.table[row_start + i];
            if raw == u32::MAX {
                continue;
            }
            let cand = raw as usize;
            if cand >= pos {
                continue;
            }
            let dist = pos - cand;
            if dist > window {
                continue;
            }
            let len = match_len(buf, cand, pos, max_len);
            if len >= self.min_len && accept_len_dist(len, dist) {
                if best.map_or(true, |b| len > b.len) {
                    best = Some(Match { len, dist });
                }
            }
        }
        best
    }

    fn find_all(&mut self, buf: &[u8], pos: usize, window: usize, max_match: usize) -> Vec<Match> {
        if pos + self.min_len > buf.len() {
            return vec![];
        }
        let h = self.hash(buf, pos);
        let row_start = h * self.row_width;
        let max_len = (buf.len() - pos).min(max_match);
        let mut matches = Vec::with_capacity(self.row_width);
        for i in 0..self.row_width {
            let raw = self.table[row_start + i];
            if raw == u32::MAX {
                continue;
            }
            let cand = raw as usize;
            if cand >= pos {
                continue;
            }
            let dist = pos - cand;
            if dist > window {
                continue;
            }
            let len = match_len(buf, cand, pos, max_len);
            if len >= self.min_len && accept_len_dist(len, dist) {
                matches.push(Match { len, dist });
            }
        }
        matches.sort_unstable_by_key(|m| m.len);
        matches
    }

    fn update(&mut self, buf: &[u8], pos: usize) {
        if pos + self.min_len > buf.len() {
            return;
        }
        let h = self.hash(buf, pos);
        let row_start = h * self.row_width;
        let end = row_start + self.row_width;
        // Front-insert: shift existing entries right by 1, overwrite slot 0.
        if self.row_width > 1 {
            self.table.copy_within(row_start..end - 1, row_start + 1);
        }
        self.table[row_start] = pos as u32;
    }
}

// ── CycledCachingMatchFinder ──────────────────────────────────────────────────
// row_width entries per bucket, cycled (round-robin) eviction.

struct CycledCachingMatchFinder {
    table: Vec<u32>,
    next_slot: Vec<u8>,
    row_width: usize,
    hash_shift: u32,
    hash_mask: usize,
    min_len: usize,
}

impl CycledCachingMatchFinder {
    fn new(hash_size: usize, row_width: usize, min_len: usize) -> Self {
        let rw = row_width.max(1);
        // hash_size is in bytes; each entry is 4 bytes (u32).
        let num_buckets = (hash_size / 4 / rw).next_power_of_two().max(16);
        let bits = num_buckets.trailing_zeros();
        Self {
            table: vec![u32::MAX; num_buckets * rw],
            next_slot: vec![0u8; num_buckets],
            row_width: rw,
            hash_shift: 32u32.saturating_sub(bits),
            hash_mask: num_buckets - 1,
            min_len,
        }
    }

    #[inline]
    fn hash(&self, buf: &[u8], pos: usize) -> usize {
        (read_u32_le(buf, pos).wrapping_mul(123_456_791) >> self.hash_shift) as usize
            & self.hash_mask
    }
}

impl MatchFinder for CycledCachingMatchFinder {
    fn min_match(&self) -> usize {
        self.min_len
    }

    fn find_best(&mut self, buf: &[u8], pos: usize, window: usize) -> Option<Match> {
        if pos + self.min_len > buf.len() {
            return None;
        }
        let h = self.hash(buf, pos);
        let row_start = h * self.row_width;
        let max_len = buf.len() - pos;
        let mut best: Option<Match> = None;

        for i in 0..self.row_width {
            let raw = self.table[row_start + i];
            if raw == u32::MAX {
                continue;
            }
            let cand = raw as usize;
            if cand >= pos {
                continue;
            }
            let dist = pos - cand;
            if dist > window {
                continue;
            }
            let len = match_len(buf, cand, pos, max_len);
            if len >= self.min_len && accept_len_dist(len, dist) {
                if best.map_or(true, |b| len > b.len) {
                    best = Some(Match { len, dist });
                }
            }
        }
        best
    }

    fn find_all(&mut self, buf: &[u8], pos: usize, window: usize, max_match: usize) -> Vec<Match> {
        if pos + self.min_len > buf.len() {
            return vec![];
        }
        let h = self.hash(buf, pos);
        let row_start = h * self.row_width;
        let max_len = (buf.len() - pos).min(max_match);
        let mut matches = Vec::with_capacity(self.row_width);
        for i in 0..self.row_width {
            let raw = self.table[row_start + i];
            if raw == u32::MAX {
                continue;
            }
            let cand = raw as usize;
            if cand >= pos {
                continue;
            }
            let dist = pos - cand;
            if dist > window {
                continue;
            }
            let len = match_len(buf, cand, pos, max_len);
            if len >= self.min_len && accept_len_dist(len, dist) {
                matches.push(Match { len, dist });
            }
        }
        matches.sort_unstable_by_key(|m| m.len);
        matches
    }

    fn update(&mut self, buf: &[u8], pos: usize) {
        if pos + self.min_len > buf.len() {
            return;
        }
        let h = self.hash(buf, pos);
        let row_start = h * self.row_width;
        let slot = (self.next_slot[h] as usize) % self.row_width;
        self.table[row_start + slot] = pos as u32;
        self.next_slot[h] = self.next_slot[h].wrapping_add(1);
    }
}

// ── BinaryTreeMatchFinder ─────────────────────────────────────────────────────
// Binary-tree match finder derived from LZMA/LzFind.c (Igor Pavlov, GPL).
// For every hash bucket the chain is organised as a binary tree keyed on the
// bytes that follow the 4/5-byte hash prefix, allowing O(depth) search for the
// longest match.

struct BinaryTreeMatchFinder {
    htable: Vec<u32>, // hash → most-recent matching position (u32::MAX = empty)
    tree: Vec<u32>,   // 2 * dict_size; [2*(pos & mask)] = left, +1 = right child
    dict_mask: usize, // dict_size - 1 (dict_size is a power of 2)
    hash_shift: u32,
    hash_mask: usize,
    max_depth: usize,
    min_len: usize, // 4 or 5 — hash bytes and minimum returned match length
}

impl BinaryTreeMatchFinder {
    fn new(hash_size: usize, dict_size: usize, max_depth: usize, min_len: usize) -> Self {
        let dict_size = dict_size.next_power_of_two().max(16);
        let hash_size = hash_size.next_power_of_two().max(16);
        let bits = hash_size.trailing_zeros();
        Self {
            htable: vec![u32::MAX; hash_size],
            tree: vec![u32::MAX; 2 * dict_size],
            dict_mask: dict_size - 1,
            hash_shift: 32u32.saturating_sub(bits),
            hash_mask: hash_size - 1,
            max_depth: max_depth.max(1),
            min_len,
        }
    }

    #[inline]
    fn hash_pos(&self, buf: &[u8], pos: usize) -> usize {
        let v0 = read_u32_le(buf, pos).wrapping_mul(123_456_791);
        let h = if self.min_len >= 5 {
            v0.wrapping_add(read_u32_le(buf, pos + 1).wrapping_mul(789_567_123))
        } else {
            v0
        };
        (h >> self.hash_shift) as usize & self.hash_mask
    }

    /// Search for the best match at `pos`, simultaneously inserting `pos` into
    /// the tree.  Mirrors `BinaryTreeMatchFinder::find_matchlen` from
    /// `MatchFinder.cpp`.
    fn find_and_insert(&mut self, buf: &[u8], pos: usize, window: usize) -> Option<Match> {
        let min_len = self.min_len;
        if pos + min_len > buf.len() {
            return None;
        }
        let len_limit = buf.len() - pos;

        let h = self.hash_pos(buf, pos);
        let mut n = self.htable[h];
        self.htable[h] = pos as u32;

        let pos_idx = (pos & self.dict_mask) * 2;
        let mut ptr1 = pos_idx; // left ("smaller") child slot for pos
        let mut ptr0 = pos_idx + 1; // right ("larger") child slot for pos

        let mut len0 = 0usize;
        let mut len1 = 0usize;
        let mut best: Option<Match> = None;
        let mut depth = self.max_depth;

        loop {
            if n == u32::MAX || depth == 0 {
                self.tree[ptr0] = u32::MAX;
                self.tree[ptr1] = u32::MAX;
                break;
            }
            depth -= 1;

            let cand = n as usize;
            if cand >= pos || pos - cand > window {
                self.tree[ptr0] = u32::MAX;
                self.tree[ptr1] = u32::MAX;
                break;
            }
            let dist = pos - cand;

            let cand_idx = (cand & self.dict_mask) * 2;
            let pair_left = self.tree[cand_idx];
            let pair_right = self.tree[cand_idx + 1];

            // Extend from the known common prefix (binary-tree invariant).
            let common = len0.min(len1);
            let mut len = common;
            while len < len_limit && buf[cand + len] == buf[pos + len] {
                len += 1;
            }

            if len >= min_len {
                if best.map_or(true, |b: Match| len > b.len) && accept_len_dist(len, dist) {
                    best = Some(Match { len, dist });
                }
            }
            if len == len_limit {
                self.tree[ptr1] = pair_left;
                self.tree[ptr0] = pair_right;
                break;
            }

            if buf[cand + len] < buf[pos + len] {
                self.tree[ptr1] = n;
                ptr1 = cand_idx + 1;
                n = pair_right;
                len1 = len;
            } else {
                self.tree[ptr0] = n;
                ptr0 = cand_idx;
                n = pair_left;
                len0 = len;
            }
        }

        best
    }

    /// Like `find_and_insert` but collects every new-longest match found during
    /// the walk.  Used by `find_all` for the optimal parser.
    fn find_all_and_insert(
        &mut self,
        buf: &[u8],
        pos: usize,
        window: usize,
        max_match: usize,
    ) -> Vec<Match> {
        let min_len = self.min_len;
        if pos + min_len > buf.len() {
            return vec![];
        }
        let len_limit = (buf.len() - pos).min(max_match);

        let h = self.hash_pos(buf, pos);
        let mut n = self.htable[h];
        self.htable[h] = pos as u32;

        let pos_idx = (pos & self.dict_mask) * 2;
        let mut ptr1 = pos_idx;
        let mut ptr0 = pos_idx + 1;

        let mut len0 = 0usize;
        let mut len1 = 0usize;
        let mut max_seen = min_len.saturating_sub(1);
        let mut matches: Vec<Match> = Vec::new();
        let mut depth = self.max_depth;

        loop {
            if n == u32::MAX || depth == 0 {
                self.tree[ptr0] = u32::MAX;
                self.tree[ptr1] = u32::MAX;
                break;
            }
            depth -= 1;

            let cand = n as usize;
            if cand >= pos || pos - cand > window {
                self.tree[ptr0] = u32::MAX;
                self.tree[ptr1] = u32::MAX;
                break;
            }
            let dist = pos - cand;

            let cand_idx = (cand & self.dict_mask) * 2;
            let pair_left = self.tree[cand_idx];
            let pair_right = self.tree[cand_idx + 1];

            let common = len0.min(len1);
            let mut len = common;
            while len < len_limit && buf[cand + len] == buf[pos + len] {
                len += 1;
            }

            if len > max_seen && accept_len_dist(len, dist) {
                matches.push(Match { len, dist });
                max_seen = len;
            }
            if len == len_limit {
                self.tree[ptr1] = pair_left;
                self.tree[ptr0] = pair_right;
                break;
            }

            if buf[cand + len] < buf[pos + len] {
                self.tree[ptr1] = n;
                ptr1 = cand_idx + 1;
                n = pair_right;
                len1 = len;
            } else {
                self.tree[ptr0] = n;
                ptr0 = cand_idx;
                n = pair_left;
                len0 = len;
            }
        }

        matches
    }
}

impl MatchFinder for BinaryTreeMatchFinder {
    fn min_match(&self) -> usize {
        self.min_len
    }

    fn find_best(&mut self, buf: &[u8], pos: usize, window: usize) -> Option<Match> {
        self.find_and_insert(buf, pos, window)
    }

    fn find_all(&mut self, buf: &[u8], pos: usize, window: usize, max_match: usize) -> Vec<Match> {
        self.find_all_and_insert(buf, pos, window, max_match)
    }

    fn update(&mut self, buf: &[u8], pos: usize) {
        // Insert pos without caring about the match; window = full dict.
        let window = self.dict_mask + 1;
        let _ = self.find_and_insert(buf, pos, window);
    }
}

// ── Hash3Mf ───────────────────────────────────────────────────────────────────
// Wraps any match finder; adds a secondary short-match (3- or 2-byte) table.
// `row_width` entries per hash bucket (MRU eviction, same as CachingMatchFinder).

struct Hash3Mf {
    inner: Box<dyn MatchFinder>,
    short_table: Vec<u32>,
    short_mask: usize,
    short_shift: u32,
    short_min: usize,
    row_width: usize,
}

impl Hash3Mf {
    fn new(
        inner: Box<dyn MatchFinder>,
        table_size: usize,
        row_width: usize,
        short_min: usize,
    ) -> Self {
        let rw = row_width.max(1);
        let size = table_size.next_power_of_two().max(256);
        let bits = size.trailing_zeros();
        Self {
            inner,
            short_table: vec![u32::MAX; size * rw],
            short_mask: size - 1,
            short_shift: 32u32.saturating_sub(bits),
            short_min,
            row_width: rw,
        }
    }

    #[inline]
    fn hash_short(&self, buf: &[u8], pos: usize) -> usize {
        let v = (buf[pos] as u32) | ((buf[pos + 1] as u32) << 8) | ((buf[pos + 2] as u32) << 16);
        (v.wrapping_mul(2_654_435_761) >> self.short_shift) as usize & self.short_mask
    }

    fn find_short_best(&self, buf: &[u8], pos: usize, window: usize) -> Option<Match> {
        if pos + 3 > buf.len() {
            return None;
        }
        let h = self.hash_short(buf, pos);
        let row_start = h * self.row_width;
        let max_len = buf.len() - pos;
        let mut best: Option<Match> = None;
        for i in 0..self.row_width {
            let raw = self.short_table[row_start + i];
            if raw == u32::MAX {
                continue;
            }
            let cand = raw as usize;
            if cand >= pos {
                continue;
            }
            let dist = pos - cand;
            if dist > window || dist == 0 {
                continue;
            }
            let len = match_len(buf, cand, pos, max_len);
            if len >= self.short_min && best.map_or(true, |b| len > b.len) {
                best = Some(Match { len, dist });
            }
        }
        best
    }

    fn find_short_all(
        &self,
        buf: &[u8],
        pos: usize,
        window: usize,
        max_match: usize,
    ) -> Vec<Match> {
        if pos + 3 > buf.len() {
            return vec![];
        }
        let h = self.hash_short(buf, pos);
        let row_start = h * self.row_width;
        let max_len = (buf.len() - pos).min(max_match);
        let mut out = Vec::with_capacity(self.row_width);
        for i in 0..self.row_width {
            let raw = self.short_table[row_start + i];
            if raw == u32::MAX {
                continue;
            }
            let cand = raw as usize;
            if cand >= pos {
                continue;
            }
            let dist = pos - cand;
            if dist > window || dist == 0 {
                continue;
            }
            let len = match_len(buf, cand, pos, max_len);
            if len >= self.short_min {
                out.push(Match { len, dist });
            }
        }
        out
    }

    fn update_short(&mut self, buf: &[u8], pos: usize) {
        if pos + 3 > buf.len() {
            return;
        }
        let h = self.hash_short(buf, pos);
        let row_start = h * self.row_width;
        if self.row_width > 1 {
            self.short_table
                .copy_within(row_start..row_start + self.row_width - 1, row_start + 1);
        }
        self.short_table[row_start] = pos as u32;
    }
}

impl MatchFinder for Hash3Mf {
    fn min_match(&self) -> usize {
        self.short_min
    }

    fn find_best(&mut self, buf: &[u8], pos: usize, window: usize) -> Option<Match> {
        let inner_best = self.inner.find_best(buf, pos, window);
        let short_best = self.find_short_best(buf, pos, window);
        match (inner_best, short_best) {
            (Some(a), Some(b)) => Some(if b.len > a.len { b } else { a }),
            (Some(a), None) => Some(a),
            (None, b) => b,
        }
    }

    fn find_all(&mut self, buf: &[u8], pos: usize, window: usize, max_match: usize) -> Vec<Match> {
        let mut matches = self.inner.find_all(buf, pos, window, max_match);
        matches.extend(self.find_short_all(buf, pos, window, max_match));
        matches.sort_unstable_by_key(|m| m.len);
        matches
    }

    fn update(&mut self, buf: &[u8], pos: usize) {
        self.inner.update(buf, pos);
        self.update_short(buf, pos);
    }
}

// ── SteppedMf ─────────────────────────────────────────────────────────────────
// Wraps any match finder and applies update_step to update_range calls.
// When step > 1, only every step-th position inside a match is inserted into
// the hash, matching FreeArc's HashCoder update_step behaviour for low levels.

struct SteppedMf {
    inner: Box<dyn MatchFinder>,
    step: usize,
}

impl SteppedMf {
    fn new(inner: Box<dyn MatchFinder>, step: usize) -> Self {
        Self {
            inner,
            step: step.max(1),
        }
    }
}

impl MatchFinder for SteppedMf {
    fn min_match(&self) -> usize {
        self.inner.min_match()
    }

    fn find_best(&mut self, buf: &[u8], pos: usize, window: usize) -> Option<Match> {
        self.inner.find_best(buf, pos, window)
    }

    fn find_all(&mut self, buf: &[u8], pos: usize, window: usize, max_match: usize) -> Vec<Match> {
        self.inner.find_all(buf, pos, window, max_match)
    }

    fn update(&mut self, buf: &[u8], pos: usize) {
        self.inner.update(buf, pos);
    }

    fn update_range(&mut self, buf: &[u8], pos: usize, len: usize) {
        let mm = self.inner.min_match();
        let mut p = pos;
        while p < pos.saturating_add(len) {
            if p + mm <= buf.len() {
                self.inner.update(buf, p);
            }
            p += self.step;
        }
    }
}

// ── Match finder construction ─────────────────────────────────────────────────

fn build_match_finder(opts: &TornadoOptions) -> Box<dyn MatchFinder> {
    let hash_size = opts.hash_size.next_power_of_two().max(16);
    let row_width = opts.hash_row_width.max(1);

    let base: Box<dyn MatchFinder> = match opts.match_finder {
        MatchFinderKind::NonCaching => Box::new(SimpleHashMatcher::new(hash_size)),
        MatchFinderKind::Caching {
            min_len,
            cycled: false,
        } => Box::new(CachingMatchFinder::new(hash_size, row_width, min_len)),
        MatchFinderKind::Caching {
            min_len,
            cycled: true,
        } => Box::new(CycledCachingMatchFinder::new(hash_size, row_width, min_len)),
        MatchFinderKind::BinaryTree { min_len } => {
            let dict_size = opts.buffer_size.next_power_of_two().max(16);
            Box::new(BinaryTreeMatchFinder::new(
                hash_size,
                dict_size,
                opts.fast_bytes,
                min_len,
            ))
        }
    };

    let aux_size = opts.aux_hash_size.next_power_of_two().max(256);
    let aux_rw = opts.aux_hash_row_width.max(1);
    let mf: Box<dyn MatchFinder> = match opts.hash3_mode {
        Hash3Mode::Disabled => base,
        Hash3Mode::Hash3 => Box::new(Hash3Mf::new(base, aux_size, aux_rw, 3)),
        Hash3Mode::Hash2 => Box::new(Hash3Mf::new(base, aux_size, aux_rw, 2)),
    };

    // Apply update_step: wrap the complete match finder (primary + aux) so that
    // interior-of-match hash updates honour the preset's skip factor.
    if opts.update_step > 1 {
        Box::new(SteppedMf::new(mf, opts.update_step))
    } else {
        mf
    }
}

// ── Shared utilities ──────────────────────────────────────────────────────────

#[inline]
fn read_u32_le(buf: &[u8], pos: usize) -> u32 {
    u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap())
}

#[inline]
fn accept_len_dist(len: usize, dist: usize) -> bool {
    match len {
        4 => dist < 48 * 1024,
        5 => dist < 192 * 1024,
        6 => dist < 1024 * 1024,
        7 => dist < 12 * 1024 * 1024,
        _ => true,
    }
}

#[inline]
fn match_len(buf: &[u8], a: usize, b: usize, max_len: usize) -> usize {
    let mut len = 0usize;
    while len + 4 <= max_len && read_u32_le(buf, a + len) == read_u32_le(buf, b + len) {
        len += 4;
    }
    while len < max_len && buf[a + len] == buf[b + len] {
        len += 1;
    }
    len
}

fn find_vle_code(value: u32, base: &[u32], extra: &[u32]) -> Option<(usize, u32)> {
    for (code, (&b, &bits)) in base.iter().zip(extra.iter()).enumerate() {
        let span = if bits == 32 {
            u32::MAX
        } else {
            (1u32 << bits).saturating_sub(1)
        };
        if value >= b && value <= b.saturating_add(span) {
            return Some((code, value - b));
        }
    }
    None
}

/// Static price estimate for a match, in bits, using the Huffman VLE tables.
/// Used by the optimal parser DP; does not need to be exact, only monotone.
fn match_price_static(raw_len: u32, dist: usize) -> u32 {
    let dist0 = dist.saturating_sub(1) as u32;
    let lcode = find_vle_code(raw_len, &LC2_BASE, &LC2_EXTRA)
        .map(|(c, _)| c)
        .unwrap_or(LC2_BASE.len() - 1);
    let dcode = find_vle_code(dist0, &DC_BASE, &DC_EXTRA)
        .map(|(c, _)| c)
        .unwrap_or(DC_BASE.len() - 1);
    9 + LC2_EXTRA[lcode] + DC_EXTRA[dcode]
}

/// Forward DP optimal parser: processes input in OPTIMAL_WINDOW blocks,
/// evaluates all match candidates per position, reconstructs the cheapest
/// token path, then emits it via `coder`.
fn run_encode_optimal<C: LzTokenCoder>(
    input: &[u8],
    window_size: usize,
    mut coder: C,
    min_match: usize,
    fast_bytes: usize,
    mut mf: Box<dyn MatchFinder>,
    tables: &[TableRegion],
) -> Result<Vec<u8>> {
    const MAX_P: u32 = u32::MAX / 2;

    let mut global_pos = 0usize;
    let mut table_idx = 0usize;

    while global_pos < input.len() {
        let block_start = global_pos;
        let block_end = (block_start + OPTIMAL_WINDOW).min(input.len());
        let block_len = block_end - block_start;

        // prices[i] = minimum bit cost to encode the first (block_start + i) bytes.
        let mut prices = vec![MAX_P; block_len + 1];
        // arrivals[i] = (len, dist) of the token that achieves prices[i].
        // dist==0 means literal.
        let mut arrivals: Vec<(usize, usize)> = vec![(1, 0); block_len + 1];
        prices[0] = 0;

        // ── Forward DP pass ───────────────────────────────────────────────────
        // `silence` mirrors the C++ FreeArc OptimalParser silence optimization:
        // after finding a match of length > fast_bytes, skip evaluating interior
        // positions (they're covered by the long match's endpoint price).  This
        // keeps the forward pass O(n * fast_bytes) rather than O(n²) on data
        // with very long matches.
        let mut silence = block_start;

        for p in block_start..block_end {
            let src = p - block_start;

            if p < silence {
                // Inside the silent range — only advance the match finder.
                mf.update(input, p);
                continue;
            }

            let cur_price = prices[src];
            if cur_price >= MAX_P {
                mf.update(input, p);
                continue;
            }

            // Evaluate encoding as a literal.
            let lp = cur_price + 9;
            if lp < prices[src + 1] {
                prices[src + 1] = lp;
                arrivals[src + 1] = (1, 0);
            }

            // Find all match candidates; cap at block boundary to avoid cross-block
            // issues.  The silence check above ensures this is called O(n / fast_bytes)
            // times on highly repetitive input, keeping match_len compute O(n) total.
            let max_match = block_end - p;
            let candidates = mf.find_all(input, p, window_size, max_match);

            let mut max_found = 0usize;
            for m in &candidates {
                let max_l = m.len;
                // Dense evaluation for short/medium lengths (O(fast_bytes) per match).
                let dense_end = max_l.min(fast_bytes);
                for l in min_match..=dense_end {
                    let mp = cur_price + match_price_static((l - min_match) as u32, m.dist);
                    let dst = src + l;
                    if dst <= block_len && mp < prices[dst] {
                        prices[dst] = mp;
                        arrivals[dst] = (l, m.dist);
                    }
                }
                // Also price the full match endpoint so long matches remain reachable.
                if max_l > fast_bytes {
                    let mp = cur_price + match_price_static((max_l - min_match) as u32, m.dist);
                    let dst = src + max_l;
                    if dst <= block_len && mp < prices[dst] {
                        prices[dst] = mp;
                        arrivals[dst] = (max_l, m.dist);
                    }
                    max_found = max_found.max(max_l);
                }
            }

            // Silence the range [p+1, p+max_found-1]: their prices were set by the
            // long match evaluation above; no need to re-evaluate them individually.
            if max_found > fast_bytes {
                silence = p + max_found;
            }

            mf.update(input, p);
        }

        // ── Path reconstruction: trace back from block_end to block_start ─────
        let mut path: Vec<(usize, usize)> = Vec::new();
        let mut i = block_len;
        while i > 0 {
            let (len, dist) = arrivals[i];
            path.push((len, dist));
            i -= len;
        }
        path.reverse();

        // ── Emit tokens in forward order ──────────────────────────────────────
        let mut emit_pos = block_start;
        for (len, dist) in path {
            while table_idx < tables.len() && tables[table_idx].start == emit_pos {
                let t = &tables[table_idx];
                coder.encode_table(t.n, t.items)?;
                table_idx += 1;
            }
            if dist == 0 {
                let b = input[emit_pos];
                let mut emitted_repchar = false;
                if let Some(pd) = coder.prevdist_last() {
                    let pd = pd as usize;
                    if emit_pos >= pd && input[emit_pos - pd] == b {
                        coder.encode_repchar()?;
                        emitted_repchar = true;
                    }
                }
                if !emitted_repchar {
                    coder.encode_literal(b)?;
                }
            } else {
                coder.encode_match(len, dist)?;
            }
            emit_pos += len;
        }
        debug_assert_eq!(
            emit_pos, block_end,
            "path reconstruction did not cover full block"
        );

        global_pos = block_end;
    }

    coder.encode_eof()?;
    Ok(coder.finish())
}

// ── LzTokenCoder trait ────────────────────────────────────────────────────────

trait LzTokenCoder {
    fn encode_literal(&mut self, b: u8) -> Result<()>;
    fn encode_match(&mut self, len: usize, dist: usize) -> Result<()>;
    /// Emit a table preprocessing command (only supported by Huffman/Arith coders).
    fn encode_table(&mut self, _type_n: usize, _items: usize) -> Result<()> {
        Err(codec_err("encode_table not supported by this coder"))
    }
    fn encode_eof(&mut self) -> Result<()>;
    fn finish(self) -> Vec<u8>;
    /// Return the most-recent match distance, or None if no match emitted yet.
    /// Coders that support REPCHAR override this.
    fn prevdist_last(&self) -> Option<u32> {
        None
    }
    /// Emit REPCHAR symbol (1-byte match at the previous distance).
    /// Only valid when prevdist_last() returns Some.
    fn encode_repchar(&mut self) -> Result<()> {
        Err(codec_err("encode_repchar not supported by this coder"))
    }
}

// ── ByteLzCoder ───────────────────────────────────────────────────────────────

struct ByteLzCoder {
    out: Vec<u8>,
    flags: u32,
    symbols_in_word: usize,
    flag_anchor: usize,
    min_match: usize,
}

impl ByteLzCoder {
    fn new(min_match: usize) -> Self {
        let mut this = Self {
            out: Vec::new(),
            flags: 0,
            symbols_in_word: 0,
            flag_anchor: 0,
            min_match,
        };
        this.reserve_flags_word();
        this
    }

    fn reserve_flags_word(&mut self) {
        self.flag_anchor = self.out.len();
        self.out.extend_from_slice(&0u32.to_le_bytes());
        self.flags = 0;
        self.symbols_in_word = 0;
    }

    fn begin_symbol(&mut self, kind: u32) {
        if self.symbols_in_word == 16 {
            self.commit_flags();
            self.reserve_flags_word();
        }
        self.flags |= kind << (self.symbols_in_word * 2);
        self.symbols_in_word += 1;
    }

    fn commit_flags(&mut self) {
        self.out[self.flag_anchor..self.flag_anchor + 4].copy_from_slice(&self.flags.to_le_bytes());
    }

    fn write_long_match(&mut self, raw_len: u32, dist: u32) {
        if dist >= (1 << 24) {
            self.out.push(255);
            self.out.push((dist >> 24) as u8);
        }

        let len_low = if raw_len >= 254 {
            self.out.push(254);
            let hi = raw_len >> 8;
            self.out.extend_from_slice(&hi.to_le_bytes()[..3]);
            raw_len & 0xFF
        } else {
            raw_len
        };

        let packed = len_low | ((dist & 0xFF_FFFF) << 8);
        self.out.extend_from_slice(&packed.to_le_bytes());
    }
}

impl LzTokenCoder for ByteLzCoder {
    fn encode_literal(&mut self, b: u8) -> Result<()> {
        self.begin_symbol(0);
        self.out.push(b);
        Ok(())
    }

    fn encode_match(&mut self, len: usize, dist: usize) -> Result<()> {
        if len < self.min_match {
            return Err(codec_err(format!(
                "match length {len} below minimum {}",
                self.min_match
            )));
        }
        if dist == 0 {
            return Err(codec_err("match distance must be non-zero"));
        }

        let raw_len = (len - self.min_match) as u32;
        if raw_len <= 0xF && dist <= 0xFFF {
            self.begin_symbol(1);
            let packed = (raw_len << 12) | dist as u32;
            self.out.extend_from_slice(&(packed as u16).to_le_bytes());
        } else if raw_len <= 0x3F && dist <= 0x3_FFFF {
            self.begin_symbol(2);
            let packed = (raw_len << 18) | dist as u32;
            self.out.extend_from_slice(&packed.to_le_bytes()[..3]);
        } else {
            self.begin_symbol(3);
            self.write_long_match(raw_len, dist as u32);
        }
        Ok(())
    }

    fn encode_eof(&mut self) -> Result<()> {
        let raw = IMPOSSIBLE_LEN - self.min_match as u32;
        self.begin_symbol(3);
        self.write_long_match(raw, IMPOSSIBLE_DIST);
        Ok(())
    }

    fn finish(mut self) -> Vec<u8> {
        self.commit_flags();
        self.out
    }
}

// ── BitLzCoder ────────────────────────────────────────────────────────────────

struct BitLzCoder {
    writer: BitWriter,
    min_match: usize,
}

impl BitLzCoder {
    fn new(min_match: usize) -> Self {
        Self {
            writer: BitWriter::new(),
            min_match,
        }
    }
}

impl LzTokenCoder for BitLzCoder {
    fn encode_literal(&mut self, b: u8) -> Result<()> {
        self.writer.write_bits(9, b as u32);
        Ok(())
    }

    fn encode_match(&mut self, len: usize, dist: usize) -> Result<()> {
        if len < self.min_match {
            return Err(codec_err(format!(
                "match length {len} below minimum {}",
                self.min_match
            )));
        }
        if dist == 0 {
            return Err(codec_err("match distance must be non-zero"));
        }

        let raw_len = (len - self.min_match) as u32;
        let (lcode, lbits_value) = find_vle_code(raw_len, &LC_BASE, &LC_EXTRA)
            .ok_or_else(|| codec_err(format!("length too large for bitcoder: {len}")))?;
        let (dcode, dbits_value) = find_vle_code(dist as u32, &DC_BASE, &DC_EXTRA)
            .ok_or_else(|| codec_err(format!("distance too large for bitcoder: {dist}")))?;

        let symbol = (((lcode as u32) + 8) << 5) | dcode as u32;
        self.writer.write_bits(9, symbol);
        self.writer.write_bits(LC_EXTRA[lcode], lbits_value);
        self.writer.write_bits(DC_EXTRA[dcode], dbits_value);
        Ok(())
    }

    fn encode_eof(&mut self) -> Result<()> {
        self.encode_match(IMPOSSIBLE_LEN as usize, IMPOSSIBLE_DIST as usize)
    }

    fn finish(self) -> Vec<u8> {
        self.writer.finish()
    }
}

struct BitWriter {
    out: Vec<u8>,
    bitbuf: u64,
    bitcount: u32,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            out: Vec::new(),
            bitbuf: 0,
            bitcount: 0,
        }
    }

    fn write_bits(&mut self, bits: u32, value: u32) {
        if bits == 0 {
            return;
        }
        debug_assert!(bits <= 32);
        let mask = if bits == 32 {
            u32::MAX
        } else {
            (1u32 << bits) - 1
        };
        self.bitbuf |= ((value & mask) as u64) << self.bitcount;
        self.bitcount += bits;

        while self.bitcount >= 32 {
            let word = self.bitbuf as u32;
            self.out.extend_from_slice(&word.to_le_bytes());
            self.bitbuf >>= 32;
            self.bitcount -= 32;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bitcount > 0 {
            let word = self.bitbuf as u32;
            self.out.extend_from_slice(&word.to_le_bytes());
        }
        self.out
    }
}

// ── HuffLzCoder ───────────────────────────────────────────────────────────────

struct HuffLzCoder {
    enc: Lz77HuffEncoder<Vec<u8>>,
    min_match: u32,
}

impl HuffLzCoder {
    fn new(min_match: usize) -> Self {
        Self {
            enc: Lz77HuffEncoder::new(Vec::new()),
            min_match: min_match as u32,
        }
    }
}

impl LzTokenCoder for HuffLzCoder {
    fn encode_literal(&mut self, b: u8) -> Result<()> {
        self.enc.encode_literal(b)
    }

    fn encode_match(&mut self, len: usize, dist: usize) -> Result<()> {
        self.enc.encode_match(len, dist, self.min_match)
    }

    fn encode_table(&mut self, type_n: usize, items: usize) -> Result<()> {
        self.enc.encode_table(type_n, items)
    }

    fn encode_eof(&mut self) -> Result<()> {
        self.enc.encode_eof(self.min_match)
    }

    fn finish(self) -> Vec<u8> {
        self.enc.finish().unwrap_or_default()
    }

    fn prevdist_last(&self) -> Option<u32> {
        let d = self.enc.prevdist_last();
        if d == 0 {
            None
        } else {
            Some(d)
        }
    }

    fn encode_repchar(&mut self) -> Result<()> {
        self.enc.encode_repchar()
    }
}

// ── ArithLzCoder ──────────────────────────────────────────────────────────────

struct ArithLzCoder {
    enc: Lz77ArithEncoder<Vec<u8>>,
    min_match: u32,
}

impl ArithLzCoder {
    fn new(min_match: usize) -> Self {
        Self {
            enc: Lz77ArithEncoder::new(Vec::new()),
            min_match: min_match as u32,
        }
    }
}

impl LzTokenCoder for ArithLzCoder {
    fn encode_literal(&mut self, b: u8) -> Result<()> {
        self.enc.encode_literal(b)
    }

    fn encode_match(&mut self, len: usize, dist: usize) -> Result<()> {
        self.enc.encode_match(len, dist, self.min_match)
    }

    fn encode_table(&mut self, type_n: usize, items: usize) -> Result<()> {
        self.enc.encode_table(type_n, items)
    }

    fn encode_eof(&mut self) -> Result<()> {
        self.enc.encode_eof(self.min_match)
    }

    fn finish(self) -> Vec<u8> {
        self.enc.finish().unwrap_or_default()
    }

    fn prevdist_last(&self) -> Option<u32> {
        let d = self.enc.prevdist_last();
        if d == 0 {
            None
        } else {
            Some(d)
        }
    }

    fn encode_repchar(&mut self) -> Result<()> {
        self.enc.encode_repchar()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
