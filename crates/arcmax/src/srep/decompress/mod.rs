//! SREP decompression — entry point and per-mode dispatch.
//!
//! Supports `IO_LZ` (versions 1 & 2, ROUND_MATCHES toggled by version) and
//! `INDEX_LZ` (version 4). `FUTURE_LZ` (version 3) is deferred to a later
//! stage and currently returns `SrepError::Unsupported`.

pub mod block;
pub mod io_lz;
pub mod index_lz;

use std::io::{Read, Seek, SeekFrom, Write};

use crate::srep::config::OutputMode;
use crate::srep::error::SrepError;
use crate::srep::format::ArchiveHeader;

/// Statistics returned after a successful decompression.
#[derive(Debug, Clone, Copy, Default)]
pub struct DecompressionReport {
    pub bytes_out: u64,
    pub blocks_decoded: u64,
}

/// Decompress an SREP archive from `input` into `output`.
///
/// `output` must be `Read + Write + Seek` because the IO_LZ format expresses
/// matches whose source lies in previously-decoded blocks; resolving those
/// references requires reading back through the output stream.
pub fn decompress<R, W>(
    mut input: R,
    mut output: W,
) -> Result<DecompressionReport, SrepError>
where
    R: Read + Seek,
    W: Read + Write + Seek,
{
    // Probe the total input length so INDEX_LZ can locate its footer.
    let file_size = {
        let cur = input.stream_position()?;
        let end = input.seek(SeekFrom::End(0))?;
        input.seek(SeekFrom::Start(cur))?;
        end
    };

    let header = ArchiveHeader::read(&mut input)?;

    match header.output_mode {
        OutputMode::IoLz => io_lz::decompress_io_lz(&mut input, &mut output, &header),
        OutputMode::IndexLz => {
            index_lz::decompress_index_lz(&mut input, &mut output, &header, file_size)
        }
        OutputMode::FutureLz => {
            Err(SrepError::Unsupported("FUTURE_LZ decompression not yet implemented"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::srep::config::{HashKind, OutputMode};
    use crate::srep::format::{ArchiveHeader, BlockHeader, StatCodec};
    use byteorder::{LE, WriteBytesExt};
    use std::io::{Cursor, Write};

    /// Build a minimal IO_LZ archive containing a single literal-only block,
    /// then round-trip it through the decoder.
    #[test]
    fn round_trip_io_lz_single_literal_block() {
        let payload = b"the quick brown fox jumps over the lazy dog";

        let mut archive = Vec::<u8>::new();
        let hdr = ArchiveHeader {
            output_mode: OutputMode::IoLz,
            round_matches: false,
            hash: HashKind::None,
            seed: vec![],
            base_len: 32,
            digest_len: 0,
        };
        hdr.write(&mut archive).unwrap();

        // One block: literal_bytes = payload.len(), original_len = payload.len(), stat_size = 0
        let block = BlockHeader {
            literal_bytes: payload.len() as u32,
            original_len: payload.len() as u32,
            stat_size: 0,
        };
        block.write(&mut archive, &[]).unwrap();
        archive.write_all(payload).unwrap();

        // EOF sentinel: two zero u32 words + zero stat_size + zero digest
        archive.write_u32::<LE>(0).unwrap();
        archive.write_u32::<LE>(0).unwrap();
        archive.write_u32::<LE>(0).unwrap();

        let mut output = Cursor::new(Vec::<u8>::new());
        let report = decompress(Cursor::new(&archive), &mut output).unwrap();

        assert_eq!(report.blocks_decoded, 1);
        assert_eq!(report.bytes_out, payload.len() as u64);
        assert_eq!(output.into_inner(), payload);
    }

    /// IO_LZ archive with a back-reference match across blocks.
    /// Block 0: 32 bytes of literal 'A'..='Z' + '0'..='5'
    /// Block 1: empty literals + one match of 32 bytes referencing block 0.
    #[test]
    fn round_trip_io_lz_back_reference() {
        let block0_data: Vec<u8> = (b'A'..=b'Z').chain(b'0'..=b'5').collect();
        assert_eq!(block0_data.len(), 32);

        let mut archive = Vec::<u8>::new();
        let hdr = ArchiveHeader {
            output_mode: OutputMode::IoLz,
            round_matches: false,
            hash: HashKind::None,
            seed: vec![],
            base_len: 32,
            digest_len: 0,
        };
        hdr.write(&mut archive).unwrap();
        let codec = StatCodec { round_matches: false, chunk_len: 32 };

        // Block 0: literal-only
        BlockHeader {
            literal_bytes: 32,
            original_len: 32,
            stat_size: 0,
        }
        .write(&mut archive, &[])
        .unwrap();
        archive.write_all(&block0_data).unwrap();

        // Block 1: 1 match (offset = 32, len = 32) targeting block 0 bytes 0..32
        let mut stats = Vec::new();
        codec.encode(&mut stats, 0, 32, 32).unwrap();
        let stat_bytes = stats.len() * size_of::<u32>();
        BlockHeader {
            literal_bytes: 0,
            original_len: 32,
            stat_size: stat_bytes as u32,
        }
        .write(&mut archive, &[])
        .unwrap();
        for s in &stats {
            archive.write_u32::<LE>(*s).unwrap();
        }

        // EOF sentinel
        archive.write_u32::<LE>(0).unwrap();
        archive.write_u32::<LE>(0).unwrap();
        archive.write_u32::<LE>(0).unwrap();

        let mut output = Cursor::new(Vec::<u8>::new());
        let report = decompress(Cursor::new(&archive), &mut output).unwrap();

        assert_eq!(report.blocks_decoded, 2);
        let mut expected = block0_data.clone();
        expected.extend_from_slice(&block0_data);
        assert_eq!(output.into_inner(), expected);
    }

    /// INDEX_LZ archive: literals inline, single global stats blob at the end.
    #[test]
    fn round_trip_index_lz_back_reference() {
        let block0_data: Vec<u8> = (0u8..32).collect();
        assert_eq!(block0_data.len(), 32);

        let mut archive = Vec::<u8>::new();
        let hdr = ArchiveHeader {
            output_mode: OutputMode::IndexLz,
            round_matches: false,
            hash: HashKind::None,
            seed: vec![],
            base_len: 32,
            digest_len: 0,
        };
        hdr.write(&mut archive).unwrap();
        let codec = StatCodec { round_matches: false, chunk_len: 32 };

        // Block 0: literal-only, stat_size = 0
        BlockHeader {
            literal_bytes: 32,
            original_len: 32,
            stat_size: 0,
        }
        .write(&mut archive, &[])
        .unwrap();
        archive.write_all(&block0_data).unwrap();

        // Block 1: empty literals, stat_size = 0 (INDEX_LZ stores stats in footer)
        BlockHeader {
            literal_bytes: 0,
            original_len: 32,
            stat_size: 0,
        }
        .write(&mut archive, &[])
        .unwrap();
        // No literal payload for block 1.

        // Footer region: [all_stats] [per-block sizes] [6-word trailer]
        let mut all_stats = Vec::<u32>::new();
        // Block 0: 0 stats. Block 1: 1 match (offset=32, len=32).
        let mut block1_stats = Vec::new();
        codec.encode(&mut block1_stats, 0, 32, 32).unwrap();
        all_stats.extend_from_slice(&block1_stats);

        let stat_bytes: u64 = (all_stats.len() * size_of::<u32>()) as u64;
        for s in &all_stats {
            archive.write_u32::<LE>(*s).unwrap();
        }

        // Per-block sizes
        let per_block_sizes: Vec<u32> = vec![0u32, (block1_stats.len() * 4) as u32];
        for s in &per_block_sizes {
            archive.write_u32::<LE>(*s).unwrap();
        }

        let footer_size = (per_block_sizes.len() * 4 + crate::srep::format::IndexLzFooter::ON_DISK_SIZE) as u32;
        let footer = crate::srep::format::IndexLzFooter { stat_size: stat_bytes, footer_size };
        footer.write(&mut archive).unwrap();

        let mut output = Cursor::new(Vec::<u8>::new());
        let report = decompress(Cursor::new(&archive), &mut output).unwrap();

        assert_eq!(report.blocks_decoded, 2);
        let mut expected = block0_data.clone();
        expected.extend_from_slice(&block0_data);
        assert_eq!(output.into_inner(), expected);
    }
}
