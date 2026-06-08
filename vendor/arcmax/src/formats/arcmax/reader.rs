use std::io::{Read, Seek, SeekFrom};
use std::str::FromStr;

use crate::error::{ArcError, Result};
use crate::formats::arcmax::index::CentralDirectory;
use crate::formats::arcmax::wire::{
    BlockHeader, FileEntry, Footer, Locator, FOOTER_SIZE, LOCATOR_SIZE, STREAM_MAGIC,
};
use crate::method::{CodecPipeline, Method};

/// Progress event emitted by [`ArchiveReader::extract_all`] for each file.
#[derive(Debug, Clone)]
pub struct ExtractProgress {
    /// 0-based index of the file just extracted.
    pub file_index: usize,
    /// Total number of files in the archive.
    pub total_files: usize,
    /// Archive path of the file just extracted.
    pub path: String,
    /// Uncompressed size of the file just extracted.
    pub file_bytes: u64,
    /// Total uncompressed bytes yielded so far (including this file).
    pub total_bytes_done: u64,
    /// Total uncompressed bytes in all files.
    pub total_bytes: u64,
    /// 0-based block id that contained this file.
    pub block_id: u32,
    /// Total number of blocks in the archive.
    pub total_blocks: u32,
}

/// Reads an ArcMax archive from any `Read + Seek` source.
pub struct ArchiveReader<R: Read + Seek> {
    source: R,
    footer: Footer,
    entries: Vec<FileEntry>,
    /// Byte offsets of each block header in the source stream (block_id → offset).
    block_offsets: Vec<u64>,
    /// One-slot block cache: (block_id, decompressed_data).
    ///
    /// Avoids re-decompressing the same solid block when multiple consecutive
    /// calls to `extract()` target files in the same block.
    block_cache: Option<(u32, Vec<u8>)>,
}

impl<R: Read + Seek> ArchiveReader<R> {
    /// Open an archive.
    ///
    /// This reads the locator (last 16 bytes), the footer, and the central
    /// directory. Block data is not read until [`extract`] is called.
    pub fn open(mut source: R) -> Result<Self> {
        // Verify stream magic.
        let mut magic = [0u8; 8];
        source.read_exact(&mut magic)?;
        if &magic != STREAM_MAGIC {
            return Err(ArcError::InvalidArchive("bad stream magic"));
        }

        // Seek to the locator at the very end.
        source.seek(SeekFrom::End(-(LOCATOR_SIZE as i64)))?;
        let mut loc_buf = [0u8; LOCATOR_SIZE];
        source.read_exact(&mut loc_buf)?;
        let locator = Locator::deserialize(&loc_buf)?;

        // Read the footer.
        source.seek(SeekFrom::Start(locator.footer_offset))?;
        let mut footer_buf = [0u8; FOOTER_SIZE];
        source.read_exact(&mut footer_buf)?;
        let footer = Footer::deserialize(&footer_buf)?;

        // Read and deserialize the central directory.
        source.seek(SeekFrom::Start(footer.cd_offset))?;
        let cd_len = footer.cd_size as usize;
        let mut cd_bytes = vec![0u8; cd_len];
        source.read_exact(&mut cd_bytes)?;

        // Verify central directory checksum.
        let actual_hash = blake3::hash(&cd_bytes);
        if actual_hash.as_bytes()[..16] != footer.cd_checksum {
            return Err(ArcError::ChecksumMismatch {
                expected: u32::from_le_bytes(footer.cd_checksum[..4].try_into().unwrap()),
                actual: u32::from_le_bytes(actual_hash.as_bytes()[..4].try_into().unwrap()),
            });
        }

        let cd = CentralDirectory::deserialize(&cd_bytes)?;
        let entries: Vec<FileEntry> = cd.entries().to_vec();

        // Build a block-offset index by scanning block headers in order.
        let block_offsets = build_block_offsets(&mut source, footer.num_blocks, &entries)?;

        Ok(Self {
            source,
            footer,
            entries,
            block_offsets,
            block_cache: None,
        })
    }

