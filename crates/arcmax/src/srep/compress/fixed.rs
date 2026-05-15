//! Fixed-block encoder for `-m3`, `-m4`, and `-m5`.
//!
//! Wraps `FixedMatcher` into the full block-read → match → write loop.
//! Per advisor guidance (point 2): `block_size` is rounded down to a multiple
//! of `chunk_len` so that `block_start` is always L-aligned.

use byteorder::{LE, WriteBytesExt};
use rand::RngCore;
use std::io::{Read, Seek, SeekFrom, Write};

use crate::srep::compress::block_sink::{
    write_index_lz_block, write_io_lz_block, MatchSink,
};
use crate::srep::compress::literal::read_full_block;
use crate::srep::compress::CompressionReport;
use crate::srep::config::{Method, OutputMode, SrepConfig};
use crate::srep::error::SrepError;
use crate::srep::format::{ArchiveHeader, IndexLzFooter, StatCodec};
use crate::srep::hash::digest::compute_block_digest;
use crate::srep::matchers::fixed::FixedMatcher;

/// Encode `input` as an SREP archive using the fixed-block matcher (-m3/-m4/-m5).
pub fn encode<R, W>(
    mut input: R,
    mut output: W,
    config: &SrepConfig,
) -> Result<CompressionReport, SrepError>
where
    R: Read + Seek,
    W: Write + Seek,
{
    if matches!(config.output_mode, OutputMode::FutureLz) {
        return Err(SrepError::Unsupported("FUTURE_LZ encoding not yet implemented"));
    }
    if config.block_size > u32::MAX as usize {
        return Err(SrepError::Overflow("block_size exceeds u32::MAX"));
    }

    let chunk_len = config.chunk_len;

    // Per advisor guidance #2: block_size must be a multiple of chunk_len so
    // that block_start stays L-aligned across block boundaries.
    let effective_block_size = (config.block_size / chunk_len) * chunk_len;
    if effective_block_size == 0 {
        return Err(SrepError::Format("block_size smaller than chunk_len"));
    }

    // Build archive header.
    let seed_len = config.hash.seed_len();
    let mut seed = vec![0u8; seed_len];
    if seed_len > 0 {
        rand::rng().fill_bytes(&mut seed);
    }
    let digest_len = config.hash.digest_len();
    // INDEX_LZ version 4 has no header bit for ROUND_MATCHES; the decoder must
    // interpret footer stats as non-rounded. Keep rounded stats only for IO_LZ
    // archives where the format version distinguishes round/non-round.
    let round_matches =
        config.round_matches() && matches!(config.output_mode, OutputMode::IoLz);

    let header = ArchiveHeader {
        output_mode: config.output_mode,
        round_matches,
        hash: config.hash,
        seed: seed.clone(),
        base_len: config.base_len(),
        digest_len,
    };
    header.write(&mut output)?;

    // Determine file size for FixedMatcher allocation.
    let file_size = {
        let pos = input.seek(std::io::SeekFrom::End(0)).map_err(SrepError::Io)?;
        input.seek(std::io::SeekFrom::Start(0)).map_err(SrepError::Io)?;
        pos
    };

    // Build the matcher.
    let mut matcher = FixedMatcher::new(
        file_size,
        chunk_len,
        config.min_match,
        config.method,
        config.hash,
        seed.clone(),
        config.acceleration.probe_stride() as i32,
    )?;

    let codec = StatCodec {
        round_matches,
        chunk_len: chunk_len as u32,
    };

    let mut buf = vec![0u8; effective_block_size];
    let mut bytes_in: u64 = 0;
    let mut blocks_written: u64 = 0;
    let mut matches_written: u64 = 0;
    let mut block_start: u64 = 0;

    // INDEX_LZ accumulates all stats here; IO_LZ writes them inline.
    let mut all_stats: Vec<u32> = Vec::new();
    let mut per_block_stat_sizes: Vec<u32> = Vec::new();

    loop {
        let n = read_full_block(&mut input, &mut buf)?;
        if n == 0 {
            break;
        }

        // Round n down to complete chunks (partial tail chunk left as literal).
        let usable = (n / chunk_len) * chunk_len;
        let block_full = &buf[..n];
        let block_aligned = &buf[..usable];

        // Compute block digest over the full (non-rounded) block.
        let digest = compute_block_digest(header.hash, &header.seed, block_full)?;

        // Phase 1: prepare (store digests + slice hashes for this block's chunks).
        matcher.prepare_block(block_start, block_aligned)?;

        // Phase 2: find matches and fill sink.
        let mut sink = MatchSink::new(block_full, codec);
        let input_pos = input.stream_position().map_err(SrepError::Io)?;
        let hits = if matches!(config.method, Method::M4 | Method::M5) {
            matcher.compress_block_with_source(
                block_start,
                block_aligned,
                &mut sink,
                Some(&mut input),
            )?
        } else {
            matcher.compress_block(block_start, block_aligned, &mut sink)?
        };
        input.seek(SeekFrom::Start(input_pos)).map_err(SrepError::Io)?;
        matches_written += hits;

        let (stats, literals) = sink.finish();

        match config.output_mode {
            OutputMode::IoLz => {
                write_io_lz_block(
                    &mut output,
                    &stats,
                    &literals,
                    n as u32,
                    &digest,
                )?;
            }
            OutputMode::IndexLz => {
                let stat_bytes = (stats.len() * size_of::<u32>()) as u32;
                // Per advisor guidance #5: push actual stat byte count, not 0.
                per_block_stat_sizes.push(stat_bytes);
                all_stats.extend_from_slice(&stats);
                write_index_lz_block(&mut output, &literals, n as u32, &digest)?;
            }
            OutputMode::FutureLz => unreachable!(),
        }

        bytes_in += n as u64;
        blocks_written += 1;
        block_start += usable as u64;
        // Note: if n > usable, the tail bytes of the partial chunk advance block_start
        // by the partial amount — but since next block_start must be L-aligned, we
        // snap forward to the next L boundary here.
        if n > usable {
            block_start += (chunk_len - (n - usable)) as u64;
        }
    }

    // Write tail.
    let bytes_out = match config.output_mode {
        OutputMode::IoLz => {
            // EOF sentinel: three zero u32 words + digest_len zeros.
            output.write_u32::<LE>(0).map_err(SrepError::Io)?;
            output.write_u32::<LE>(0).map_err(SrepError::Io)?;
            output.write_u32::<LE>(0).map_err(SrepError::Io)?;
            if digest_len > 0 {
                output.write_all(&vec![0u8; digest_len]).map_err(SrepError::Io)?;
            }
            let pos = output.stream_position().map_err(SrepError::Io)?;
            output.flush().map_err(SrepError::Io)?;
            pos
        }
        OutputMode::IndexLz => {
            // Write all stat data, then per-block sizes, then footer.
            let stat_size: u64 = all_stats.len() as u64 * size_of::<u32>() as u64;
            for &s in &all_stats {
                output.write_u32::<LE>(s).map_err(SrepError::Io)?;
            }
            for &sz in &per_block_stat_sizes {
                output.write_u32::<LE>(sz).map_err(SrepError::Io)?;
            }
            let sizes_bytes = per_block_stat_sizes.len() * size_of::<u32>();
            let footer_size = (sizes_bytes + IndexLzFooter::ON_DISK_SIZE) as u32;
            IndexLzFooter { stat_size, footer_size }.write(&mut output)?;
            let pos = output.stream_position().map_err(SrepError::Io)?;
            output.flush().map_err(SrepError::Io)?;
            pos
        }
        OutputMode::FutureLz => unreachable!(),
    };

    Ok(CompressionReport {
        bytes_in,
        bytes_out,
        blocks_written,
        matches_written,
    })
}
