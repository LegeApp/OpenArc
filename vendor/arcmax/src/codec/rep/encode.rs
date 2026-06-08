use std::io::{Read, Write};

use crate::codec::rep::options::RepOptions;
use crate::error::{ArcError, Result};

// ── Constants ─────────────────────────────────────────────────────────────────

const PRIME: u32 = 153191;
const MAX_BLOCK: usize = 8 * 1024 * 1024; // 8 MiB — max data encoded per output chunk

#[derive(Debug, Clone, Copy)]
struct RepMatch {
    literal_start: usize,
    literal_len: usize,
    offset: usize,
    len: usize,
}

fn codec_err(msg: impl Into<String>) -> ArcError {
    ArcError::Codec {
        codec: "rep",
        message: msg.into(),
    }
}

// ── Integer helpers ───────────────────────────────────────────────────────────

/// Largest power of 2 ≤ n.  rounddown_pow2(0) == 1 (mirrors C++ `rounddown_to_power_of`).
pub(crate) fn rounddown_pow2(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    1usize << (usize::BITS - 1 - n.leading_zeros())
}

/// Smallest power of 2 ≥ n.  roundup_pow2(0) == 0 (mirrors C++ `roundup_to_power_of`).
pub(crate) fn roundup_pow2(n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 1;
    }
    1usize << (usize::BITS - (n - 1).leading_zeros())
}

/// PRIME^exp mod 2^32 (wrapping).
pub(crate) fn power_u32(base: u32, exp: usize) -> u32 {
    let mut result: u32 = 1;
    let mut b = base;
    let mut e = exp;
    while e > 0 {
        if e & 1 != 0 {
            result = result.wrapping_mul(b);
        }
        b = b.wrapping_mul(b);
        e >>= 1;
    }
    result
}

// ── Hash parameter calculation ────────────────────────────────────────────────

/// Returns `(hash_size, L)` from `opts` — mirrors `CalcHashSize` in rep.cpp.
///
/// `L` is the rolling-hash block length (≥1).
/// `hash_size` is always a power of 2 and ≥1.
pub(crate) fn calc_hash_params(opts: &RepOptions) -> (usize, usize) {
    let smallest = (opts.smallest_len.min(opts.min_match_len)) as usize;
    let l: usize = if opts.chunk_size > 0 {
        opts.chunk_size
    } else {
        rounddown_pow2(smallest.saturating_sub(1)) / 2
    }
    .max(1);

    let hash_size: usize = if opts.hash_size_log > 0 {
        1usize << opts.hash_size_log
    } else {
        // roundup_to_power_of(BlockSize/3*2, 2) / max(L/4, 16)
        let numerator = roundup_pow2(opts.block_size / 3 * 2);
        numerator / l.div_euclid(4).max(16)
    };

    (roundup_pow2(hash_size).max(1), l)
}

// ── Rolling hash helpers ──────────────────────────────────────────────────────

/// Single rolling-hash step: remove `sub`, add `add`.
///
/// `hash = hash * PRIME + add - sub * PRIME^L`
#[inline(always)]
fn update_hash(hash: u32, sub: u8, add: u8, prime_power_l: u32) -> u32 {
    hash.wrapping_mul(PRIME)
        .wrapping_add(add as u32)
        .wrapping_sub((sub as u32).wrapping_mul(prime_power_l))
}

/// Compute the initial hash for `buf[start .. start+l]`.
fn initial_hash(buf: &[u8], start: usize, l: usize) -> u32 {
    let mut h = 0u32;
    for i in 0..l {
        h = h.wrapping_mul(PRIME).wrapping_add(buf[start + i] as u32);
    }
    h
}

