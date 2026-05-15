//! Shared block-level decode logic for IO_LZ and INDEX_LZ modes.
//!
//! Given a block's stat stream, literal payload, and a back-reference source
//! for bytes that fall in previously-decoded blocks, reconstruct the original
//! block bytes. This mirrors the `decompress()` function in srep.cpp lines
//! 1290–1336.

use std::io::{Read, Seek, SeekFrom};

use crate::srep::error::SrepError;
use crate::srep::format::stat::StatCodec;
use crate::srep::types::{Offset, Stat};

/// Decode one block's worth of literals + matches into `out`.
///
/// # Arguments
/// * `block_start` — Absolute byte offset of this block in the uncompressed stream.
/// * `out`         — Output buffer sized to the block's `original_len`.
/// * `literals`    — Literal bytes for this block (`literal_bytes` bytes total).
/// * `stats`       — Decoded match stream for this block (may be empty).
/// * `codec`       — How to interpret `stats` (round-matches mode + chunk_len).
/// * `back_src`    — Random-access source for matches whose `src < block_start`.
pub fn decode_block<R: Read + Seek>(
    block_start: Offset,
    out: &mut [u8],
    literals: &[u8],
    stats: &[Stat],
    codec: StatCodec,
    back_src: &mut R,
) -> Result<(), SrepError> {
    let mut stat_slice: &[Stat] = stats;
    let mut out_pos: usize = 0;
    let mut lit_pos: usize = 0;

    while stat_slice.len() >= codec.stats_per_match() {
        let em = codec.decode(&mut stat_slice).ok_or(SrepError::Format(
            "stat stream truncated mid-match",
        ))?;

        let block_pos = block_start
            .checked_add(out_pos as u64)
            .ok_or(SrepError::Overflow("block position"))?;
        let lz = codec.resolve(em, block_pos)?;

        let lit_len = em.literal_len as usize;
        let match_len = lz.len as usize;

        // Bounds checks (mirrors C++ `lit_len > inend - in || ... || lz_match.src >= lz_match.dest`)
        if lit_len > literals.len().saturating_sub(lit_pos) {
            return Err(SrepError::Format("literal bytes overflow input buffer"));
        }
        if lit_len + match_len > out.len().saturating_sub(out_pos) {
            return Err(SrepError::Format("match overflows output buffer"));
        }
        if lz.src >= lz.dest {
            return Err(SrepError::Format("match src >= dest"));
        }

        // 1) Copy literal bytes.
        out[out_pos..out_pos + lit_len]
            .copy_from_slice(&literals[lit_pos..lit_pos + lit_len]);
        lit_pos += lit_len;
        out_pos += lit_len;

        let mut src = lz.src;
        let mut remaining = match_len;

        // 2) Copy any portion of the match that lives in previous blocks (back_src).
        if src < block_start {
            let avail = (block_start - src) as usize;
            let bytes = avail.min(remaining);
            back_src.seek(SeekFrom::Start(src))?;
            back_src.read_exact(&mut out[out_pos..out_pos + bytes])?;
            out_pos += bytes;
            src += bytes as u64;
            remaining -= bytes;
        }

        // 3) Copy in-block match data with LZ77 overlap semantics (forward
        //    byte-by-byte: each destination byte may have just been written).
        if remaining > 0 {
            let src_idx = (src - block_start) as usize;
            if src_idx + remaining > out.len() {
                return Err(SrepError::Format("in-block match exceeds buffer"));
            }
            for i in 0..remaining {
                out[out_pos + i] = out[src_idx + i];
            }
            out_pos += remaining;
        }
    }

    // 4) Tail literals: must exactly fill the rest of the output buffer.
    let tail_in = literals.len() - lit_pos;
    let tail_out = out.len() - out_pos;
    if tail_in != tail_out {
        return Err(SrepError::Format("block size mismatch at tail"));
    }
    out[out_pos..].copy_from_slice(&literals[lit_pos..]);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn decode_block_no_matches() {
        // Single block of pure literals, no stats.
        let codec = StatCodec { round_matches: false, chunk_len: 32 };
        let literals = b"hello world".to_vec();
        let mut out = vec![0u8; literals.len()];
        let mut back = Cursor::new(Vec::<u8>::new());
        decode_block(0, &mut out, &literals, &[], codec, &mut back).unwrap();
        assert_eq!(out, b"hello world");
    }

    #[test]
    fn decode_block_self_overlap_rle() {
        // A match where dest = src + 1, len = 5: classic LZ77 RLE.
        // Block starts with 'A' literal, then a match (src=0, dest=1, len=4) → "AAAAA"
        let codec = StatCodec { round_matches: false, chunk_len: 32 };
        let mut stats = Vec::new();
        // lit_len=1 ('A'), offset = dest - src = 1, match_len = 36 (>= chunk_len)
        // For a 36-byte match starting at dest=1, src=0, we need 37 bytes of output.
        codec.encode(&mut stats, 1, 1, 36).unwrap();
        let literals = b"A".to_vec();
        let mut out = vec![0u8; 37];
        let mut back = Cursor::new(Vec::<u8>::new());
        decode_block(0, &mut out, &literals, &stats, codec, &mut back).unwrap();
        assert_eq!(out, vec![b'A'; 37]);
    }

    #[test]
    fn decode_block_back_reference() {
        // Block 1 references bytes from block 0 (which is in `back_src`).
        let codec = StatCodec { round_matches: false, chunk_len: 32 };
        let prev_block = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcd".to_vec(); // 40 bytes
        // This block starts at offset 40. Match: lit_len=0, offset=40 (dest=40, src=0), len=32.
        let mut stats = Vec::new();
        codec.encode(&mut stats, 0, 40, 32).unwrap();
        let literals: Vec<u8> = Vec::new();
        let mut out = vec![0u8; 32];
        let mut back = Cursor::new(prev_block.clone());
        decode_block(40, &mut out, &literals, &stats, codec, &mut back).unwrap();
        assert_eq!(out, prev_block[..32]);
    }

    #[test]
    fn decode_block_rejects_src_ge_dest() {
        let codec = StatCodec { round_matches: false, chunk_len: 32 };
        let mut stats = Vec::new();
        // offset = 0 → src = dest → invalid
        codec.encode(&mut stats, 0, 0, 32).unwrap();
        let mut out = vec![0u8; 32];
        let mut back = Cursor::new(Vec::<u8>::new());
        let err = decode_block(0, &mut out, &[], &stats, codec, &mut back).unwrap_err();
        assert!(matches!(err, SrepError::Format(_)));
    }
}
