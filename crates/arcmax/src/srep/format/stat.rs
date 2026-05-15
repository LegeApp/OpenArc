use crate::srep::error::SrepError;
use crate::srep::types::{Offset, Stat};

/// A decoded LZ match record (absolute positions in the uncompressed stream).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LzMatch {
    /// Absolute byte position of the match source in the original file.
    pub src: Offset,
    /// Absolute byte position of the match destination in the original file.
    pub dest: Offset,
    /// Match length in bytes.
    pub len: u32,
}

/// An intermediate match representation used during encoding — relative values
/// before absolute positions are resolved.
#[derive(Debug, Clone, Copy)]
pub struct EncodedMatch {
    /// Literal bytes preceding this match.
    pub literal_len: u32,
    /// Byte distance: dest - src (for IO_LZ mode).
    pub offset: u64,
    /// Total match length in bytes.
    pub len: u32,
}

/// Translates between `EncodedMatch` values and the on-disk STAT stream.
///
/// The STAT format (from ENCODE_LZ_MATCH / DECODE_LZ_MATCH macros in srep.cpp):
///
/// Round-matches mode (3 stats per match — `-m3` without dict):
///   [0] literal_len
///   [1] (dest_rounded / L - src / L)  — offset divided by L
///   [2] (len - L) / L                 — stored length divided by L
///
/// Non-round mode (4 stats per match):
///   [0] literal_len
///   [1] (dest - src) low 32 bits
///   [2] (dest - src) high 32 bits
///   [3] (len - L)                     — stored length (bytes)
#[derive(Debug, Clone, Copy)]
pub struct StatCodec {
    /// True when using `-m3` without dictionary (matches are L-aligned).
    pub round_matches: bool,
    /// `L` = chunk_len = BASE_LEN for the standard `-m3` case.
    pub chunk_len: u32,
}

impl StatCodec {
    pub fn stats_per_match(self) -> usize {
        if self.round_matches { 3 } else { 4 }
    }

    /// Encode one match into the stats buffer.
    pub fn encode(
        self,
        stats: &mut Vec<Stat>,
        lit_len: u32,
        offset: u64,
        len: u32,
    ) -> Result<(), SrepError> {
        let l = self.chunk_len as u64;
        let l1 = if self.round_matches { l } else { 1u64 };
        if self.round_matches && (offset % l1 != 0 || (len as u64) % l1 != 0) {
            return Err(SrepError::Format("rounded match not aligned to chunk_len"));
        }

        let stored_offset = offset
            .checked_div(l1)
            .ok_or(SrepError::Overflow("stat offset division"))?;

        let stored_len = (len as u64)
            .checked_sub(l)
            .ok_or(SrepError::Format("match shorter than chunk_len (BASE_LEN)"))?
            .checked_div(l1)
            .ok_or(SrepError::Overflow("stat length division"))?;

        stats.push(lit_len);
        stats.push(stored_offset as u32);
        if !self.round_matches {
            stats.push((stored_offset >> 32) as u32);
        }
        stats.push(stored_len as u32);
        Ok(())
    }

    /// Decode one match from the front of a stat slice, advancing the slice.
    /// Returns `None` when fewer stats remain than `stats_per_match()`.
    pub fn decode(self, stats: &mut &[Stat]) -> Option<EncodedMatch> {
        let needed = self.stats_per_match();
        if stats.len() < needed {
            return None;
        }

        let l = self.chunk_len as u64;
        let l1 = if self.round_matches { l } else { 1u64 };

        let lit_len = stats[0];
        let mut stored_offset = stats[1] as u64;
        let stored_len;

        if self.round_matches {
            stored_len = stats[2] as u64;
            *stats = &stats[3..];
        } else {
            stored_offset |= (stats[2] as u64) << 32;
            stored_len = stats[3] as u64;
            *stats = &stats[4..];
        }

        let offset = stored_offset * l1;
        let len = (stored_len * l1 + l) as u32;

        Some(EncodedMatch { literal_len: lit_len, offset, len })
    }

    /// Resolve an `EncodedMatch` to an absolute `LzMatch` given the current
    /// output position (= absolute byte offset in the original file up to and
    /// including literals already written).
    ///
    /// This mirrors the IO_LZ path of DECODE_LZ_MATCH:
    ///   dest = block_pos + lit_len
    ///   src  = (dest / l1 * l1) - offset
    pub fn resolve(self, em: EncodedMatch, block_pos: Offset) -> Result<LzMatch, SrepError> {
        let l1 = if self.round_matches { self.chunk_len as u64 } else { 1u64 };

        let dest = block_pos
            .checked_add(em.literal_len as u64)
            .ok_or(SrepError::Overflow("dest position"))?;

        // dest / l1 * l1 rounds dest DOWN to an l1-multiple
        let dest_rounded = (dest / l1) * l1;
        let src = dest_rounded
            .checked_sub(em.offset)
            .ok_or(SrepError::Format("match src underflows 0"))?;

        Ok(LzMatch { src, dest, len: em.len })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codec_round(round: bool, chunk_len: u32, lit: u32, offset: u64, len: u32) {
        let codec = StatCodec { round_matches: round, chunk_len };
        let mut stats: Vec<Stat> = Vec::new();
        codec.encode(&mut stats, lit, offset, len).expect("encode");

        assert_eq!(stats.len(), codec.stats_per_match());

        let slice: &[Stat] = &stats;
        let mut rem = slice;
        let decoded = codec.decode(&mut rem).expect("decode");

        assert!(rem.is_empty());
        assert_eq!(decoded.literal_len, lit);
        assert_eq!(decoded.offset, offset);
        assert_eq!(decoded.len, len);
    }

    #[test]
    fn roundtrip_nonround() {
        // 4-stat mode: arbitrary byte offsets
        codec_round(false, 32, 100, 12345678, 128);
        codec_round(false, 32, 0, 0xFFFF_FFFF_0000u64, 32);
        codec_round(false, 64, 7, 999, 64);
    }

    #[test]
    fn roundtrip_round() {
        // 3-stat mode: offset and (len - L) must be divisible by L
        let l: u32 = 32;
        codec_round(true, l, 50, 32 * 1000, 32 * 5); // offset = 32000, len = 160
        codec_round(true, l, 0, 0, 32);               // minimal match
    }

    #[test]
    fn len_below_chunk_len_is_error() {
        let codec = StatCodec { round_matches: false, chunk_len: 32 };
        let mut stats = Vec::new();
        assert!(codec.encode(&mut stats, 0, 100, 31).is_err());
    }

    #[test]
    fn empty_slice_returns_none() {
        let codec = StatCodec { round_matches: false, chunk_len: 32 };
        let slice: &[Stat] = &[];
        let mut rem = slice;
        assert!(codec.decode(&mut rem).is_none());
    }

    #[test]
    fn resolve_nonround() {
        let codec = StatCodec { round_matches: false, chunk_len: 32 };
        // block_pos = 1000, lit_len = 10 → dest = 1010, offset = 500 → src = 510
        let em = EncodedMatch { literal_len: 10, offset: 500, len: 64 };
        let lz = codec.resolve(em, 1000).unwrap();
        assert_eq!(lz.dest, 1010);
        assert_eq!(lz.src, 510);
        assert_eq!(lz.len, 64);
    }
}
