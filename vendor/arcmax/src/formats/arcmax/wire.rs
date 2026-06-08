//! Wire-format types for the ArcMax archive format.
//!
//! All multi-byte integers are little-endian. No serde — hand-rolled for
//! guaranteed stable output.

use std::io::{Read, Write};

use crate::error::{ArcError, Result};

// ---------------------------------------------------------------------------
// Magic constants
// ---------------------------------------------------------------------------

pub const STREAM_MAGIC: &[u8; 8] = b"ARCMAX\x01\x00";
pub const BLOCK_MAGIC: &[u8; 4] = b"AMXB";
pub const LOCATOR_MAGIC: &[u8; 4] = b"AMXL";

pub const LOCATOR_SIZE: usize = 16;
pub const FOOTER_SIZE: usize = 64;

pub const FLAG_ENCRYPTED: u8 = 0b0000_0001;
pub const FLAG_RECOVERY: u8 = 0b0000_0010;

// ---------------------------------------------------------------------------
// Locator — last 16 bytes of the archive
// ---------------------------------------------------------------------------

/// The final 16 bytes of an ArcMax archive, always readable with a 16-byte
/// seek-to-end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locator {
    pub footer_offset: u64,
    pub footer_size: u32,
}

impl Locator {
    pub fn serialize(&self, out: &mut impl Write) -> Result<()> {
        out.write_all(LOCATOR_MAGIC)?;
        out.write_all(&self.footer_offset.to_le_bytes())?;
        out.write_all(&self.footer_size.to_le_bytes())?;
        Ok(())
    }

    pub fn deserialize(buf: &[u8; LOCATOR_SIZE]) -> Result<Self> {
        if &buf[0..4] != LOCATOR_MAGIC {
            return Err(ArcError::InvalidArchive("bad locator magic"));
        }
        let footer_offset = u64::from_le_bytes(buf[4..12].try_into().unwrap());
        let footer_size = u32::from_le_bytes(buf[12..16].try_into().unwrap());
        Ok(Self {
            footer_offset,
            footer_size,
        })
    }
}

// ---------------------------------------------------------------------------
// Footer — fixed 64 bytes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Footer {
    pub version: u8,
    pub flags: u8,
    pub num_blocks: u32,
    pub num_entries: u32,
    pub cd_offset: u64,
    pub cd_size: u32,
    pub cd_compressed: u32,
    pub cd_checksum: [u8; 16],
    pub created_at: i64,
}

impl Footer {
    pub fn serialize(&self, out: &mut impl Write) -> Result<()> {
        let mut buf = [0u8; FOOTER_SIZE];
        buf[0] = self.version;
        buf[1] = self.flags;
        buf[2..6].copy_from_slice(&self.num_blocks.to_le_bytes());
        buf[6..10].copy_from_slice(&self.num_entries.to_le_bytes());
        buf[10..18].copy_from_slice(&self.cd_offset.to_le_bytes());
        buf[18..22].copy_from_slice(&self.cd_size.to_le_bytes());
        buf[22..26].copy_from_slice(&self.cd_compressed.to_le_bytes());
        buf[26..42].copy_from_slice(&self.cd_checksum);
        buf[42..50].copy_from_slice(&self.created_at.to_le_bytes());
        // bytes 50..64 stay zero (padding)
        out.write_all(&buf)?;
        Ok(())
    }

    pub fn deserialize(buf: &[u8; FOOTER_SIZE]) -> Result<Self> {
        let version = buf[0];
        if version != 1 {
            return Err(ArcError::InvalidArchive("unsupported footer version"));
        }
        let flags = buf[1];
        let num_blocks = u32::from_le_bytes(buf[2..6].try_into().unwrap());
        let num_entries = u32::from_le_bytes(buf[6..10].try_into().unwrap());
        let cd_offset = u64::from_le_bytes(buf[10..18].try_into().unwrap());
        let cd_size = u32::from_le_bytes(buf[18..22].try_into().unwrap());
        let cd_compressed = u32::from_le_bytes(buf[22..26].try_into().unwrap());
        let cd_checksum: [u8; 16] = buf[26..42].try_into().unwrap();
        let created_at = i64::from_le_bytes(buf[42..50].try_into().unwrap());
        Ok(Self {
            version,
            flags,
            num_blocks,
            num_entries,
            cd_offset,
            cd_size,
            cd_compressed,
            cd_checksum,
            created_at,
        })
    }
}

