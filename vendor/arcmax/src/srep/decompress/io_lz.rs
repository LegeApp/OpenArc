//! IO_LZ decompression: stats are stored inline with each block, and matches
//! whose source falls in earlier blocks are resolved by seeking back through
//! the output file. Format versions 1 and 2.

use byteorder::{ReadBytesExt, LE};
use std::io::{Read, Seek, SeekFrom, Write};

use crate::srep::decompress::block::decode_block;
use crate::srep::decompress::DecompressionReport;
use crate::srep::error::SrepError;
use crate::srep::format::{ArchiveHeader, BlockHeader, StatCodec};
use crate::srep::hash::verify_block_digest;
use crate::srep::types::Offset;

pub fn decompress_io_lz<R, W>(
    input: &mut R,
    output: &mut W,
    header: &ArchiveHeader,
) -> Result<DecompressionReport, SrepError>
where
    R: Read,
    W: Read + Write + Seek,
{
    let codec = StatCodec {
        round_matches: header.round_matches,
        chunk_len: header.base_len,
    };

    let mut block_start: Offset = 0;
    let mut blocks_decoded: u64 = 0;

    loop {
        // Peek at the first 8 bytes to detect the EOF sentinel (two zero u32 words).
        // We have to be lenient — the file may simply be EOF too.
        let lit_word = match input.read_u32::<LE>() {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(SrepError::Io(e)),
        };
        let orig_word = input.read_u32::<LE>()?;
        if lit_word == 0 && orig_word == 0 {
            // EOF sentinel.
            break;
        }

        // Read the rest of the block header (stat_size + digest bytes).
        let stat_size = input.read_u32::<LE>()?;
        let mut digest = vec![0u8; header.digest_len];
        if !digest.is_empty() {
            input.read_exact(&mut digest)?;
        }
        let block_hdr = BlockHeader {
            literal_bytes: lit_word,
            original_len: orig_word,
            stat_size,
        };

        // Read inline stat array (stat_size bytes = stat_size / 4 words).
        if block_hdr.stat_size as usize % size_of::<u32>() != 0 {
            return Err(SrepError::Format("stat_size not multiple of 4"));
        }
        let stat_count = block_hdr.stat_size as usize / size_of::<u32>();
        let mut stats = vec![0u32; stat_count];
        for s in &mut stats {
            *s = input.read_u32::<LE>()?;
        }

        // Read literal payload.
        let mut literals = vec![0u8; block_hdr.literal_bytes as usize];
        if !literals.is_empty() {
            input.read_exact(&mut literals)?;
        }

        // Decode the block.
        let mut block = vec![0u8; block_hdr.original_len as usize];
        decode_block(block_start, &mut block, &literals, &stats, codec, output)?;

        // Verify block integrity digest against decoded content.
        verify_block_digest(header.hash, &header.seed, &block, &digest)?;

        // Append decoded block to output (seek so subsequent back-reads see it).
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
