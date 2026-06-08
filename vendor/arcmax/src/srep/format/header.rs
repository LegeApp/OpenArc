use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::io::{Read, Seek, SeekFrom, Write};

use crate::srep::config::{HashKind, OutputMode};
use crate::srep::error::SrepError;
use crate::srep::format::constants::*;
use crate::srep::types::Stat;

/// Decoded representation of the SREP archive header (first 16+ bytes).
///
/// On-disk layout (little-endian u32 words):
///   [0] BULAT_ZIGANSHIN_SIGNATURE (0x26351817)
///   [1] SREP_SIGNATURE            (0x50455253)
///   [2] packed: bits  0- 7 = format_version
///                     8-15 = hash_num
///                    16-23 = hash_seed_size
///                    24-31 = (hash_digest_len - 16) & 0xFF
///   [3] base_len
///   [4..4+seed_size] hash seed bytes
#[derive(Debug, Clone)]
pub struct ArchiveHeader {
    pub output_mode: OutputMode,
    pub round_matches: bool,
    pub hash: HashKind,
    /// Random seed for keyed hashes (VMAC / SipHash). Empty for unkeyed hashes.
    pub seed: Vec<u8>,
    /// `BASE_LEN` = minimum guaranteed match length (= chunk_len for -m3).
    pub base_len: u32,
    /// Digest length in bytes per block (from the archive, may differ from HashKind default).
    pub digest_len: usize,
}

impl ArchiveHeader {
    pub fn read<R: Read>(r: &mut R) -> Result<Self, SrepError> {
        let sig0 = r.read_u32::<LE>()?;
        let sig1 = r.read_u32::<LE>()?;
        if sig0 != BULAT_ZIGANSHIN_SIGNATURE || sig1 != SREP_SIGNATURE {
            return Err(SrepError::Format("not an SREP archive: invalid signature"));
        }

        let packed = r.read_u32::<LE>()?;
        let format_version = (packed & 0xFF) as u8;
        let hash_num = ((packed >> 8) & 0xFF) as u8;
        let seed_size = ((packed >> 16) & 0xFF) as usize;
        // C++ stores (hash_size - 16) as a u8, allowing wraparound for small
        // digests like SipHash's 8 bytes (encoded as 248). Decode mirrors that.
        let digest_stored = ((packed >> 24) & 0xFF) as usize;
        let digest_len = (digest_stored + 16) & 0xFF;
        let base_len = r.read_u32::<LE>()?;

        let (output_mode, round_matches) = match format_version {
            FORMAT_VERSION_IO_LZ_ROUND => (OutputMode::IoLz, true),
            FORMAT_VERSION_IO_LZ_NONROUND => (OutputMode::IoLz, false),
            FORMAT_VERSION_FUTURE_LZ => (OutputMode::FutureLz, false),
            FORMAT_VERSION_INDEX_LZ => (OutputMode::IndexLz, false),
            v => {
                return Err(SrepError::Format(if v < FORMAT_VERSION_IO_LZ_ROUND {
                    "unsupported archive format version (too old)"
                } else {
                    "unsupported archive format version (too new)"
                }))
            }
        };

        let hash = HashKind::from_archive_num(hash_num).unwrap_or(HashKind::None);

        let mut seed = vec![0u8; seed_size];
        if seed_size > 0 {
            r.read_exact(&mut seed)?;
        }

        Ok(ArchiveHeader {
            output_mode,
            round_matches,
            hash,
            seed,
            base_len,
            digest_len,
        })
    }