    /// List all file entries in the archive.
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// Extract a single file by path, decompressing only the block it lives in.
    ///
    /// Returns `Err` if the path is not found or if BLAKE3 integrity fails.
    pub fn extract(&mut self, path: &str) -> Result<Vec<u8>> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.path == path)
            .ok_or_else(|| ArcError::InvalidArchive("path not found in archive"))?
            .clone();

        let block_data = self.decompress_block(entry.block_id)?;

        let start = entry.offset_in_block as usize;
        let end = start + entry.size_in_block as usize;
        if end > block_data.len() {
            return Err(ArcError::InvalidArchive("file extent exceeds block size"));
        }
        let file_bytes = block_data[start..end].to_vec();

        // Verify BLAKE3 integrity.
        let actual_hash = blake3::hash(&file_bytes);
        if actual_hash.as_bytes() != &entry.blake3_hash {
            return Err(ArcError::ChecksumMismatch {
                expected: u32::from_le_bytes(entry.blake3_hash[..4].try_into().unwrap()),
                actual: u32::from_le_bytes(actual_hash.as_bytes()[..4].try_into().unwrap()),
            });
        }

        Ok(file_bytes)
    }

    /// Return the footer (archive metadata).
    pub fn footer(&self) -> &Footer {
        &self.footer
    }

    // --- internal -----------------------------------------------------------

    fn decompress_block(&mut self, block_id: u32) -> Result<Vec<u8>> {
        // Return cached data if we already decompressed this block.
        if let Some((cached_id, ref data)) = self.block_cache {
            if cached_id == block_id {
                return Ok(data.clone());
            }
        }

        let offset = *self
            .block_offsets
            .get(block_id as usize)
            .ok_or(ArcError::InvalidArchive("block_id out of range"))?;

        self.source.seek(SeekFrom::Start(offset))?;
        let header = BlockHeader::deserialize(&mut self.source)?;

        let mut compressed = vec![0u8; header.compressed_sz as usize];
        self.source.read_exact(&mut compressed)?;

        // Verify block CRC32.
        let actual_crc = crc32fast::hash(&compressed);
        if actual_crc != header.block_checksum {
            return Err(ArcError::ChecksumMismatch {
                expected: header.block_checksum,
                actual: actual_crc,
            });
        }

        let method = Method::from_str(&header.method)
            .map_err(|e| ArcError::InvalidMethod(format!("bad method in block header: {}", e)))?;
        let mut pipeline = CodecPipeline::new(method);
        let mut decompressed = Vec::new();
        pipeline.decompress(&compressed[..], &mut decompressed)?;

        self.block_cache = Some((block_id, decompressed.clone()));
        Ok(decompressed)
    }

    /// Extract all files, calling `on_file` for each one.
    ///
    /// Unlike calling [`extract`] in a loop, this decompresses each solid block
    /// exactly once regardless of how many files it contains.  Entries are
    /// processed in block order (block 0, then 1, …), which is also their
    /// insertion order.
    ///
    /// `on_file` receives an [`ExtractProgress`] and the file data.  Return an
    /// error from `on_file` to abort early.
    ///
    /// The closure may return any error type `E` as long as `E: From<ArcError>`,
    /// so callers using `anyhow` can use `?` with `with_context()` naturally.
    pub fn extract_all<E, F>(&mut self, mut on_file: F) -> std::result::Result<(), E>
    where
        E: From<ArcError>,
        F: FnMut(ExtractProgress, Vec<u8>) -> std::result::Result<(), E>,
    {
        // Pre-compute totals for progress reporting.
        let total_files = self.entries.len();
        let total_bytes: u64 = self.entries.iter().map(|e| e.original_size).sum();
        let total_blocks = self.footer.num_blocks;

        // Sort entries by (block_id, offset_in_block) so each block is
        // decompressed exactly once in a single sequential pass.
        let mut order: Vec<usize> = (0..total_files).collect();
        order.sort_by_key(|&i| (self.entries[i].block_id, self.entries[i].offset_in_block));

        let mut bytes_done: u64 = 0;

        for (seq_index, &entry_index) in order.iter().enumerate() {
            let entry = self.entries[entry_index].clone();

            let block_data = self.decompress_block(entry.block_id)?;

            let start = entry.offset_in_block as usize;
            let end = start + entry.size_in_block as usize;
            if end > block_data.len() {
                return Err(ArcError::InvalidArchive("file extent exceeds block size").into());
            }
            let file_bytes = block_data[start..end].to_vec();

            // Verify BLAKE3 integrity.
            let actual_hash = blake3::hash(&file_bytes);
            if actual_hash.as_bytes() != &entry.blake3_hash {
                return Err(ArcError::ChecksumMismatch {
                    expected: u32::from_le_bytes(entry.blake3_hash[..4].try_into().unwrap()),
                    actual: u32::from_le_bytes(actual_hash.as_bytes()[..4].try_into().unwrap()),
                }
                .into());
            }

            bytes_done += entry.original_size;

            on_file(
                ExtractProgress {
                    file_index: seq_index,
                    total_files,
                    path: entry.path,
                    file_bytes: entry.original_size,
                    total_bytes_done: bytes_done,
                    total_bytes,
                    block_id: entry.block_id,
                    total_blocks,
                },
                file_bytes,
            )?;
        }

        Ok(())
    }
}

/// Scan block headers in sequence to build a `block_id → file offset` table.
///
/// Block headers are written consecutively starting right after the stream
/// magic (at offset 8). Each header is followed by `compressed_sz` bytes of
/// payload. We scan until we have found all `num_blocks` blocks or run out of
/// data before reaching `cd_offset`.
fn build_block_offsets<R: Read + Seek>(
    source: &mut R,
    num_blocks: u32,
    entries: &[FileEntry],
) -> Result<Vec<u64>> {
    if num_blocks == 0 {
        return Ok(Vec::new());
    }

    // Determine the upper bound: the smallest `offset_in_block` plus its
    // `block_id` entry's block start. We simply scan from offset 8 (after
    // stream magic) and collect in order.
    source.seek(SeekFrom::Start(STREAM_MAGIC.len() as u64))?;

    let mut offsets = Vec::with_capacity(num_blocks as usize);

    for _ in 0..num_blocks {
        let here = source.stream_position()?;
        offsets.push(here);

        let header = BlockHeader::deserialize(source)?;
        // Skip over the compressed payload.
        source.seek(SeekFrom::Current(header.compressed_sz as i64))?;
    }

    let _ = entries; // entries are not needed for offset scan
    Ok(offsets)
}
