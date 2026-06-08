//! PNG-style per-row predictor for uncompressed raster images.
//!
//! Implements the five PNG (RFC 2083 §6) row filters — None, Sub, Up, Average,
//! Paeth — applied per-row, with per-row optimal filter selection using the
//! minimum-sum-of-absolute-values heuristic (also from the PNG spec).
//!
//! ## When to use this
//!
//! For *uncompressed* raster formats — BMP, TIFF (no compression), TGA,
//! PPM/PGM/PBM, raw image dumps — paired with a back-end LZ coder. The filter
//! turns spatial correlation into near-zero residuals, which LZ/entropy coders
//! handle much better than the raw pixels.
//!
//! Not for: already-compressed images (PNG/JPEG/WebP — they did this already),
//! camera RAW Bayer data (use `RawBayerFilter` — it understands the CFA layout),
//! or PCM audio (use stride-2 `DeltaFilter`).
//!
//! ## Output layout
//!
//! For each row: 1 filter-type byte + `row_stride` filtered bytes.
//! Total output size is `input.len() + num_rows` bytes.  The expansion is
//! repaid many times over by the downstream coder's improved ratio on the
//! near-zero residual stream.
//!
//! ## Round-trip integrity
//!
//! `row_stride` and `bytes_per_pixel` must match between encode and decode.
//! These are baked into [`RowFilterOptions`] and travel with the [`Method`]
//! through the pipeline, so a single-codec roundtrip is safe.

use crate::codec::filters::Filter;
use crate::error::{ArcError, Result};

/// Options for the per-row predictor filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowFilterOptions {
    /// Bytes per row.  For BMP/TIFF this is the image width × bytes-per-pixel,
    /// rounded up to the format's row alignment (typically 4 bytes for BMP).
    pub row_stride: usize,
    /// Bytes per pixel — used by Sub/Average/Paeth filters to find the "left"
    /// neighbour.  Typical values: 1 (greyscale8), 2 (greyscale16/RGB565), 3 (RGB24),
    /// 4 (RGBA32 / BGRA32).
    pub bytes_per_pixel: usize,
}

impl Default for RowFilterOptions {
    fn default() -> Self {
        // Sensible default for a 1024-wide RGBA image — callers should override.
        Self {
            row_stride: 4096,
            bytes_per_pixel: 4,
        }
    }
}

/// PNG row filter types (RFC 2083 §6).
const FILTER_NONE: u8 = 0;
const FILTER_SUB: u8 = 1;
const FILTER_UP: u8 = 2;
const FILTER_AVERAGE: u8 = 3;
const FILTER_PAETH: u8 = 4;

pub struct RowFilter {
    options: RowFilterOptions,
}

impl RowFilter {
    pub fn new(options: RowFilterOptions) -> Result<Self> {
        if options.row_stride == 0 {
            return Err(ArcError::InvalidMethod(
                "row_filter: row_stride must be > 0".to_string(),
            ));
        }
        if options.bytes_per_pixel == 0 || options.bytes_per_pixel > options.row_stride {
            return Err(ArcError::InvalidMethod(format!(
                "row_filter: bytes_per_pixel must be 1..=row_stride, got {}",
                options.bytes_per_pixel
            )));
        }
        Ok(Self { options })
    }

    pub fn options(&self) -> RowFilterOptions {
        self.options
    }
}

impl Filter for RowFilter {
    fn name(&self) -> &'static str {
        "row_filter"
    }

    fn encode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        let stride = self.options.row_stride;
        let bpp = self.options.bytes_per_pixel;

        // Tail-row handling: if input length isn't a multiple of stride, the
        // final partial row is emitted with filter=None to keep the round-trip
        // exact.  This happens for non-rectangular synthetic fixtures and
        // tail-padded image formats.
        let full_rows = input.len() / stride;
        let tail_len = input.len() - full_rows * stride;

        output.reserve(input.len() + full_rows + if tail_len > 0 { 1 } else { 0 });

        let mut prev_row: Option<&[u8]> = None;
        // Per-row scratch for the candidate filter outputs.
        let mut candidate = vec![0u8; stride];

        for row_idx in 0..full_rows {
            let row_start = row_idx * stride;
            let row = &input[row_start..row_start + stride];

            // Pick the best filter for this row using the minimum-sum-of-absolute
            // (signed-as-i8) values heuristic from PNG spec §12.8.
            let (best_filter, best_bytes) = pick_best_filter(row, prev_row, bpp, &mut candidate);

            output.push(best_filter);
            output.extend_from_slice(best_bytes);

            // Save the *unfiltered* row for the next iteration's Up/Paeth lookups.
            prev_row = Some(row);
        }

        if tail_len > 0 {
            output.push(FILTER_NONE);
            output.extend_from_slice(&input[full_rows * stride..]);
        }

        Ok(())
    }

    fn decode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        let stride = self.options.row_stride;
        let bpp = self.options.bytes_per_pixel;
        let start = output.len();

        let mut i = 0;
        while i < input.len() {
            let filter_byte = input[i];
            i += 1;
            let remaining = input.len() - i;
            let row_len = remaining.min(stride);
            let row = &input[i..i + row_len];
            i += row_len;

            // Decode this row, appending to output.
            let prev_row_start = if output.len() >= start + stride {
                Some(output.len() - stride)
            } else {
                None
            };

            // Buffer the decoded row in `output` directly.
            let row_decoded_start = output.len();
            output.reserve(row_len);
            // We use a closure to access `prev_row` without borrowing conflicts.
            for (j, &b) in row.iter().enumerate() {
                let left = if j >= bpp && row_len == stride {
                    output[row_decoded_start + j - bpp]
                } else {
                    0
                };
                let up = if let Some(p) = prev_row_start {
                    input_or_zero(output, p + j, row_len == stride)
                } else {
                    0
                };
                let up_left =
                    if let (Some(p), true) = (prev_row_start, j >= bpp && row_len == stride) {
                        output[p + j - bpp]
                    } else {
                        0
                    };

                let recovered = match filter_byte {
                    FILTER_NONE => b,
                    FILTER_SUB => b.wrapping_add(left),
                    FILTER_UP => b.wrapping_add(up),
                    FILTER_AVERAGE => b.wrapping_add(((left as u16 + up as u16) / 2) as u8),
                    FILTER_PAETH => b.wrapping_add(paeth_predictor(left, up, up_left)),
                    _ => {
                        return Err(ArcError::Codec {
                            codec: "row_filter",
                            message: format!("unknown row filter byte: {filter_byte}"),
                        })
                    }
                };
                output.push(recovered);
            }
        }
        Ok(())
    }
}

