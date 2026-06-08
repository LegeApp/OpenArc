//! Linear Predictive Coding (LPC) audio pre-filter.
//!
//! PCM audio has strong short-range correlation. The plain `delta` filter
//! captures only first-order differences. FLAC-style LPC uses up to 4th-order
//! fixed linear prediction, computing residuals `r[i] = s[i] - predict(s, i)`
//! where the predictor uses the last `order` decoded samples.
//!
//! This filter operates on 16-bit little-endian PCM. It selects the best-fit
//! order (0..=4) per `BLOCK_SAMPLES`-sample block, then emits a 1-byte block
//! header (the chosen order) followed by the residuals as signed 16-bit LE.
//!
//! The residuals typically have low absolute values, which PPMd/LZMA model
//! much more efficiently than the original signal.

use crate::codec::filters::Filter;
use crate::error::{ArcError, Result};

/// Maximum LPC order supported.
const MAX_ORDER: usize = 4;
/// Samples per encoding block for order selection.
const BLOCK_SAMPLES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LpcOptions {
    /// Maximum prediction order to evaluate (1..=4). Higher = better compression.
    pub max_order: usize,
}

impl Default for LpcOptions {
    fn default() -> Self {
        Self {
            max_order: MAX_ORDER,
        }
    }
}

pub struct LpcFilter {
    opts: LpcOptions,
}

impl LpcFilter {
    pub fn new(opts: LpcOptions) -> Result<Self> {
        if opts.max_order == 0 || opts.max_order > MAX_ORDER {
            return Err(ArcError::InvalidMethod(format!(
                "LPC: max_order must be 1..={MAX_ORDER}, got {}",
                opts.max_order
            )));
        }
        Ok(Self { opts })
    }
}

impl Filter for LpcFilter {
    fn name(&self) -> &'static str {
        "lpc"
    }

    fn encode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        if input.len() % 2 != 0 {
            return Err(ArcError::Codec {
                codec: "lpc",
                message: "input must be 16-bit LE PCM (even byte count)".into(),
            });
        }

        let samples: Vec<i16> = input
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();

        output.reserve(input.len() + samples.len() / BLOCK_SAMPLES + 1);

        for block in samples.chunks(BLOCK_SAMPLES) {
            let order = pick_order(block, self.opts.max_order);
            output.push(order as u8);
            encode_block(block, order, output);
        }

        Ok(())
    }

    fn decode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        if input.is_empty() {
            return Ok(());
        }

        output.reserve(input.len());
        let mut pos = 0usize;

        while pos < input.len() {
            let order = input[pos] as usize;
            pos += 1;

            if order > MAX_ORDER {
                return Err(ArcError::Codec {
                    codec: "lpc",
                    message: format!("invalid LPC order byte {order} in stream"),
                });
            }

            // Consume at most BLOCK_SAMPLES samples (each 2 bytes of residuals).
            let remaining_samples = (input.len() - pos) / 2;
            let n = remaining_samples.min(BLOCK_SAMPLES);
            if n == 0 {
                break;
            }

            let residuals: Vec<i16> = input[pos..pos + n * 2]
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
            pos += n * 2;

            decode_block(&residuals, order, output);
        }

        Ok(())
    }
}

// --- FLAC fixed-predictor coefficients ---------------------------------------

/// `COEFFS[order]` = integer coefficients for the FLAC fixed predictor of that order.
/// Prediction: `p = Σ COEFFS[order][k] * history[n-1-k]`
const COEFFS: [&[i32]; 5] = [
    &[],             // order 0: predict 0
    &[1],            // order 1: p = s[i-1]
    &[2, -1],        // order 2: p = 2s[i-1] - s[i-2]
    &[3, -3, 1],     // order 3: p = 3s[i-1] - 3s[i-2] + s[i-3]
    &[4, -6, 4, -1], // order 4: p = 4s[i-1] - 6s[i-2] + 4s[i-3] - s[i-4]
];

fn predict(history: &[i16], order: usize) -> i16 {
    if order == 0 || history.is_empty() {
        return 0;
    }
    let coeffs = COEFFS[order];
    let len = history.len();
    let mut p: i32 = 0;
    for (k, &c) in coeffs.iter().enumerate() {
        if k >= len {
            break;
        }
        p += c * (history[len - 1 - k] as i32);
    }
    p.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn pick_order(block: &[i16], max_order: usize) -> usize {
    let mut best_order = 0usize;
    let mut best_cost = u64::MAX;

    for order in 0..=max_order {
        let cost: u64 = block
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let start = i.saturating_sub(order);
                let p = predict(&block[start..i], order);
                (s as i32 - p as i32).unsigned_abs() as u64
            })
            .sum();
        if cost < best_cost {
            best_cost = cost;
            best_order = order;
        }
    }

    best_order
}

fn encode_block(block: &[i16], order: usize, output: &mut Vec<u8>) {
    for (i, &s) in block.iter().enumerate() {
        let start = i.saturating_sub(order);
        let p = predict(&block[start..i], order);
        let r = (s as i32 - p as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        output.extend_from_slice(&r.to_le_bytes());
    }
}

fn decode_block(residuals: &[i16], order: usize, output: &mut Vec<u8>) {
    let mut decoded: Vec<i16> = Vec::with_capacity(residuals.len());
    for &r in residuals {
        let start = decoded.len().saturating_sub(order);
        let p = predict(&decoded[start..], order);
        let s = (r as i32 + p as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        decoded.push(s);
        output.extend_from_slice(&s.to_le_bytes());
    }
}
