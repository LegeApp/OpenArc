use std::collections::HashMap;
use std::fmt;
use std::io::{Seek, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;
use crate::filetype::{detect, suggest_pipeline, SuggestionConfig};
use crate::formats::arcmax::index::CentralDirectory;
use crate::formats::arcmax::wire::{
    BlockHeader, FileEntry, Footer, Locator, FOOTER_SIZE, STREAM_MAGIC,
};
use crate::method::{CodecPipeline, Method};

/// Default solid-block size threshold: 64 MiB.
pub const DEFAULT_BLOCK_THRESHOLD: usize = 64 * 1024 * 1024;

/// Progress event emitted after each solid block is compressed and written.
#[derive(Debug, Clone)]
pub struct WriteProgress {
    /// 0-based index of the block just written.
    pub block_id: u32,
    /// Number of files packed into this block.
    pub files_in_block: usize,
    /// Uncompressed size of this block in bytes.
    pub block_uncompressed_bytes: u64,
    /// Compressed size of this block in bytes (as written to disk).
    pub block_compressed_bytes: u64,
    /// Total uncompressed bytes flushed so far (including this block).
    pub total_uncompressed_bytes: u64,
    /// Compression ratio for this block (compressed / uncompressed).
    pub block_ratio: f64,
}

/// Writes an ArcMax archive to any `Write + Seek` sink.
///
/// Call [`add_file`] for each file, then [`finish`] to flush and write the
/// central directory, footer, and locator.
pub struct ArchiveWriter<W: Write + Seek> {
    output: W,
    block_threshold: usize,
    default_method: Method,
    /// Accumulated uncompressed data for the current solid block.
    block_buf: Vec<u8>,
    /// Per-file slices within `block_buf`.
    pending: Vec<PendingEntry>,
    block_id: u32,
    cd: CentralDirectory,
    /// Whether content-addressed dedup is enabled.
    dedup: bool,
    /// Cross-block dedup: hash → (block_id, offset_in_block, size) for files in flushed blocks.
    seen_hashes: HashMap<[u8; 32], (u32, u64, u64)>,
    /// Within-block dedup: hash → (offset_in_block_buf, len) for files in the current pending block.
    block_hashes: HashMap<[u8; 32], (usize, usize)>,
    /// Total uncompressed bytes flushed across all blocks so far.
    total_uncompressed: u64,
    /// Optional callback invoked after each block is compressed and written.
    progress_fn: Option<Box<dyn FnMut(WriteProgress) + Send + 'static>>,
}

struct PendingEntry {
    path: String,
    offset: usize,
    len: usize,
    mtime_secs: i64,
    mtime_nanos: u32,
    permissions: u32,
}

impl<W: Write + Seek> ArchiveWriter<W> {
    /// Create a new archive writer. Writes the stream magic immediately.
    pub fn new(mut output: W) -> Result<Self> {
        output.write_all(STREAM_MAGIC)?;
        Ok(Self {
            output,
            block_threshold: DEFAULT_BLOCK_THRESHOLD,
            default_method: Method::Store,
            block_buf: Vec::new(),
            pending: Vec::new(),
            block_id: 0,
            cd: CentralDirectory::new(),
            dedup: false,
            seen_hashes: HashMap::new(),
            block_hashes: HashMap::new(),
            total_uncompressed: 0,
            progress_fn: None,
        })
    }

    /// Register a callback invoked after each solid block is compressed and written.
    ///
    /// The callback receives a [`WriteProgress`] describing the block that was
    /// just flushed. Use this to drive a progress bar or log compression ratios.
    pub fn with_progress<F: FnMut(WriteProgress) + Send + 'static>(mut self, f: F) -> Self {
        self.progress_fn = Some(Box::new(f));
        self
    }

    /// Override the solid-block size threshold (default 64 MiB).
    pub fn with_block_threshold(mut self, bytes: usize) -> Self {
        self.block_threshold = bytes;
        self
    }

    /// Override the default compression method for blocks.
    pub fn with_method(mut self, method: Method) -> Self {
        self.default_method = method;
        self
    }

    /// Enable content-addressed file-level deduplication.
    ///
    /// When enabled, adding a file whose BLAKE3 hash matches a previously
    /// added file stores only a reference — no duplicate bytes are written to
    /// any block.  Dedup is across blocks as well as within a single block.
    pub fn with_dedup(mut self) -> Self {
        self.dedup = true;
        self
    }

    /// Add a file to the archive.
    ///
    /// The data is accumulated in an internal buffer. When the buffer exceeds
    /// the block threshold, the block is automatically flushed.
    pub fn add_file(&mut self, path: &str, data: &[u8]) -> Result<()> {
        self.add_file_with_meta(path, data, 0, 0, 0)
    }

    /// Add a file with metadata (mtime and Unix permissions).
    pub fn add_file_with_meta(
        &mut self,
        path: &str,
        data: &[u8],
        mtime_secs: i64,
        mtime_nanos: u32,
        permissions: u32,
    ) -> Result<()> {
        if self.dedup {
            let hash = *blake3::hash(data).as_bytes();

            // Cross-block hit: identical content was written in a previous block.
            if let Some(&(block_id, offset_in_block, size_in_block)) = self.seen_hashes.get(&hash) {
                self.cd.push(FileEntry {
                    path: path.to_string(),
                    original_size: data.len() as u64,
                    mtime_secs,
                    mtime_nanos,
                    permissions,
                    blake3_hash: hash,
                    block_id,
                    offset_in_block,
                    size_in_block,
                });
                return Ok(());
            }

            // Within-block hit: identical content is already in the current block buffer.
            if let Some(&(existing_offset, existing_len)) = self.block_hashes.get(&hash) {
                self.pending.push(PendingEntry {
                    path: path.to_string(),
                    offset: existing_offset,
                    len: existing_len,
                    mtime_secs,
                    mtime_nanos,
                    permissions,
                });
                // No new data; no size change; no threshold check needed.
                return Ok(());
            }

            // First occurrence: record before extending the buffer.
            self.block_hashes
                .insert(hash, (self.block_buf.len(), data.len()));
        }

        let offset = self.block_buf.len();
        self.block_buf.extend_from_slice(data);
        self.pending.push(PendingEntry {
            path: path.to_string(),
            offset,
            len: data.len(),
            mtime_secs,
            mtime_nanos,
            permissions,
        });

        if self.block_buf.len() >= self.block_threshold {
            self.flush_block()?;
        }
        Ok(())
    }

    /// Resolve the concrete method for a block.
    ///
    /// When `default_method` is `Method::Auto`, detect the first pending file's
    /// type and run it through the filetype router.  Otherwise return the
    /// configured method unchanged.
    fn resolve_method(&self, block_buf: &[u8], pending: &[PendingEntry]) -> Method {
        match &self.default_method {
            Method::Auto => {
                if let Some(pe) = pending.first() {
                    let file_start = pe.offset;
                    let file_end = (pe.offset + pe.len).min(block_buf.len());
                    let header = &block_buf[file_start..file_end.min(file_start + 16)];
                    let hint = detect(&pe.path, header);
                    let cfg = SuggestionConfig::default().with_input_size(block_buf.len() as u64);
                    suggest_pipeline(hint, &cfg)
                } else {
                    Method::Store
                }
            }
            other => other.clone(),
        }
    }

    /// Flush the current solid block (if non-empty) to the output.
    ///
    /// Compresses with the configured method, writes a block header, the
    /// compressed bytes, and records all pending file entries in the central
    /// directory.
    pub fn flush_block(&mut self) -> Result<()> {
        if self.block_buf.is_empty() {
            return Ok(());
        }

        let uncompressed = std::mem::take(&mut self.block_buf);
        let pending = std::mem::take(&mut self.pending);
        let block_id = self.block_id;
        self.block_id += 1;

        // Resolve the method — Auto uses the first pending file's type hint.
        let method = self.resolve_method(&uncompressed, &pending);
        let method_str = format!("{}", MethodDisplay(&method));
        let mut pipeline = CodecPipeline::new(method);
        let mut compressed = Vec::new();
        pipeline.compress(&uncompressed[..], &mut compressed)?;

        let block_checksum = crc32fast::hash(&compressed);

        // Remember where this block starts (after the header).
        let block_header = BlockHeader {
            block_id,
            method: method_str,
            uncompressed_sz: uncompressed.len() as u64,
            compressed_sz: compressed.len() as u64,
            block_checksum,
        };

        let block_start = self.output.stream_position()?;
        block_header.serialize(&mut self.output)?;
        self.output.write_all(&compressed)?;

        let _ = block_start; // block_start not stored in FileEntry per design

        // Fire progress callback if registered.
        let uncompressed_len = uncompressed.len() as u64;
        let compressed_len = compressed.len() as u64;
        self.total_uncompressed += uncompressed_len;
        if let Some(ref mut cb) = self.progress_fn {
            cb(WriteProgress {
                block_id,
                files_in_block: pending.len(),
                block_uncompressed_bytes: uncompressed_len,
                block_compressed_bytes: compressed_len,
                total_uncompressed_bytes: self.total_uncompressed,
                block_ratio: compressed_len as f64 / uncompressed_len.max(1) as f64,
            });
        }

        // Register every file that lived in this block.
        for pe in pending {
            let hash_bytes = blake3::hash(&uncompressed[pe.offset..pe.offset + pe.len]);
            let mut blake3_hash = [0u8; 32];
            blake3_hash.copy_from_slice(hash_bytes.as_bytes());

            let entry = FileEntry {
                path: pe.path,
                original_size: pe.len as u64,
                mtime_secs: pe.mtime_secs,
                mtime_nanos: pe.mtime_nanos,
                permissions: pe.permissions,
                blake3_hash,
                block_id,
                offset_in_block: pe.offset as u64,
                size_in_block: pe.len as u64,
            };

            if self.dedup {
                // Record this location so future duplicate files can reference it.
                self.seen_hashes.entry(blake3_hash).or_insert((
                    block_id,
                    pe.offset as u64,
                    pe.len as u64,
                ));
            }

            self.cd.push(entry);
        }

        if self.dedup {
            self.block_hashes.clear();
        }

        Ok(())
    }

    /// Flush all remaining data, write the central directory, footer, and
    /// locator. Consumes the writer.
    pub fn finish(mut self) -> Result<W> {
        self.flush_block()?;

        let num_blocks = self.block_id;
        let num_entries = self.cd.len() as u32;

        // Central directory.
        let cd_bytes = self.cd.serialize()?;
        let cd_checksum_hash = blake3::hash(&cd_bytes);
        let mut cd_checksum = [0u8; 16];
        cd_checksum.copy_from_slice(&cd_checksum_hash.as_bytes()[..16]);

        let cd_offset = self.output.stream_position()?;
        self.output.write_all(&cd_bytes)?;

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Footer.
        let footer_offset = self.output.stream_position()?;
        let footer = Footer {
            version: 1,
            flags: 0,
            num_blocks,
            num_entries,
            cd_offset,
            cd_size: cd_bytes.len() as u32,
            cd_compressed: 0,
            cd_checksum,
            created_at,
        };
        footer.serialize(&mut self.output)?;

        // Locator.
        let locator = Locator {
            footer_offset,
            footer_size: FOOTER_SIZE as u32,
        };
        locator.serialize(&mut self.output)?;

        Ok(self.output)
    }
}

// We need `Display` for Method to produce the method string. Rather than
// duplicating logic, use the existing `fmt::Display` impl.
struct MethodDisplay<'a>(&'a Method);

impl<'a> fmt::Display for MethodDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0, f)
    }
}