/// Lookup helper: returns `vec[idx]` if `valid_row`, else 0 (synthetic top-row
/// neighbour).  We split this out so the borrow checker doesn't complain about
/// taking `&output` while we're also pushing into `output` in the encode loop.
#[inline]
fn input_or_zero(vec: &[u8], idx: usize, valid_row: bool) -> u8 {
    if valid_row && idx < vec.len() {
        vec[idx]
    } else {
        0
    }
}

/// Try all five filters, return `(best_type_byte, slice_of_best_candidate)`.
///
/// `scratch` is reused across rows — must be ≥ `row.len()`.
fn pick_best_filter<'s>(
    row: &[u8],
    prev_row: Option<&[u8]>,
    bpp: usize,
    scratch: &'s mut [u8],
) -> (u8, &'s [u8]) {
    let n = row.len();
    debug_assert!(scratch.len() >= n);

    // Filter NONE: output = row.  Sum-of-abs is sum of |as_i8| over row.
    let none_score = sum_abs_signed(row);

    // We try the other four into `scratch`, keep track of best.
    let mut best_type = FILTER_NONE;
    let mut best_score = none_score;

    // Sub: row[i] - row[i-bpp]
    apply_sub(row, bpp, scratch);
    let score = sum_abs_signed(&scratch[..n]);
    if score < best_score {
        best_score = score;
        best_type = FILTER_SUB;
    }

    // Up: row[i] - prev_row[i]
    if let Some(prev) = prev_row {
        let mut buf = vec![0u8; n];
        apply_up(row, prev, &mut buf);
        let score = sum_abs_signed(&buf);
        if score < best_score {
            best_score = score;
            best_type = FILTER_UP;
            scratch[..n].copy_from_slice(&buf);
        }

        // Average: row[i] - (left + up) / 2
        let mut buf = vec![0u8; n];
        apply_average(row, prev, bpp, &mut buf);
        let score = sum_abs_signed(&buf);
        if score < best_score {
            best_score = score;
            best_type = FILTER_AVERAGE;
            scratch[..n].copy_from_slice(&buf);
        }

        // Paeth: row[i] - paeth(left, up, up_left)
        let mut buf = vec![0u8; n];
        apply_paeth(row, prev, bpp, &mut buf);
        let score = sum_abs_signed(&buf);
        if score < best_score {
            best_score = score;
            best_type = FILTER_PAETH;
            scratch[..n].copy_from_slice(&buf);
        }
    }

    let _ = best_score;
    if best_type == FILTER_NONE {
        // Materialize the unchanged row into scratch so the returned slice has
        // the right lifetime.  This costs one row-copy per row but keeps the
        // borrow checker happy with a single return type.
        scratch[..n].copy_from_slice(row);
    }
    // SUB / UP / AVERAGE / PAETH all wrote their winning buffer into scratch above.
    (best_type, &scratch[..n])
}

#[inline]
fn sum_abs_signed(buf: &[u8]) -> u64 {
    buf.iter().map(|&b| ((b as i8).unsigned_abs()) as u64).sum()
}

fn apply_sub(row: &[u8], bpp: usize, out: &mut [u8]) {
    for i in 0..row.len() {
        let left = if i >= bpp { row[i - bpp] } else { 0 };
        out[i] = row[i].wrapping_sub(left);
    }
}

fn apply_up(row: &[u8], prev: &[u8], out: &mut [u8]) {
    for i in 0..row.len() {
        out[i] = row[i].wrapping_sub(prev[i]);
    }
}

fn apply_average(row: &[u8], prev: &[u8], bpp: usize, out: &mut [u8]) {
    for i in 0..row.len() {
        let left = if i >= bpp { row[i - bpp] } else { 0 };
        let up = prev[i];
        let avg = ((left as u16 + up as u16) / 2) as u8;
        out[i] = row[i].wrapping_sub(avg);
    }
}

fn apply_paeth(row: &[u8], prev: &[u8], bpp: usize, out: &mut [u8]) {
    for i in 0..row.len() {
        let left = if i >= bpp { row[i - bpp] } else { 0 };
        let up = prev[i];
        let up_left = if i >= bpp { prev[i - bpp] } else { 0 };
        out[i] = row[i].wrapping_sub(paeth_predictor(left, up, up_left));
    }
}

/// PNG Paeth predictor (RFC 2083 §6.6) — picks whichever of a/b/c minimises
/// the linear prediction error |a + b - c - x| for each of x = a, b, c.
#[inline]
fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
