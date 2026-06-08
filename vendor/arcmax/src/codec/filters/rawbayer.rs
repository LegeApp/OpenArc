use crate::codec::filters::Filter;
use crate::error::{ArcError, Result};

/// Byte-split + Bayer horizontal delta-2 filter for camera RAW sensor data.
///
/// Encode: deinterleave high/low bytes of each 16-bit pixel, then subtract
/// the pixel two positions back (same Bayer color channel) within each half.
/// Decode: undo delta, then re-interleave.
///
/// Requires even-length input (whole 16-bit pixels). Fails fast on odd length,
/// since that indicates misaligned or non-RAW data where this filter has no
/// business being applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawBayerOptions;

impl Default for RawBayerOptions {
    fn default() -> Self {
        Self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RawBayerFilter;

impl RawBayerFilter {
    pub fn new(_opts: RawBayerOptions) -> Self {
        Self
    }
}

impl Filter for RawBayerFilter {
    fn name(&self) -> &'static str {
        "rawbayer"
    }

    fn encode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        let n = input.len();
        if n % 2 != 0 {
            return Err(ArcError::InvalidMethod(
                "rawbayer: input length must be even (16-bit pixels)".to_string(),
            ));
        }
        let npix = n / 2;
        output.reserve(n);

        // Byte-split: high bytes [0..npix), low bytes [npix..n)
        let mut hi = vec![0u8; npix];
        let mut lo = vec![0u8; npix];
        for i in 0..npix {
            hi[i] = input[i * 2 + 1]; // big-end byte of LE u16
            lo[i] = input[i * 2];
        }

        // Delta-2 (Bayer-aware: same color channel skip) within each half
        bayer_delta_encode(&mut hi);
        bayer_delta_encode(&mut lo);

        output.extend_from_slice(&hi);
        output.extend_from_slice(&lo);
        Ok(())
    }

    fn decode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        let n = input.len();
        if n % 2 != 0 {
            return Err(ArcError::InvalidMethod(
                "rawbayer: encoded length must be even".to_string(),
            ));
        }
        let npix = n / 2;
        output.reserve(n);

        let mut hi = input[..npix].to_vec();
        let mut lo = input[npix..].to_vec();

        bayer_delta_decode(&mut hi);
        bayer_delta_decode(&mut lo);

        for i in 0..npix {
            output.push(lo[i]);
            output.push(hi[i]);
        }
        Ok(())
    }
}

/// In-place horizontal delta with skip-2 (same Bayer color channel).
/// Positions 0 and 1 are kept as-is (first pixel of each channel).
fn bayer_delta_encode(data: &mut [u8]) {
    // Walk backwards so we don't corrupt values we still need.
    // Skip positions 0 and 1 — they have no predecessor in their channel.
    for i in (2..data.len()).rev() {
        data[i] = data[i].wrapping_sub(data[i - 2]);
    }
}

fn bayer_delta_decode(data: &mut [u8]) {
    for i in 2..data.len() {
        data[i] = data[i].wrapping_add(data[i - 2]);
    }
}
