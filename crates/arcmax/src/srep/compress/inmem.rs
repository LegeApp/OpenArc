//! Encoder wrapper for SREP `-m0` in-memory dictionary mode.

use byteorder::{LE, WriteBytesExt};
use rand::RngCore;
use std::io::{Read, Seek, Write};

use crate::srep::compress::block_sink::{
    write_index_lz_block, write_io_lz_block, MatchSink,
};
use crate::srep::compress::literal::read_full_block;
use crate::srep::compress::CompressionReport;
use crate::srep::config::OutputMode;
use crate::srep::config::SrepConfig;
use crate::srep::error::SrepError;
use crate::srep::format::{ArchiveHeader, IndexLzFooter, StatCodec};
use crate::srep::hash::digest::compute_block_digest;
use crate::srep::matchers::InMemDeduplicator;

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

    let dict_size = if config.dict_size == 0 {
        config.block_size.saturating_mul(4).max(config.min_match)
    } else {
        config.dict_size
    };

    let seed_len = config.hash.seed_len();
    let mut seed = vec![0u8; seed_len];
    if seed_len > 0 {
        rand::rng().fill_bytes(&mut seed);
    }

    let header = ArchiveHeader {
        output_mode: config.output_mode,
        round_matches: false,
        hash: config.hash,
        seed: seed.clone(),
        base_len: config.base_len(),
        digest_len: config.hash.digest_len(),
    };
    header.write(&mut output)?;

    let mut matcher =
        InMemDeduplicator::new(config.chunk_len, config.min_match, dict_size)?;
    let codec = StatCodec {
        round_matches: false,
        chunk_len: config.base_len(),
    };

    let mut buf = vec![0u8; config.block_size];
    let mut bytes_in = 0u64;
    let mut blocks_written = 0u64;
    let mut matches_written = 0u64;
    let mut block_start = 0u64;
    let mut all_stats = Vec::<u32>::new();
    let mut per_block_stat_sizes = Vec::<u32>::new();

    loop {
        let n = read_full_block(&mut input, &mut buf)?;
        if n == 0 {
            break;
        }

        let block = &buf[..n];
        let digest = compute_block_digest(header.hash, &header.seed, block)?;
        let prepared = matcher.prepare_block(block);
        let mut sink = MatchSink::new(block, codec);
        let hits = matcher.compress_block(block_start, block, &prepared, &mut sink)?;
        matches_written += hits;
        matcher.commit_block(block_start, block, &prepared);

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

    let bytes_out = match config.output_mode {
        OutputMode::IoLz => {
            output.write_u32::<LE>(0).map_err(SrepError::Io)?;
            output.write_u32::<LE>(0).map_err(SrepError::Io)?;
            output.write_u32::<LE>(0).map_err(SrepError::Io)?;
            if header.digest_len > 0 {
                output
                    .write_all(&vec![0u8; header.digest_len])
                    .map_err(SrepError::Io)?;
            }
            let pos = output.stream_position().map_err(SrepError::Io)?;
            output.flush().map_err(SrepError::Io)?;
            pos
        }
        OutputMode::IndexLz => {
            let stat_size = all_stats.len() as u64 * size_of::<u32>() as u64;
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