// ---------------------------------------------------------------------------
// FileEntry — one record in the central directory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub original_size: u64,
    pub mtime_secs: i64,
    pub mtime_nanos: u32,
    pub permissions: u32,
    pub blake3_hash: [u8; 32],
    pub block_id: u32,
    pub offset_in_block: u64,
    pub size_in_block: u64,
}

impl FileEntry {
    pub fn serialize(&self, out: &mut impl Write) -> Result<()> {
        let path_bytes = self.path.as_bytes();
        let path_len = path_bytes.len() as u16;
        out.write_all(&path_len.to_le_bytes())?;
        out.write_all(path_bytes)?;
        out.write_all(&self.original_size.to_le_bytes())?;
        out.write_all(&self.mtime_secs.to_le_bytes())?;
        out.write_all(&self.mtime_nanos.to_le_bytes())?;
        out.write_all(&self.permissions.to_le_bytes())?;
        out.write_all(&self.blake3_hash)?;
        out.write_all(&self.block_id.to_le_bytes())?;
        out.write_all(&self.offset_in_block.to_le_bytes())?;
        out.write_all(&self.size_in_block.to_le_bytes())?;
        Ok(())
    }

    pub fn deserialize(r: &mut impl Read) -> Result<Self> {
        let mut len_buf = [0u8; 2];
        r.read_exact(&mut len_buf)?;
        let path_len = u16::from_le_bytes(len_buf) as usize;
        let mut path_buf = vec![0u8; path_len];
        r.read_exact(&mut path_buf)?;
        let path = String::from_utf8(path_buf)
            .map_err(|_| ArcError::InvalidArchive("non-UTF-8 path in central directory"))?;

        let original_size = read_u64(r)?;
        let mtime_secs = read_i64(r)?;
        let mtime_nanos = read_u32(r)?;
        let permissions = read_u32(r)?;
        let mut blake3_hash = [0u8; 32];
        r.read_exact(&mut blake3_hash)?;
        let block_id = read_u32(r)?;
        let offset_in_block = read_u64(r)?;
        let size_in_block = read_u64(r)?;

        Ok(Self {
            path,
            original_size,
            mtime_secs,
            mtime_nanos,
            permissions,
            blake3_hash,
            block_id,
            offset_in_block,
            size_in_block,
        })
    }
}

// ---------------------------------------------------------------------------
// BlockHeader — prepended to each compressed block on disk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub block_id: u32,
    pub method: String,
    pub uncompressed_sz: u64,
    pub compressed_sz: u64,
    pub block_checksum: u32,
}

impl BlockHeader {
    pub fn serialized_size(&self) -> usize {
        4 + 4 + 2 + self.method.len() + 8 + 8 + 4
    }

    pub fn serialize(&self, out: &mut impl Write) -> Result<()> {
        out.write_all(BLOCK_MAGIC)?;
        out.write_all(&self.block_id.to_le_bytes())?;
        let method_bytes = self.method.as_bytes();
        out.write_all(&(method_bytes.len() as u16).to_le_bytes())?;
        out.write_all(method_bytes)?;
        out.write_all(&self.uncompressed_sz.to_le_bytes())?;
        out.write_all(&self.compressed_sz.to_le_bytes())?;
        out.write_all(&self.block_checksum.to_le_bytes())?;
        Ok(())
    }

    pub fn deserialize(r: &mut impl Read) -> Result<Self> {
        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if &magic != BLOCK_MAGIC {
            return Err(ArcError::InvalidArchive("bad block magic"));
        }
        let block_id = read_u32(r)?;
        let method_len = read_u16(r)? as usize;
        let mut method_buf = vec![0u8; method_len];
        r.read_exact(&mut method_buf)?;
        let method = String::from_utf8(method_buf)
            .map_err(|_| ArcError::InvalidArchive("non-UTF-8 method in block header"))?;
        let uncompressed_sz = read_u64(r)?;
        let compressed_sz = read_u64(r)?;
        let block_checksum = read_u32(r)?;
        Ok(Self {
            block_id,
            method,
            uncompressed_sz,
            compressed_sz,
            block_checksum,
        })
    }
}

// ---------------------------------------------------------------------------
// I/O helpers
// ---------------------------------------------------------------------------

fn read_u16(r: &mut impl Read) -> Result<u16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32(r: &mut impl Read) -> Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64(r: &mut impl Read) -> Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_i64(r: &mut impl Read) -> Result<i64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(i64::from_le_bytes(buf))
}
