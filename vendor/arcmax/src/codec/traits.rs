use std::io::{Read, Write};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Compress,
    Decompress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryUsage {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub working_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CodecReport {
    pub bytes_in: u64,
    pub bytes_out: u64,
}

/// Static metadata about a codec's performance and resource profile.
///
/// Values are conservative estimates on typical compressible data (source code,
/// logs, binary executables). Benchmarks on a specific corpus will differ.
/// Zero in any numeric field means "unknown / not measured."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecCapabilities {
    /// Compression throughput in MB/s (worst-case preset / single thread).
    pub compress_speed_mb_per_sec: u32,
    /// Decompression throughput in MB/s.
    pub decompress_speed_mb_per_sec: u32,
    /// Typical output size as a percentage of input size on compressible data.
    /// 30 = 30 % of original (0.30×).  100 = Store (no compression).
    /// For pre-processing filters this is 100 (they do not reduce size alone).
    pub typical_ratio_pct: u8,
    /// Minimum input size in bytes where this codec is worth applying.
    /// Below this the framing overhead may exceed the savings.
    pub min_useful_bytes: u32,
    /// Peak working memory in MiB (excluding input/output buffers).
    pub peak_memory_mib: u32,
    /// Whether the codec itself exploits multiple CPU threads internally.
    /// Note: the outer pipeline can always parallelize independent blocks
    /// regardless of this flag.
    pub parallelizable: bool,
}

impl Default for CodecCapabilities {
    fn default() -> Self {
        // Conservative / unknown defaults.
        Self {
            compress_speed_mb_per_sec: 0,
            decompress_speed_mb_per_sec: 0,
            typical_ratio_pct: 50,
            min_useful_bytes: 0,
            peak_memory_mib: 0,
            parallelizable: false,
        }
    }
}

impl CodecCapabilities {
    /// Return a human-readable one-line summary of the capabilities.
    pub fn summary(&self) -> String {
        format!(
            "comp {}MB/s  decomp {}MB/s  ratio ~{}%  min {}B  peak {}MiB  parallel={}",
            self.compress_speed_mb_per_sec,
            self.decompress_speed_mb_per_sec,
            self.typical_ratio_pct,
            self.min_useful_bytes,
            self.peak_memory_mib,
            self.parallelizable,
        )
    }

    /// Combine two capabilities for a sequential pipeline stage.
    ///
    /// Speed becomes the minimum of both (bottleneck); memory sums; ratio
    /// composes multiplicatively; min_useful_bytes takes the maximum.
    pub fn compose(self, next: CodecCapabilities) -> CodecCapabilities {
        let ratio =
            (self.typical_ratio_pct as u32).saturating_mul(next.typical_ratio_pct as u32) / 100;
        CodecCapabilities {
            compress_speed_mb_per_sec: nonzero_min(
                self.compress_speed_mb_per_sec,
                next.compress_speed_mb_per_sec,
            ),
            decompress_speed_mb_per_sec: nonzero_min(
                self.decompress_speed_mb_per_sec,
                next.decompress_speed_mb_per_sec,
            ),
            typical_ratio_pct: ratio.min(100) as u8,
            min_useful_bytes: self.min_useful_bytes.max(next.min_useful_bytes),
            peak_memory_mib: self.peak_memory_mib.saturating_add(next.peak_memory_mib),
            parallelizable: self.parallelizable && next.parallelizable,
        }
    }
}

/// min of two values, treating 0 as "unknown" (so `min(0, x) = x`, not 0).
fn nonzero_min(a: u32, b: u32) -> u32 {
    match (a, b) {
        (0, b) => b,
        (a, 0) => a,
        (a, b) => a.min(b),
    }
}

pub trait Codec {
    fn name(&self) -> &'static str;

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport>;

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport>;

    fn memory_usage(&self, direction: Direction) -> MemoryUsage;

    /// Static capabilities of this codec.
    ///
    /// The default implementation returns all-unknown values and is appropriate
    /// for FFI-backed codecs that have not been profiled.
    fn capabilities(&self) -> CodecCapabilities {
        CodecCapabilities::default()
    }
}
