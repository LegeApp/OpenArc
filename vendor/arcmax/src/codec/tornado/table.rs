// Table preprocessing for Tornado: numeric-sequence delta coding.
//
// The encoder detects monotone numeric sequences (N=2 or N=4 bytes per element)
// and applies a delta transform so the LZ77 coder sees smaller values.
// The decoder inverts the transform after full decompression.
//
// Wire protocol: the encoder emits a table command (special match with
// len = IMPOSSIBLE_LEN + N, dist = items - 1) just before the LZ-encoded
// delta-transformed table data.  The decoder records the output position,
// decompresses the table data normally, then applies undiff_table.

// ── Types ─────────────────────────────────────────────────────────────────────

/// A detected numeric table region in the encoder's input buffer.
#[derive(Debug, Clone, Copy)]
pub(super) struct TableRegion {
    /// Byte offset of element 0 in the buffer.
    pub start: usize,
    /// Bytes per element (2 or 4).
    pub n: usize,
    /// Number of elements.
    pub items: usize,
}

// ── diff / undiff ─────────────────────────────────────────────────────────────

/// In-place LE delta forward transform: element[i] -= element[i-1] for i > 0.
pub(super) fn diff_table(buf: &mut [u8], start: usize, n: usize, items: usize) {
    if items <= 1 {
        return;
    }
    let end = start + n * items;
    debug_assert!(end <= buf.len());
    match n {
        2 => {
            let mut prev = u16::from_le_bytes([buf[start], buf[start + 1]]);
            let mut r = start + 2;
            while r + 2 <= end {
                let cur = u16::from_le_bytes([buf[r], buf[r + 1]]);
                let diff = cur.wrapping_sub(prev);
                buf[r] = diff as u8;
                buf[r + 1] = (diff >> 8) as u8;
                prev = cur;
                r += 2;
            }
        }
        4 => {
            let mut prev = u32::from_le_bytes(buf[start..start + 4].try_into().unwrap());
            let mut r = start + 4;
            while r + 4 <= end {
                let cur = u32::from_le_bytes(buf[r..r + 4].try_into().unwrap());
                let diff = cur.wrapping_sub(prev);
                buf[r..r + 4].copy_from_slice(&diff.to_le_bytes());
                prev = cur;
                r += 4;
            }
        }
        _ => {
            // General case: iterate backwards so we don't clobber inputs.
            let mut r = end;
            while r > start + n {
                r -= n;
                let mut carry: i32 = 0;
                for i in 0..n {
                    let a = buf[r + i] as i32;
                    let b = buf[r + i - n] as i32;
                    let new_carry = ((a - b - carry) < 0) as i32;
                    buf[r + i] = (a - b - carry).rem_euclid(256) as u8;
                    carry = new_carry;
                }
            }
        }
    }
}

/// In-place LE delta inverse: element[i] += element[i-1] for i > 0.
pub(super) fn undiff_table(buf: &mut [u8], start: usize, n: usize, items: usize) {
    if items <= 1 {
        return;
    }
    let end = start + n * items;
    debug_assert!(end <= buf.len());
    match n {
        2 => {
            let mut prev = u16::from_le_bytes([buf[start], buf[start + 1]]);
            let mut r = start + 2;
            while r + 2 <= end {
                let diff = u16::from_le_bytes([buf[r], buf[r + 1]]);
                let cur = prev.wrapping_add(diff);
                buf[r] = cur as u8;
                buf[r + 1] = (cur >> 8) as u8;
                prev = cur;
                r += 2;
            }
        }
        4 => {
            let mut prev = u32::from_le_bytes(buf[start..start + 4].try_into().unwrap());
            let mut r = start + 4;
            while r + 4 <= end {
                let diff = u32::from_le_bytes(buf[r..r + 4].try_into().unwrap());
                let cur = prev.wrapping_add(diff);
                buf[r..r + 4].copy_from_slice(&cur.to_le_bytes());
                prev = cur;
                r += 4;
            }
        }
        _ => {
            let mut r = start;
            while r + n < end {
                r += n;
                let mut carry: i32 = 0;
                for i in 0..n {
                    let temp = buf[r + i] as i32 + buf[r + i - n] as i32 + carry;
                    buf[r + i] = temp as u8;
                    carry = temp >> 8;
                }
            }
        }
    }
}

