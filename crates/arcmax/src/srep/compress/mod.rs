pub mod block_sink;
pub mod fixed;
pub mod inmem;
pub mod literal;

use std::io::{Read, Seek, Write};

use crate::srep::config::{Method, SrepConfig};
use crate::srep::error::SrepError;

/// Statistics returned after a successful compression run.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompressionReport {
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub blocks_written: u64,
    pub matches_written: u64,
}

/// Compress `input` into `output` as an SREP archive using `config`.
pub fn compress<R, W>(
    input: R,
    output: W,
    config: SrepConfig,
) -> Result<CompressionReport, SrepError>
where
    R: Read + Seek,
    W: Write + Seek,
{
    match config.method {
        Method::M0 => inmem::encode(input, output, &config),
        Method::M3 | Method::M4 | Method::M5 => fixed::encode(input, output, &config),
        // M1/M2 fall back to literal-only until CDC matchers are implemented.
        Method::M1 | Method::M2 => literal::encode(input, output, &config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::srep::config::{HashKind, Method, OutputMode, SrepConfig};
    use crate::srep::decompress::decompress;
    use crate::srep::error::SrepError;
    use std::io::Cursor;

    fn cfg(mode: OutputMode) -> SrepConfig {
        SrepConfig {
            method: Method::M3,
            output_mode: mode,
            hash: HashKind::None,
            block_size: 4096,
            ..Default::default()
        }
    }

    fn roundtrip(data: &[u8], mode: OutputMode) -> Vec<u8> {
        let input = Cursor::new(data.to_vec());
        let mut compressed = Cursor::new(Vec::<u8>::new());
        compress(input, &mut compressed, cfg(mode)).expect("compress failed");
        compressed.set_position(0);
        let mut decoded = Cursor::new(Vec::<u8>::new());
        decompress(compressed, &mut decoded).expect("decompress failed");
        decoded.into_inner()
    }

    #[test]
    fn round_trip_io_lz_literal_only() {
        let payload: Vec<u8> = (0u32..100_000).map(|i| (i % 251) as u8).collect();
        let result = roundtrip(&payload, OutputMode::IoLz);
        assert_eq!(result, payload);
    }

    #[test]
    fn round_trip_index_lz_literal_only() {
        let payload: Vec<u8> = (0u32..50_000).map(|i| (i % 199) as u8).collect();
        let result = roundtrip(&payload, OutputMode::IndexLz);
        assert_eq!(result, payload);
    }

    #[test]
    fn round_trip_multi_block_io_lz() {
        // 3 full 4096-byte blocks + 1 partial block
        let payload: Vec<u8> = (0u32..13_000).map(|i| (i % 127) as u8).collect();
        let result = roundtrip(&payload, OutputMode::IoLz);
        assert_eq!(result, payload);
    }

    #[test]
    fn round_trip_multi_block_index_lz() {
        let payload: Vec<u8> = (0u32..13_000).map(|i| (i % 127) as u8).collect();
        let result = roundtrip(&payload, OutputMode::IndexLz);
        assert_eq!(result, payload);
    }

    #[test]
    fn round_trip_empty_input_io_lz() {
        let result = roundtrip(&[], OutputMode::IoLz);
        assert!(result.is_empty());
    }

    #[test]
    fn round_trip_empty_input_index_lz() {
        let result = roundtrip(&[], OutputMode::IndexLz);
        assert!(result.is_empty());
    }

    #[test]
    fn round_trip_exact_block_boundary() {
        // Exactly block_size bytes — one full block, no partial.
        let payload = vec![0xABu8; 4096];
        assert_eq!(roundtrip(&payload, OutputMode::IoLz), payload);
        assert_eq!(roundtrip(&payload, OutputMode::IndexLz), payload);
    }

    #[test]
    fn round_trip_block_size_plus_one() {
        // block_size + 1 bytes — one full block and one 1-byte block.
        let payload = vec![0xCDu8; 4097];
        assert_eq!(roundtrip(&payload, OutputMode::IoLz), payload);
        assert_eq!(roundtrip(&payload, OutputMode::IndexLz), payload);
    }

    #[test]
    fn compress_reports_correct_byte_counts() {
        let payload = vec![0u8; 8192];
        let mut compressed = Cursor::new(Vec::<u8>::new());
        let rep = compress(Cursor::new(payload.clone()), &mut compressed, cfg(OutputMode::IoLz)).unwrap();
        assert_eq!(rep.bytes_in, 8192);
        assert_eq!(rep.blocks_written, 2); // 4096 + 4096
        assert_eq!(rep.bytes_out, compressed.get_ref().len() as u64);
    }

    #[test]
    fn rejects_future_lz() {
        let cfg = SrepConfig {
            hash: HashKind::None,
            output_mode: OutputMode::FutureLz,
            ..Default::default()
        };
        let mut out = Cursor::new(Vec::<u8>::new());
        let err = compress(Cursor::new(b"data".to_vec()), &mut out, cfg).unwrap_err();
        assert!(matches!(err, SrepError::Unsupported(_)));
    }

    #[test]
    fn round_trip_io_lz_siphash() {
        let payload: Vec<u8> = (0u32..50_000).map(|i| (i % 173) as u8).collect();
        let cfg = SrepConfig {
            method: Method::M3,
            output_mode: OutputMode::IoLz,
            hash: HashKind::SipHash,
            block_size: 4096,
            ..Default::default()
        };
        let mut compressed = Cursor::new(Vec::<u8>::new());
        compress(Cursor::new(payload.clone()), &mut compressed, cfg).unwrap();
        compressed.set_position(0);
        let mut decoded = Cursor::new(Vec::<u8>::new());
        decompress(compressed, &mut decoded).unwrap();
        assert_eq!(decoded.into_inner(), payload);
    }

    #[test]
    fn round_trip_index_lz_siphash() {
        let payload: Vec<u8> = (0u32..50_000).map(|i| (i % 251) as u8).collect();
        let cfg = SrepConfig {
            method: Method::M3,
            output_mode: OutputMode::IndexLz,
            hash: HashKind::SipHash,
            block_size: 8192,
            ..Default::default()
        };
        let mut compressed = Cursor::new(Vec::<u8>::new());
        compress(Cursor::new(payload.clone()), &mut compressed, cfg).unwrap();
        compressed.set_position(0);
        let mut decoded = Cursor::new(Vec::<u8>::new());
        decompress(compressed, &mut decoded).unwrap();
        assert_eq!(decoded.into_inner(), payload);
    }

    #[test]
    fn round_trip_m0_inmem_finds_matches() {
        let mut payload = Vec::new();
        let pattern: Vec<u8> = (0u8..=127).collect();
        for _ in 0..128 {
            payload.extend_from_slice(&pattern);
        }

        let cfg = SrepConfig {
            method: Method::M0,
            output_mode: OutputMode::IoLz,
            hash: HashKind::None,
            chunk_len: 16,
            min_match: 16,
            block_size: 1024,
            dict_size: 4096,
            ..Default::default()
        };

        let mut compressed = Cursor::new(Vec::<u8>::new());
        let report = compress(Cursor::new(payload.clone()), &mut compressed, cfg).unwrap();
        assert!(report.matches_written > 0);

        compressed.set_position(0);
        let mut decoded = Cursor::new(Vec::<u8>::new());
        decompress(compressed, &mut decoded).unwrap();
        assert_eq!(decoded.into_inner(), payload);
    }

    #[test]
    fn round_trip_m0_index_lz_with_wrapping_dictionary() {
        let mut payload = Vec::new();
        let pattern: Vec<u8> = (0u8..=63).collect();
        for _ in 0..64 {
            payload.extend_from_slice(&pattern);
        }

        let cfg = SrepConfig {
            method: Method::M0,
            output_mode: OutputMode::IndexLz,
            hash: HashKind::None,
            chunk_len: 16,
            min_match: 16,
            block_size: 128,
            dict_size: 256,
            ..Default::default()
        };

        let mut compressed = Cursor::new(Vec::<u8>::new());
        let report = compress(Cursor::new(payload.clone()), &mut compressed, cfg).unwrap();
        assert!(report.matches_written > 0);

        compressed.set_position(0);
        let mut decoded = Cursor::new(Vec::<u8>::new());
        decompress(compressed, &mut decoded).unwrap();
        assert_eq!(decoded.into_inner(), payload);
    }

    #[test]
    fn round_trip_m4_fixed_reread_finds_matches() {
        let mut payload = Vec::new();
        let pattern: Vec<u8> = (0u8..=255).collect();
        for _ in 0..64 {
            payload.extend_from_slice(&pattern);
        }

        let cfg = SrepConfig {
            method: Method::M4,
            output_mode: OutputMode::IoLz,
            hash: HashKind::None,
            chunk_len: 32,
            min_match: 32,
            block_size: 1024,
            ..Default::default()
        };

        let mut compressed = Cursor::new(Vec::<u8>::new());
        let report = compress(Cursor::new(payload.clone()), &mut compressed, cfg).unwrap();
        assert!(report.matches_written > 0);

        compressed.set_position(0);
        let mut decoded = Cursor::new(Vec::<u8>::new());
        decompress(compressed, &mut decoded).unwrap();
        assert_eq!(decoded.into_inner(), payload);
    }

    #[test]
    fn round_trip_m3_extends_cross_block_by_digest_chunks() {
        let pattern: Vec<u8> = (0u8..=255).collect();
        let mut payload = Vec::new();
        payload.extend_from_slice(&pattern);
        payload.extend_from_slice(&pattern);

        let cfg = SrepConfig {
            method: Method::M3,
            output_mode: OutputMode::IoLz,
            hash: HashKind::SipHash,
            chunk_len: 32,
            min_match: 32,
            block_size: 256,
            ..Default::default()
        };

        let mut compressed = Cursor::new(Vec::<u8>::new());
        let report = compress(Cursor::new(payload.clone()), &mut compressed, cfg).unwrap();
        assert!(report.matches_written > 0);
        assert!(
            report.matches_written < 8,
            "digest extension should coalesce some repeated chunks"
        );

        compressed.set_position(0);
        let mut decoded = Cursor::new(Vec::<u8>::new());
        decompress(compressed, &mut decoded).unwrap();
        assert_eq!(decoded.into_inner(), payload);
    }

    #[test]
    fn round_trip_io_lz_sha512() {
        let payload: Vec<u8> = (0u32..8192).map(|i| (i % 97) as u8).collect();
        let cfg = SrepConfig {
            method: Method::M3,
            output_mode: OutputMode::IoLz,
            hash: HashKind::Sha512,
            block_size: 4096,
            ..Default::default()
        };
        let mut compressed = Cursor::new(Vec::<u8>::new());
        compress(Cursor::new(payload.clone()), &mut compressed, cfg).unwrap();
        compressed.set_position(0);
        let mut decoded = Cursor::new(Vec::<u8>::new());
        decompress(compressed, &mut decoded).unwrap();
        assert_eq!(decoded.into_inner(), payload);
    }

    #[test]
    fn rejects_unimplemented_hash_vmac() {
        let cfg = SrepConfig {
            hash: HashKind::Vmac,
            ..Default::default()
        };
        let mut out = Cursor::new(Vec::<u8>::new());
        let err = compress(Cursor::new(b"hello world".to_vec()), &mut out, cfg).unwrap_err();
        assert!(matches!(err, SrepError::Unsupported(_)));
    }
}
