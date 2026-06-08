//! Sampling-based automatic codec selection.
//!
//! Given a slice of input data and a list of candidate codecs, [`select_by_sampling`]
//! compresses a representative sample of the data with each candidate and returns
//! the candidate that produced the smallest output.  This lets callers avoid
//! hand-tuning the codec per workload — useful as a fallback when the
//! [`crate::filetype`] routing has no opinion (`FileTypeHint::Unknown` /
//! `FileTypeHint::Binary` of unusual structure).
//!
//! ## When to use this
//!
//! - You don't know what kind of data you're compressing.
//! - You're benchmarking and want the *best* of a small set of codecs without
//!   running the full corpus N times.
//! - You're streaming an unknown content type and need a quick decision.
//!
//! ## When NOT to use this
//!
//! - Already-routed file types: prefer [`crate::filetype::suggest_pipeline`].
//! - Tiny inputs: the sampling overhead dominates.  Below ~64 KiB just pick a
//!   sensible default (LZMA or PPMd) directly.
//! - Latency-critical paths where the sampling pass itself is too expensive.
//!
//! ## Sampling strategy
//!
//! We take three slices — head, middle, tail — concatenated, totalling
//! `sample_size` bytes.  This catches content that varies across the file
//! (e.g., a tar with text at the start and binaries later).

use std::io::Cursor;
use std::time::{Duration, Instant};

use crate::error::Result;
use crate::method::pipeline::CodecPipeline;
use crate::method::Method;

/// Default sample size: 1 MiB.  Large enough to characterize most workloads,
/// small enough that even the slowest candidate (lzma:max) finishes in
/// sub-second time.
pub const DEFAULT_SAMPLE_SIZE: usize = 1024 * 1024;

/// Below this input size, sampling doesn't pay back — just use the first
/// candidate as a sensible default.
pub const MIN_INPUT_FOR_SAMPLING: usize = 64 * 1024;

/// Outcome of [`select_by_sampling`] — winner plus the trial table.
#[derive(Debug, Clone)]
pub struct SelectionResult {
    /// Label and method of the winning candidate (smallest sample output).
    pub winner_label: String,
    pub winner_method: Method,
    /// Per-candidate trial output for transparency / logging.
    pub trials: Vec<Trial>,
}

#[derive(Debug, Clone)]
pub struct Trial {
    pub label: String,
    /// `Some(size)` on success; `None` if compression failed for this candidate.
    pub compressed_size: Option<usize>,
    /// Wall-clock time taken to compress the sample.
    pub elapsed: Duration,
}

impl Trial {
    /// Compression ratio over the sample (`compressed_size / sample_size`).
    pub fn ratio(&self, sample_size: usize) -> Option<f64> {
        self.compressed_size.map(|n| n as f64 / sample_size as f64)
    }
}

/// Sample `data`, compress with each `(label, method)` pair, return the winner.
///
/// `sample_size` is the *total* number of bytes sampled from `data`.  When
/// `data.len() <= sample_size` the whole input is used.  When larger, three
/// equal-sized slices are taken from head, middle, and tail.
///
/// Returns `Err` only if `candidates` is empty.  Per-candidate failures are
/// recorded in [`Trial::compressed_size`] = `None`; they do not abort the
/// selection (the winner is chosen from the candidates that succeeded).
pub fn select_by_sampling(
    data: &[u8],
    candidates: &[(String, Method)],
    sample_size: usize,
) -> Result<SelectionResult> {
    if candidates.is_empty() {
        return Err(crate::error::ArcError::InvalidMethod(
            "select_by_sampling: candidates list is empty".to_string(),
        ));
    }

    // Tiny inputs: skip sampling, return the first candidate.
    if data.len() < MIN_INPUT_FOR_SAMPLING {
        let (label, method) = &candidates[0];
        return Ok(SelectionResult {
            winner_label: label.clone(),
            winner_method: method.clone(),
            trials: vec![Trial {
                label: label.clone(),
                compressed_size: None,
                elapsed: Duration::ZERO,
            }],
        });
    }

    let sample = build_sample(data, sample_size);
    let mut trials = Vec::with_capacity(candidates.len());

    for (label, method) in candidates {
        let t0 = Instant::now();
        let mut compressed = Vec::with_capacity(sample.len() / 2);
        let result =
            CodecPipeline::new(method.clone()).compress(Cursor::new(&sample), &mut compressed);
        let elapsed = t0.elapsed();
        match result {
            Ok(_) => trials.push(Trial {
                label: label.clone(),
                compressed_size: Some(compressed.len()),
                elapsed,
            }),
            Err(_) => trials.push(Trial {
                label: label.clone(),
                compressed_size: None,
                elapsed,
            }),
        }
    }

    // Pick the smallest successful trial.  Ties broken by lower elapsed time.
    let best_idx = trials
        .iter()
        .enumerate()
        .filter_map(|(i, t)| t.compressed_size.map(|sz| (i, sz, t.elapsed)))
        .min_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)))
        .map(|(i, _, _)| i);

    let winner_idx = best_idx.unwrap_or(0); // fall back to first candidate if all failed
    let (label, method) = &candidates[winner_idx];

    Ok(SelectionResult {
        winner_label: label.clone(),
        winner_method: method.clone(),
        trials,
    })
}