// ── Table detection (encoder only) ────────────────────────────────────────────

/// floor(log2(x)), returns 0 for x == 0.
fn lb(x: u32) -> u32 {
    if x == 0 {
        0
    } else {
        31u32.saturating_sub(x.leading_zeros())
    }
}

/// Port of FreeArc `check_for_data_table`.
///
/// Scans `buf[start..]` with stride `n` using signed-16-bit heuristics to
/// detect a monotone numeric sequence.  Returns the element count if a table
/// of at least 40 elements is found, otherwise `None`.
fn find_table_n(buf: &[u8], start: usize, n: usize) -> Option<usize> {
    if n < 2 || buf.len() < start + n * 42 {
        return None;
    }

    let read_i16 = |pos: usize| -> i32 { i16::from_le_bytes([buf[pos], buf[pos + 1]]) as i32 };

    // Direction: sign of the first differential (element 1 vs element 0).
    let init_diff = read_i16(start + n) - read_i16(start);
    let mut dir: i32 = if init_diff < 0 { -1 } else { 1 };

    let mut lensum: i32 = 600;
    let mut len: i32 = 0;
    let mut lenminus: i32 = 0;
    let mut lastpoint = start + n; // last position where a "good" run ended

    let mut t = start + n;

    while t + 2 <= buf.len() {
        let cur = read_i16(t);
        let prev = read_i16(t - n);
        let diff = cur - prev;

        let itemlb = lb(1 + cur.unsigned_abs());
        let difflb = lb(1 + diff.unsigned_abs());
        let itemlb_adj = itemlb.saturating_sub(if itemlb > 10 { 1 } else { 0 });

        if dir < 0 && diff < 0 && difflb < itemlb_adj {
            len += 1;
        } else if dir > 0 && diff > 0 && difflb < itemlb_adj {
            len += 1;
        } else if diff == 0 {
            lenminus += 1;
        } else {
            if len > 3 {
                lastpoint = t;
            }
            lensum = lensum - lensum / 8 + len.min(10) * 30;
            dir = if diff < 0 { -1 } else { 1 };
            len = 0;
            if lensum < 500 {
                break;
            }
        }

        t += n;
    }

    let span = (t - start) as i64;
    let lastspan = (lastpoint - start) as i64;
    let threshold = n as i64 * (40 + lenminus as i64);
    let min_span = n.max(20) as i64;

    if span > threshold && lastspan > min_span {
        let items = (lastpoint - start) / n;
        if items > 1 {
            Some(items)
        } else {
            None
        }
    } else {
        None
    }
}

/// Scan `buf` for numeric table regions, apply `diff_table` in-place, and
/// return all detected regions sorted by start position.
///
/// Called by the encoder before the LZ parsing pass.
pub(super) fn detect_and_apply_tables(buf: &mut [u8]) -> Vec<TableRegion> {
    let mut regions: Vec<TableRegion> = Vec::new();

    for n in [2usize, 4] {
        let min_bytes = n * 42;
        let mut pos = 0usize;

        while pos + min_bytes <= buf.len() {
            // Skip positions already covered by a previously detected region.
            if regions
                .iter()
                .any(|r| pos >= r.start && pos < r.start + r.n * r.items)
            {
                pos += n;
                continue;
            }

            if let Some(items) = find_table_n(buf, pos, n) {
                diff_table(buf, pos, n, items);
                regions.push(TableRegion {
                    start: pos,
                    n,
                    items,
                });
                pos += n * items;
            } else {
                pos += n;
            }
        }
    }

    regions.sort_by_key(|r| r.start);
    regions
}

// ── Unit tests ─────────────────────────────────────────────────────────────────
