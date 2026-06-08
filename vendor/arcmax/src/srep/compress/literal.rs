use byteorder::{WriteBytesExt, LE};
use rand::RngCore;
use std::io::{Read, Seek, Write};

use crate::srep::compress::block_sink::{write_index_lz_block, write_io_lz_block};
use crate::srep::compress::CompressionReport;
use crate::srep::config::{OutputMode, SrepConfig};
use crate::srep::error::SrepError;
use crate::srep::format::{ArchiveHeader, IndexLzFooter};
use crate::srep::hash::digest::compute_block_digest;

/// Build the archive header for a literal-only encode.
///
/// Generates a cryptographic random seed for keyed hashes (SipHash).
/// Rejects FUTURE_LZ and block sizes > u32::MAX.
fn build_archive_header(config: &SrepConfig) -> Result<ArchiveHeader, SrepError> {
    if matches!(config.output_mode, OutputMode::FutureLz) {
        return Err(SrepError::Unsupported(
            "FUTURE_LZ encoding not yet implemented",
        ));
    }
    if config.block_size > u32::MAX as usize {
        return Err(SrepError::Overflow("block_size exceeds u32::MAX"));
    }

    // Generate a fresh random seed for keyed hashes.
    let seed_len = config.hash.seed_len();
    let mut seed = vec![0u8; seed_len];
    if seed_len > 0 {
        rand::rng().fill_bytes(&mut seed);
    }

    let digest_len = config.hash.digest_len();

    Ok(ArchiveHeader {
        output_mode: config.output_mode,
        round_matches: false,
        hash: config.hash,
        seed,
        base_len: config.base_len(),
        digest_len,
    })
}

/// Fill `buf` as completely as possible from `r`, stopping at EOF.
/// Returns the number of bytes actually read (0 signals EOF).
pub(super) fn read_full_block<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<usize, SrepError> {
    let mut pos = 0;
    while pos < buf.len() {
        match r.read(&mut buf[pos..]) {
            Ok(0) => break,
            Ok(n) => pos += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(SrepError::Io(e)),
        }
    }
    Ok(pos)
}

/// Write the IO_LZ EOF sentinel: all three header fields zero plus `digest_len`
/// zero bytes (placeholder digest for the sentinel block).
fn write_io_lz_tail<W: Write + Seek>(w: &mut W, digest_len: usize) -> Result<u64, SrepError> {
    w.write_u32::<LE>(0).map_err(SrepError::Io)?;
    w.write_u32::<LE>(0).map_err(SrepError::Io)?;
    w.write_u32::<LE>(0).map_err(SrepError::Io)?;
    if digest_len > 0 {
        w.write_all(&vec![0u8; digest_len]).map_err(SrepError::Io)?;
    }
    let total = w.stream_position().map_err(SrepError::Io)?;
    w.flush().map_err(SrepError::Io)?;
    Ok(total)
}

/// Write the INDEX_LZ tail: empty stat blob + per-block stat sizes + 24-byte footer.
fn write_index_lz_tail<W: Write + Seek>(
    w: &mut W,
    per_block_stat_sizes: &[u32],
) -> Result<u64, SrepError> {
    let stat_size: u64 = 0; // literal-only: no stat entries

    for &s in per_block_stat_sizes {
        w.write_u32::<LE>(s).map_err(SrepError::Io)?;
    }
    let sizes_bytes = per_block_stat_sizes.len() * size_of::<u32>();

    let footer_size = (sizes_bytes + IndexLzFooter::ON_DISK_SIZE) as u32;
    IndexLzFooter {
        stat_size,
        footer_size,
    }
    .write(w)?;

    let total = w.stream_position().map_err(SrepError::Io)?;
    w.flush().map_err(SrepError::Io)?;
    Ok(total)
}

/// Encode `input` as a literal-only SREP archive into `output`.
///
/// Every input byte becomes a literal byte; no matches are emitted.  If the
/// configured hash is not `None`, a digest of each decoded block (= raw input
/// chunk) is written into the block header.
pub fn encode<R, W>(
    mut input: R,
    mut output: W,
    config: &SrepConfig,
) -> Result<CompressionReport, SrepError>
where
    R: Read + Seek,
    W: Write + Seek,
{
    let header = build_archive_header(config)?;
    header.write(&mut output)?;

    let block_capacity = config.block_size;
    let mut buf = vec![0u8; block_capacity];
    let mut bytes_in: u64 = 0;
    let mut blocks_written: u64 = 0;
    let mut per_block_stat_sizes: Vec<u32> = Vec::new();

    loop {
        let n = read_full_block(&mut input, &mut buf)?;
        if n == 0 {
            break;
        }
        let block = &buf[..n];

        // Compute block digest (empty slice when HashKind::None).
        let digest = compute_block_digest(header.hash, &header.seed, block)?;

        match config.output_mode {
            OutputMode::IoLz => {
                write_io_lz_block(&mut output, &[], block, n as u32, &digest)?;
            }
            OutputMode::IndexLz => {
                write_index_lz_block(&mut output, block, n as u32, &digest)?;
                per_block_stat_sizes.push(0);
            }
            OutputMode::FutureLz => unreachable!(),
        }

        bytes_in += n as u64;
        blocks_written += 1;
    }

    let bytes_out = match config.output_mode {
        OutputMode::IoLz => write_io_lz_tail(&mut output, header.digest_len)?,
        OutputMode::IndexLz => write_index_lz_tail(&mut output, &per_block_stat_sizes)?,
        OutputMode::FutureLz => unreachable!(),
    };

    Ok(CompressionReport {
        bytes_in,
        bytes_out,
        blocks_written,
        matches_written: 0,
    })
}