/// Within `buf[start .. start+l]`, find the position with the maximum rolling
/// hash and return `(max_hash, offset_within_block)`.
///
/// Mirrors `Job::process()` from rep.cpp (single-threaded version).
/// Requires `buf` to have at least `start + 2*l` bytes available.
pub(crate) fn max_hash_in_block(
    buf: &[u8],
    start: usize,
    l: usize,
    prime_power_l: u32,
) -> (u32, usize) {
    // Initial hash = hash(buf[start .. start+l])
    let mut hash = initial_hash(buf, start, l);
    let mut max_hash = hash;
    let mut max_offset = 0usize;

    // Slide the window across the block; at step i the window is
    // buf[start+i .. start+i+l].
    for i in 0..l {
        if hash > max_hash {
            max_hash = hash;
            max_offset = i;
        }
        if i + 1 < l {
            hash = update_hash(hash, buf[start + i], buf[start + i + l], prime_power_l);
        }
    }
    (max_hash, max_offset)
}

// ── Match boundary helpers ────────────────────────────────────────────────────

/// Walk backward from `(match_pos, cur_pos)` while bytes match, stopping at
/// `lower`.  Returns the new `cur_pos` (start of the match region).
///
/// Mirrors `find_match_start` in rep.cpp.
pub(crate) fn find_match_start(
    buf: &[u8],
    match_pos: usize,
    cur_pos: usize,
    lower: usize,
) -> usize {
    let mut p = match_pos;
    let mut q = cur_pos;
    while q > lower && p > 0 {
        if buf[p - 1] != buf[q - 1] {
            return q;
        }
        p -= 1;
        q -= 1;
    }
    q
}

/// Walk forward from `(match_pos, cur_pos)` while bytes match, stopping at
/// `upper`.  Returns the new `cur_pos` (end of the match region, exclusive).
///
/// Mirrors `find_match_end` in rep.cpp.
pub(crate) fn find_match_end(buf: &[u8], match_pos: usize, cur_pos: usize, upper: usize) -> usize {
    let mut p = match_pos;
    let mut q = cur_pos;
    while q < upper && buf[p] == buf[q] {
        p += 1;
        q += 1;
    }
    q
}

// ── I/O helpers ───────────────────────────────────────────────────────────────

pub(crate) fn write_u32_le(w: &mut dyn Write, v: u32) -> Result<()> {
    w.write_all(&v.to_le_bytes()).map_err(ArcError::Io)
}

pub(crate) fn write_i32_le(w: &mut dyn Write, v: i32) -> Result<()> {
    w.write_all(&v.to_le_bytes()).map_err(ArcError::Io)
}

// ── Block writer ──────────────────────────────────────────────────────────────

fn checked_i32(v: usize, what: &str) -> Result<i32> {
    i32::try_from(v).map_err(|_| codec_err(format!("REP {what} exceeds i32 range: {v}")))
}

fn checked_u32(v: usize, what: &str) -> Result<u32> {
    u32::try_from(v).map_err(|_| codec_err(format!("REP {what} exceeds u32 range: {v}")))
}

fn write_rep_chunk(
    output: &mut dyn Write,
    buf: &[u8],
    matches: &[RepMatch],
    tail_start: usize,
    tail_len: usize,
) -> Result<u64> {
    let num = matches.len();
    let literal_bytes = matches
        .iter()
        .map(|m| m.literal_len)
        .sum::<usize>()
        .saturating_add(tail_len);
    let compr_size = 4usize
        .saturating_add(num.saturating_mul(4))
        .saturating_add(num.saturating_mul(4))
        .saturating_add((num + 1).saturating_mul(4))
        .saturating_add(literal_bytes);

    write_u32_le(output, checked_u32(compr_size, "chunk size")?)?;
    write_i32_le(output, checked_i32(num, "match count")?)?;

    for m in matches {
        write_i32_le(output, checked_i32(m.len, "match length")?)?;
    }
    for m in matches {
        write_i32_le(output, checked_i32(m.offset, "match offset")?)?;
    }
    for m in matches {
        write_i32_le(output, checked_i32(m.literal_len, "literal length")?)?;
    }
    write_i32_le(output, checked_i32(tail_len, "literal length")?)?;

    for m in matches {
        output
            .write_all(&buf[m.literal_start..m.literal_start + m.literal_len])
            .map_err(ArcError::Io)?;
    }
    output
        .write_all(&buf[tail_start..tail_start + tail_len])
        .map_err(ArcError::Io)?;

    Ok(4 + compr_size as u64)
}