    pub fn write<W: Write>(&self, w: &mut W) -> Result<(), SrepError> {
        w.write_u32::<LE>(BULAT_ZIGANSHIN_SIGNATURE)?;
        w.write_u32::<LE>(SREP_SIGNATURE)?;

        let format_version: u8 = match (self.output_mode, self.round_matches) {
            (OutputMode::IoLz, true) => FORMAT_VERSION_IO_LZ_ROUND,
            (OutputMode::IoLz, false) => FORMAT_VERSION_IO_LZ_NONROUND,
            (OutputMode::FutureLz, _) => FORMAT_VERSION_FUTURE_LZ,
            (OutputMode::IndexLz, _) => FORMAT_VERSION_INDEX_LZ,
        };

        let hash_num = self.hash.archive_num();
        let seed_size = self.seed.len() as u8;
        // Matches C++: stored byte = (hash_size - 16) with u8 wraparound so
        // sub-16 digests (SipHash = 8 → 248) round-trip correctly.
        let digest_stored = (self.digest_len as u8).wrapping_sub(16);

        let packed: Stat = (format_version as u32)
            | ((hash_num as u32) << 8)
            | ((seed_size as u32) << 16)
            | ((digest_stored as u32) << 24);

        w.write_u32::<LE>(packed)?;
        w.write_u32::<LE>(self.base_len)?;

        if !self.seed.is_empty() {
            w.write_all(&self.seed)?;
        }
        Ok(())
    }

    /// Total bytes occupied by the archive header (fixed words + seed).
    pub fn on_disk_size(&self) -> usize {
        ARCHIVE_HEADER_WORDS * size_of::<Stat>() + self.seed.len()
    }
}

/// The 24-byte INDEX_LZ footer (6 STAT words, written at the very end of the archive).
///
/// Layout (all little-endian u32):
///   [0] stat_size low 32 bits   — total bytes of all stat arrays
///   [1] stat_size high 32 bits
///   [2] footer_size             — total bytes of footer region (stats + block-size list + this record)
///   [3] footer_version          — must be FOOTER_VERSION (1)
///   [4] ~SREP_SIGNATURE
///   [5] ~BULAT_ZIGANSHIN_SIGNATURE
#[derive(Debug, Clone, Copy)]
pub struct IndexLzFooter {
    /// Total bytes of the concatenated per-block STAT arrays.
    pub stat_size: u64,
    /// Total bytes of the footer region (stat data + per-block sizes + these 6 words).
    pub footer_size: u32,
}

impl IndexLzFooter {
    pub const ON_DISK_SIZE: usize = INDEX_LZ_FOOTER_WORDS * size_of::<Stat>();

    pub fn read<R: Read + Seek>(r: &mut R, file_size: u64) -> Result<Self, SrepError> {
        r.seek(SeekFrom::Start(file_size - Self::ON_DISK_SIZE as u64))?;

        let stat_lo = r.read_u32::<LE>()?;
        let stat_hi = r.read_u32::<LE>()?;
        let footer_size = r.read_u32::<LE>()?;
        let footer_version = r.read_u32::<LE>()?;
        let check_sig = r.read_u32::<LE>()?;
        let check_bulat = r.read_u32::<LE>()?;

        if check_sig != !SREP_SIGNATURE || check_bulat != !BULAT_ZIGANSHIN_SIGNATURE {
            return Err(SrepError::Format("INDEX_LZ footer signature mismatch"));
        }
        if footer_version != FOOTER_VERSION as u32 {
            return Err(SrepError::Format("unsupported INDEX_LZ footer version"));
        }

        Ok(IndexLzFooter {
            stat_size: stat_lo as u64 | ((stat_hi as u64) << 32),
            footer_size,
        })
    }

    pub fn write<W: Write>(&self, w: &mut W) -> Result<(), SrepError> {
        w.write_u32::<LE>(self.stat_size as u32)?;
        w.write_u32::<LE>((self.stat_size >> 32) as u32)?;
        w.write_u32::<LE>(self.footer_size)?;
        w.write_u32::<LE>(FOOTER_VERSION as u32)?;
        w.write_u32::<LE>(!SREP_SIGNATURE)?;
        w.write_u32::<LE>(!BULAT_ZIGANSHIN_SIGNATURE)?;
        Ok(())
    }
}
