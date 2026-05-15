use byteorder::{LE, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

use crate::srep::error::SrepError;

/// The per-block header written before each compressed block in the archive.
///
/// On-disk layout (all little-endian u32):
///   [0] literal_bytes   — unmatched input bytes in this block
///   [1] original_len    — total uncompressed bytes in this block
///   [2] stat_size       — bytes of STAT stream that follow (0 for INDEX_LZ/FUTURE_LZ)
///   [3..3+digest_len]   — block integrity hash (digest_len bytes, may be 0)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockHeader {
    pub literal_bytes: u32,
    pub original_len: u32,
    /// Bytes of inline STAT data (0 for INDEX_LZ and FUTURE_LZ modes).
    pub stat_size: u32,
}

impl BlockHeader {
    /// Returns true when this is the two-zero-word EOF sentinel.
    pub fn is_eof_sentinel(self) -> bool {
        self.literal_bytes == 0 && self.original_len == 0
    }

    pub fn read<R: Read>(r: &mut R, digest_len: usize) -> Result<(Self, Vec<u8>), SrepError> {
        let literal_bytes = r.read_u32::<LE>()?;
        let original_len = r.read_u32::<LE>()?;
        let stat_size = r.read_u32::<LE>()?;

        let mut digest = vec![0u8; digest_len];
        if digest_len > 0 {
            r.read_exact(&mut digest)?;
        }

        Ok((
            BlockHeader { literal_bytes, original_len, stat_size },
            digest,
        ))
    }

    pub fn write<W: Write>(self, w: &mut W, digest: &[u8]) -> Result<(), SrepError> {
        w.write_u32::<LE>(self.literal_bytes)?;
        w.write_u32::<LE>(self.original_len)?;
        w.write_u32::<LE>(self.stat_size)?;
        if !digest.is_empty() {
            w.write_all(digest)?;
        }
        Ok(())
    }

    /// Size of the on-disk block header in bytes.
    pub fn on_disk_size(digest_len: usize) -> usize {
        3 * size_of::<u32>() + digest_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn roundtrip_no_digest() {
        let hdr = BlockHeader { literal_bytes: 1234, original_len: 5678, stat_size: 0 };
        let mut buf = Vec::new();
        hdr.write(&mut buf, &[]).unwrap();
        assert_eq!(buf.len(), 12);

        let mut cur = Cursor::new(&buf);
        let (decoded, digest) = BlockHeader::read(&mut cur, 0).unwrap();
        assert_eq!(decoded, hdr);
        assert!(digest.is_empty());
    }

    #[test]
    fn roundtrip_with_digest() {
        let hdr = BlockHeader { literal_bytes: 100, original_len: 200, stat_size: 50 };
        let digest = [0xABu8; 20]; // SHA-1 length
        let mut buf = Vec::new();
        hdr.write(&mut buf, &digest).unwrap();
        assert_eq!(buf.len(), 12 + 20);

        let mut cur = Cursor::new(&buf);
        let (decoded, dec_digest) = BlockHeader::read(&mut cur, 20).unwrap();
        assert_eq!(decoded, hdr);
        assert_eq!(dec_digest.as_slice(), &digest);
    }
}
