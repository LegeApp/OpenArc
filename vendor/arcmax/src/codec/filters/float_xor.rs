//! Float XOR-difference predictor filter.
//!
//! Scientific data (HDF5, NetCDF, NumPy, Parquet float columns) consists of
//! slowly-varying IEEE-754 floats. XOR-differencing consecutive samples yields
//! residuals with many leading zero bits, which BSC/PPMd/LZMA compress
//! dramatically better than the raw bytes.
//!
//! ## Encode
//!
//! ```text
//! out[0..stride*eb]         = in[0..stride*eb]       (verbatim)
//! out[i*eb..(i+1)*eb]       = in[i*eb..(i+1)*eb]
//!                             XOR in[(i-stride)*eb..(i-stride+1)*eb]
//! ```
//!
//! ## Decode (inverse XOR)
//!
//! ```text
//! out[0..stride*eb]         = in[0..stride*eb]
//! out[i*eb..(i+1)*eb]       = in[i*eb..(i+1)*eb]
//!                             XOR out[(i-stride)*eb..(i-stride+1)*eb]
//! ```

use crate::codec::filters::Filter;
use crate::error::{ArcError, Result};

/// Precision of the float samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatPrecision {
    F32,
    F64,
}

impl FloatPrecision {
    pub fn bytes(self) -> usize {
        match self {
            FloatPrecision::F32 => 4,
            FloatPrecision::F64 => 8,
        }
    }
}

/// Options for `FloatXorFilter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatXorOptions {
    /// Float element width.
    pub precision: FloatPrecision,
    /// Differencing stride in *elements* (not bytes). 1 = consecutive samples;
    /// N = per-channel for N-channel interleaved data.
    pub stride: usize,
}

impl Default for FloatXorOptions {
    fn default() -> Self {
        Self {
            precision: FloatPrecision::F32,
            stride: 1,
        }
    }
}

/// XOR-difference filter for packed IEEE-754 float arrays.
pub struct FloatXorFilter {
    opts: FloatXorOptions,
}

impl FloatXorFilter {
    pub fn new(opts: FloatXorOptions) -> Result<Self> {
        if opts.stride == 0 {
            return Err(ArcError::InvalidMethod(
                "FloatXor: stride must be > 0".into(),
            ));
        }
        Ok(Self { opts })
    }

    fn element_bytes(&self) -> usize {
        self.opts.precision.bytes()
    }

    fn stride_bytes(&self) -> usize {
        self.opts.stride * self.element_bytes()
    }
}

impl Filter for FloatXorFilter {
    fn name(&self) -> &'static str {
        "floatxor"
    }

    fn encode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        let eb = self.element_bytes();
        let sb = self.stride_bytes();

        if input.len() % eb != 0 {
            return Err(ArcError::Codec {
                codec: "floatxor",
                message: format!(
                    "input length {} is not a multiple of element size {eb}",
                    input.len()
                ),
            });
        }

        output.reserve(input.len());
        // First `stride` elements are copied verbatim.
        let first_plain = sb.min(input.len());
        output.extend_from_slice(&input[..first_plain]);

        let mut i = first_plain;
        while i + eb <= input.len() {
            let prev = &input[i - sb..i - sb + eb];
            let curr = &input[i..i + eb];
            for k in 0..eb {
                output.push(curr[k] ^ prev[k]);
            }
            i += eb;
        }

        Ok(())
    }

    fn decode(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        let eb = self.element_bytes();
        let sb = self.stride_bytes();

        if input.len() % eb != 0 {
            return Err(ArcError::Codec {
                codec: "floatxor",
                message: format!(
                    "input length {} is not a multiple of element size {eb}",
                    input.len()
                ),
            });
        }

        output.reserve(input.len());
        let first_plain = sb.min(input.len());
        output.extend_from_slice(&input[..first_plain]);

        let mut i = first_plain;
        while i + eb <= input.len() {
            // XOR the encoded residual against the already-decoded element `stride` back.
            let prev_start = output.len() - sb;
            let prev: Vec<u8> = output[prev_start..prev_start + eb].to_vec();
            let residual = &input[i..i + eb];
            for k in 0..eb {
                output.push(residual[k] ^ prev[k]);
            }
            i += eb;
        }

        Ok(())
    }
}