fn encode_data_block(buf: &[u8], output: &mut dyn Write, opts: &RepOptions) -> Result<u64> {
    if buf.is_empty() {
        return Ok(0);
    }

    let (hash_size, l) = calc_hash_params(opts);
    if l == 0 || hash_size == 0 {
        return Err(codec_err("REP hash parameters resolved to zero"));
    }

    let hash_mask = hash_size - 1;
    let prime_power_l = power_u32(PRIME, l);
    let mut hasharr = vec![usize::MAX; hash_size];
    let mut bytes_out = 0u64;

    let min_match_len = opts.min_match_len.max(1) as usize;
    let smallest_len = opts.smallest_len.max(1).min(opts.min_match_len.max(1)) as usize;
    let barrier = opts.barrier.max(0) as usize;

    let mut indexed_pos = 0usize;
    let mut emitted_pos = 0usize;

    while emitted_pos < buf.len() {
        let flush_end = (emitted_pos + MAX_BLOCK).min(buf.len());
        let mut matches = Vec::new();

        while indexed_pos + (2 * l) <= buf.len() && indexed_pos + l < flush_end {
            let (hash, best_offset) = max_hash_in_block(buf, indexed_pos, l, prime_power_l);
            let i = indexed_pos + best_offset;
            let slot = (hash as usize) & hash_mask;

            if i >= emitted_pos {
                let match_pos = hasharr[slot];
                if match_pos != usize::MAX && match_pos < i {
                    let offset = i - match_pos;
                    if offset <= opts.block_size {
                        let lower = emitted_pos.max(offset);
                        let upper = flush_end.min(buf.len());
                        let start = find_match_start(buf, match_pos, i, lower);
                        let end = find_match_end(buf, match_pos, i, upper);
                        let threshold = if offset < barrier {
                            min_match_len
                        } else {
                            smallest_len
                        };

                        if end - start >= threshold {
                            matches.push(RepMatch {
                                literal_start: emitted_pos,
                                literal_len: start - emitted_pos,
                                offset,
                                len: end - start,
                            });
                            emitted_pos = end;
                        }
                    }
                }
            }

            hasharr[slot] = i;
            indexed_pos += l;
        }

        let tail_start = emitted_pos;
        let tail_len = flush_end - emitted_pos;
        bytes_out += write_rep_chunk(output, buf, &matches, tail_start, tail_len)?;
        emitted_pos = flush_end;
    }

    Ok(bytes_out)
}

/// REP encoder — encodes `input` into the REP wire format and writes to `output`.
pub fn rep_compress(
    input: &mut dyn Read,
    output: &mut dyn Write,
    opts: &RepOptions,
) -> Result<u64> {
    if opts.block_size == 0 {
        return Err(codec_err("REP block size must be non-zero"));
    }
    checked_u32(opts.block_size, "block size")?;
    checked_i32(opts.block_size, "block size")?;

    write_u32_le(output, opts.block_size as u32)?;
    let mut bytes_out = 4u64;
    let mut block = vec![0u8; opts.block_size.min(MAX_BLOCK)];

    loop {
        let mut filled = 0usize;
        while filled < opts.block_size {
            if filled == block.len() {
                block.resize((block.len() * 2).min(opts.block_size), 0);
            }
            let read_end = opts.block_size.min(block.len());
            let n = input.read(&mut block[filled..read_end])?;
            if n == 0 {
                break;
            }
            filled += n;
        }

        if filled == 0 {
            break;
        }

        bytes_out += encode_data_block(&block[..filled], output, opts)?;

        if filled < opts.block_size {
            break;
        }
    }

    write_u32_le(output, 0)?;
    Ok(bytes_out + 4)
}

// ── Tests ─────────────────────────────────────────────────────────────────────
