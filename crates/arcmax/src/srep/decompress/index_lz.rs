//! INDEX_LZ decompression: per-block headers carry only literal data, while
//! the full match list lives in the footer of the archive. Format version 4.
//!
//! On-disk archive layout (after the archive header):
//!
//! ```text
//! [block_0_header (3 u32 + digest)] [block_0_literals]
//! [block_1_header                  ] [block_1_literals]
//! ...
//! [block_N_header                  ] [block_N_literals]
//! [all stats concatenated  (footer.stat_size bytes)]
//! [per-block stat sizes    (4 bytes × total_blocks)]
//! [6-word footer           (24 bytes)]
//! ```

use byteorder::{LE, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom, Write};

use crate::srep::decompress::block::decode_block;
use crate::srep::decompress::DecompressionReport;
use crate::srep::error::SrepError;
use crate::srep::format::{ArchiveHeader, BlockHeader, IndexLzFooter, StatCodec};
use crate::srep::hash::verify_block_digest;
use crate::srep::types::Offset;

pub fn decompress_index_lz<R, W>(
    input: &mut R,
    output: &mut W,
    header: &ArchiveHeader,
    file_size: u64,
) -> Result<DecompressionReport, SrepError>
where
    R: Read + Seek,
    W: Read + Write + Seek,
{
    let codec = StatCodec {
        round_matches: header.round_matches,
        chunk_len: header.base_len,
    };

    // 1) Read the 6-word footer at the very end of the file.
    let footer = IndexLzFooter::read(input, file_size)?;

    // 2) Read the concatenated stat data: footer.stat_size bytes located just
    //    before the per-block sizes + footer trailer.
    if footer.stat_size as usize % size_of::<u32>() != 0 {
        return Err(SrepError::Format("footer.stat_size not multiple of 4"));
    }

    let stat_region_end = file_size
        .checked_sub(footer.footer_size as u64)
        .ok_or(SrepError::Format("footer_size exceeds file_size"))?;
    let stat_region_start = stat_region_end
        .checked_sub(footer.stat_size)
        .ok_or(SrepError::Format("stat region underflows file"))?;

    input.seek(SeekFrom::Start(stat_region_start))?;
    let total_stat_words = (footer.stat_size / size_of::<u32>() as u64) as usize;
    let mut all_stats = vec![0u32; total_stat_words];
    for s in &mut all_stats {
        *s = input.read_u32::<LE>()?;
    }

    // 3) Read the per-block stat sizes (immediately following stat data).
    let trailer_size = footer.footer_size as usize;
    let sizes_bytes = trailer_size
        .checked_sub(IndexLzFooter::ON_DISK_SIZE)
        .ok_or(SrepError::Format("footer_size smaller than footer trailer"))?;
    if sizes_bytes % size_of::<u32>() != 0 {
        return Err(SrepError::Format("per-block sizes list not multiple of 4"));
    }
    let total_blocks = sizes_bytes / size_of::<u32>();
    let mut block_stat_sizes = vec![0u32; total_blocks];
    for s in &mut block_stat_sizes {
        *s = input.read_u32::<LE>()?;
    }

    // Sanity check: sum of per-block sizes must equal footer.stat_size.
    let summed: u64 = block_stat_sizes.iter().map(|&n| n as u64).sum();
    if summed != footer.stat_size {
        return Err(SrepError::Format(
            "per-block stat-size sum does not match footer total",
        ));
    }

    // 4) Seek back to the first block (right after the archive header) and
    //    decode each block in turn.
    input.seek(SeekFrom::Start(header.on_disk_size() as u64))?;

    let mut block_start: Offset = 0;
    let mut stat_cursor: usize = 0;
    let mut blocks_decoded: u64 = 0;

    for &per_block_size in &block_stat_sizes {
        // Read block header (literal_bytes, original_len, stat_size=0).
        let (block_hdr, digest) = BlockHeader::read(input, header.digest_len)?;
        if block_hdr.stat_size != 0 {
            return Err(SrepError::Format(
                "INDEX_LZ block header should have stat_size = 0",
            ));
        }

        // Read literal payload.
        let mut literals = vec![0u8; block_hdr.literal_bytes as usize];
        if !literals.is_empty() {
            input.read_exact(&mut literals)?;
        }

        // Slice off this block's stat entries from the global array.
        let count = per_block_size as usize / size_of::<u32>();
        if stat_cursor + count > all_stats.len() {
            return Err(SrepError::Format("stat array exhausted prematurely"));
        }
        let stats_slice = &all_stats[stat_cursor..stat_cursor + count];
        stat_cursor += count;

        // Decode the block.
        let mut block = vec![0u8; block_hdr.original_len as usize];
        decode_block(block_start, &mut block, &literals, stats_slice, codec, output)?;

        // Verify block integrity digest against decoded content.
        verify_block_digest(header.hash, &header.seed, &block, &digest)?;

        // Append decoded block to output.
        output.seek(SeekFrom::Start(block_start))?;
        output.write_all(&block)?;

        block_start += block_hdr.original_len as u64;
        blocks_decoded += 1;
    }

    output.flush().map_err(SrepError::Io)?;

    Ok(DecompressionReport {
        bytes_out: block_start,
        blocks_decoded,
    })
}
