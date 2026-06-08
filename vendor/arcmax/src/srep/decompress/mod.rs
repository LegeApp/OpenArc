//! SREP decompression — entry point and per-mode dispatch.
//!
//! Supports `IO_LZ` (versions 1 & 2, ROUND_MATCHES toggled by version) and
//! `INDEX_LZ` (version 4). `FUTURE_LZ` (version 3) is deferred to a later
//! stage and currently returns `SrepError::Unsupported`.

pub mod block;
pub mod index_lz;
pub mod io_lz;

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
pub fn decompress<R, W>(mut input: R, mut output: W) -> Result<DecompressionReport, SrepError>
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
        OutputMode::FutureLz => Err(SrepError::Unsupported(
            "FUTURE_LZ decompression not yet implemented",
        )),
    }
}
