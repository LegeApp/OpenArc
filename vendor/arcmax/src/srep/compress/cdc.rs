//! CDC encoder for SREP `-m1` (fast) and `-m2` (ZPAQ).
//!
//! Mirrors the structure of `fixed.rs`: reads blocks, runs the CDC matcher,
//! and writes SREP blocks in IO_LZ or INDEX_LZ format.

use byteorder::{WriteBytesExt, LE};
use rand::RngCore;
use std::io::{Read, Seek, Write};

use crate::srep::compress::block_sink::{write_index_lz_block, write_io_lz_block, MatchSink};
use crate::srep::compress::literal::read_full_block;
use crate::srep::compress::CompressionReport;
use crate::srep::config::{Method, OutputMode, SrepConfig};
use crate::srep::error::SrepError;
use crate::srep::format::{ArchiveHeader, IndexLzFooter, StatCodec};
use crate::srep::hash::digest::compute_block_digest;
use crate::srep::matchers::cdc::{CdcMatcher, CdcMode};

/// Encode `input` as an SREP archive using content-defined chunking (-m1/-m2).
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
        return Err(SrepError::Unsupported(
            "FUTURE_LZ encoding not yet implemented",
        ));
    }

    let mode = match config.method {
        Method::M1 => CdcMode::Fast,
        Method::M2 => CdcMode::Zpaq,
        _ => unreachable!("cdc::encode called with non-CDC method"),
    };

    // Build archive header.
    let seed_len = config.hash.seed_len();
    let mut seed = vec![0u8; seed_len];
    if seed_len > 0 {
        rand::rng().fill_bytes(&mut seed);
    }
    let digest_len = config.hash.digest_len();
    let round_matches = config.round_matches() && matches!(config.output_mode, OutputMode::IoLz);

    let header = ArchiveHeader {
        output_mode: config.output_mode,
        round_matches,
        hash: config.hash,
        seed: seed.clone(),
        base_len: config.base_len(),
        digest_len,
    };
    header.write(&mut output)?;

    let avg_len = config.chunk_len; // chunk_len is the CDC target average length
    let mut matcher = CdcMatcher::new(mode, avg_len, config.min_match, seed.clone());

    // base_len = BASE_LEN in the stat codec: the minimum guaranteed match length.
    // Stored as `len - base_len` in the stat stream (non-round mode, 4 stats/match).
    let codec = StatCodec {
        round_matches,
        chunk_len: config.base_len(),
    };

    let mut buf = vec![0u8; config.block_size];
    let mut bytes_in: u64 = 0;
    let mut blocks_written: u64 = 0;
    let mut matches_written: u64 = 0;
    let mut block_start: u64 = 0;

    let mut all_stats: Vec<u32> = Vec::new();
    let mut per_block_stat_sizes: Vec<u32> = Vec::new();

    loop {
        let n = read_full_block(&mut input, &mut buf)?;
        if n == 0 {
            break;
        }
        let block = &buf[..n];

        let digest = compute_block_digest(header.hash, &header.seed, block)?;

        let mut sink = MatchSink::new(block, codec);
        let hits = matcher.compress_block(block_start, block, &mut sink)?;
        matches_written += hits;

        let (stats, literals) = sink.finish();

        match config.output_mode {
            OutputMode::IoLz => {
                write_io_lz_block(&mut output, &stats, &literals, n as u32, &digest)?;
            }
            OutputMode::IndexLz => {
                let stat_bytes = (stats.len() * size_of::<u32>()) as u32;
                per_block_stat_sizes.push(stat_bytes);
                all_stats.extend_from_slice(&stats);
                write_index_lz_block(&mut output, &literals, n as u32, &digest)?;
            }
            OutputMode::FutureLz => unreachable!(),
        }

        bytes_in += n as u64;
        blocks_written += 1;
        block_start += n as u64;
    }

    // Write tail.
    let bytes_out = match config.output_mode {
        OutputMode::IoLz => {
            output.write_u32::<LE>(0).map_err(SrepError::Io)?;
            output.write_u32::<LE>(0).map_err(SrepError::Io)?;
            output.write_u32::<LE>(0).map_err(SrepError::Io)?;
            if digest_len > 0 {
                output
                    .write_all(&vec![0u8; digest_len])
                    .map_err(SrepError::Io)?;
            }
            let pos = output.stream_position().map_err(SrepError::Io)?;
            output.flush().map_err(SrepError::Io)?;
            pos
        }
        OutputMode::IndexLz => {
            let stat_size: u64 = all_stats.len() as u64 * size_of::<u32>() as u64;
            for &s in &all_stats {
                output.write_u32::<LE>(s).map_err(SrepError::Io)?;
            }
            for &sz in &per_block_stat_sizes {
                output.write_u32::<LE>(sz).map_err(SrepError::Io)?;
            }
            let sizes_bytes = per_block_stat_sizes.len() * size_of::<u32>();
            let footer_size = (sizes_bytes + IndexLzFooter::ON_DISK_SIZE) as u32;
            IndexLzFooter {
                stat_size,
                footer_size,
            }
            .write(&mut output)?;
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