/// Build a representative sample from `data`.
///
/// When `data.len() <= sample_size`: return the whole input.
/// When larger: head + middle + tail, each roughly `sample_size / 3` bytes.
fn build_sample(data: &[u8], sample_size: usize) -> Vec<u8> {
    if data.len() <= sample_size {
        return data.to_vec();
    }

    let third = sample_size / 3;
    let head = &data[..third];
    let mid_start = (data.len() - third) / 2;
    let mid = &data[mid_start..mid_start + third];
    let tail = &data[data.len() - third..];

    let mut sample = Vec::with_capacity(third * 3);
    sample.extend_from_slice(head);
    sample.extend_from_slice(mid);
    sample.extend_from_slice(tail);
    sample
}

// ─────────────────────────────────────────────────────────────────────────────
// Standard candidate sets
// ─────────────────────────────────────────────────────────────────────────────

/// A small, well-balanced candidate list covering the speed/ratio spectrum.
///
/// Order matches "fastest first," which becomes the tiebreaker when sample
/// sizes are equal.  Use this when you want a general-purpose `auto:` knob.
pub fn standard_candidates() -> Vec<(String, Method)> {
    use crate::codec::lz4::Lz4Options;
    use crate::codec::lzma::LzmaOptions;
    use crate::codec::ppmd::PpmdOptions;
    use crate::codec::zstd::ZstdOptions;

    vec![
        ("lz4".to_string(), Method::Lz4(Lz4Options::default())),
        ("zstd:3".to_string(), Method::Zstd(ZstdOptions { level: 3 })),
        (
            "zstd:11".to_string(),
            Method::Zstd(ZstdOptions { level: 11 }),
        ),
        ("ppmd:o6".to_string(), Method::Ppmd(PpmdOptions::default())),
        (
            "lzma:d16m".to_string(),
            Method::Lzma(LzmaOptions {
                dict_size: 16 * 1024 * 1024,
                ..LzmaOptions::default()
            }),
        ),
    ]
}

/// Smaller, faster candidate set — only "fast" codecs.  For real-time scenarios.
pub fn fast_candidates() -> Vec<(String, Method)> {
    use crate::codec::brotli::BrotliOptions;
    use crate::codec::lz4::Lz4Options;
    use crate::codec::snappy::SnappyOptions;
    use crate::codec::zstd::ZstdOptions;

    vec![
        ("lz4".to_string(), Method::Lz4(Lz4Options::default())),
        ("snappy".to_string(), Method::Snappy(SnappyOptions)),
        ("zstd:1".to_string(), Method::Zstd(ZstdOptions { level: 1 })),
        ("zstd:3".to_string(), Method::Zstd(ZstdOptions { level: 3 })),
        (
            "brotli:q3".to_string(),
            Method::Brotli(BrotliOptions {
                quality: 3,
                lgwin: 22,
            }),
        ),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
