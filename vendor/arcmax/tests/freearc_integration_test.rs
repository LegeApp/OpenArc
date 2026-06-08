use std::str::FromStr;

use arcmax::codec::ppmd::{PpmdOptions, PpmdVariant};
use arcmax::method::{CodecPipeline, Method};
use arcmax::srep::{HashKind, OutputMode, SrepConfig};
use arcmax::{compress_with, decompress, CompressionOptions};

#[test]
fn method_parser_accepts_common_free_arc_names() {
    for method in [
        "storing",
        "tornado:5",
        "rep",
        "srep",
        "dict",
        "lzp:m32:h18",
        "grzip:0",
        "lzma",
        "lzma2",
        "lz4",
        "ppmd:o6:mem16m",
        "delta:4",
        "exe",
        "bcj_x86",
        "dispack",
        "4x4:b16m:t2:storing",
        "zstd:3",
    ] {
        Method::from_str(method).unwrap_or_else(|err| panic!("{method}: {err}"));
    }
}

#[test]
fn ppmd_parser_accepts_freearc_bare_order_and_memory() {
    let method = Method::from_str("ppmd:6:16m").unwrap();
    assert_eq!(
        method,
        Method::Ppmd(PpmdOptions {
            order: 6,
            memory_size: 16 * 1024 * 1024,
            variant: PpmdVariant::H,
        })
    );
}

#[test]
fn ppmd_parser_rejects_invalid_order() {
    assert!(Method::from_str("ppmd:o1:mem16m").is_err());
    assert!(Method::from_str("ppmd:o17:mem16m").is_err());
}

#[test]
fn store_pipeline_roundtrip() {
    let input = b"FreeArc store pipeline test payload";
    let mut pipeline = CodecPipeline::new(Method::Store);

    let mut compressed = Vec::new();
    pipeline.compress(&input[..], &mut compressed).unwrap();

    let mut decoded = Vec::new();
    pipeline.decompress(&compressed[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
}

#[test]
fn ppmd_pipeline_roundtrip() {
    let input = b"FreeArc native PPMd-H pipeline payload ".repeat(1024);
    let method = Method::from_str("ppmd:o6:mem16m").unwrap();
    let mut pipeline = CodecPipeline::new(method);

    let mut compressed = Vec::new();
    pipeline.compress(&input[..], &mut compressed).unwrap();

    let mut decoded = Vec::new();
    pipeline.decompress(&compressed[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
    assert!(
        compressed.len() < input.len(),
        "expected PPMd-H to compress repetitive text"
    );
}

#[test]
fn public_api_roundtrip_is_self_describing() {
    let input = b"Public API self describing frame payload ".repeat(256);
    let method = Method::from_str("ppmd:o6:mem16m").unwrap();

    let compressed =
        compress_with(&input, CompressionOptions::default().with_method(method)).unwrap();
    let decoded = decompress(&compressed).unwrap();

    assert_eq!(decoded, input);
}

#[test]
fn pipeline_v2_decompress_uses_embedded_method() {
    let input = b"Embedded method should override construction method ".repeat(256);
    let method = Method::from_str("ppmd:o6:mem16m").unwrap();
    let mut encoder = CodecPipeline::new(method);
    let mut compressed = Vec::new();
    encoder.compress(&input[..], &mut compressed).unwrap();

    let mut decoder = CodecPipeline::new(Method::Store);
    let mut decoded = Vec::new();
    decoder.decompress(&compressed[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
}

#[test]
fn srep_pipeline_roundtrip() {
    let input = b"FreeArc SREP pipeline payload ".repeat(512);
    let cfg = SrepConfig {
        output_mode: OutputMode::IoLz,
        hash: HashKind::None,
        block_size: 4096,
        ..SrepConfig::default()
    };

    let mut pipeline = CodecPipeline::new(Method::Srep(cfg));
    let mut compressed = Vec::new();
    pipeline.compress(&input[..], &mut compressed).unwrap();

    let mut decoded = Vec::new();
    pipeline.decompress(&compressed[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
}

#[test]
fn rep_pipeline_roundtrip() {
    let input = b"FreeArc REP pipeline payload ".repeat(512);
    let method = Method::from_str("rep:b64k:m32:s32").unwrap();
    let mut pipeline = CodecPipeline::new(method);

    let mut compressed = Vec::new();
    pipeline.compress(&input[..], &mut compressed).unwrap();

    let mut decoded = Vec::new();
    pipeline.decompress(&compressed[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
}

#[test]
fn rep_lz4_pipeline_roundtrip() {
    let input = b"FreeArc REP plus LZ4 pipeline payload ".repeat(512);
    let method = Method::from_str("rep:b64k:m32:s32+lz4").unwrap();
    let mut pipeline = CodecPipeline::new(method);

    let mut compressed = Vec::new();
    pipeline.compress(&input[..], &mut compressed).unwrap();

    let mut decoded = Vec::new();
    pipeline.decompress(&compressed[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
}

#[test]
fn dict_pipeline_roundtrip() {
    let input = b"FreeArc Dict pipeline repeated words repeated words ".repeat(512);
    let method = Method::from_str("dict:b64k").unwrap();
    let mut pipeline = CodecPipeline::new(method);

    let mut compressed = Vec::new();
    pipeline.compress(&input[..], &mut compressed).unwrap();

    let mut decoded = Vec::new();
    pipeline.decompress(&compressed[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
}

#[test]
fn dict_lz4_pipeline_roundtrip() {
    let input = b"FreeArc Dict plus LZ4 pipeline repeated words repeated words ".repeat(512);
    let method = Method::from_str("dict:b64k+lz4").unwrap();
    let mut pipeline = CodecPipeline::new(method);

    let mut compressed = Vec::new();
    pipeline.compress(&input[..], &mut compressed).unwrap();

    let mut decoded = Vec::new();
    pipeline.decompress(&compressed[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
}

#[test]
fn delta_store_pipeline_roundtrip() {
    let input = b"FreeArc delta pipeline payload".repeat(256);
    let method = Method::from_str("delta:2+storing").unwrap();
    let mut pipeline = CodecPipeline::new(method);

    let mut compressed = Vec::new();
    pipeline.compress(&input[..], &mut compressed).unwrap();

    let mut decoded = Vec::new();
    pipeline.decompress(&compressed[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
}

#[test]
fn bcj_delta_store_pipeline_roundtrip() {
    let input = [
        0x90, 0xE8, 0x40, 0x00, 0x00, 0x00, 0xE9, 0x10, 0x00, 0x00, 0x00,
    ]
    .repeat(256);
    let method = Method::from_str("exe+delta:1+storing").unwrap();
    let mut pipeline = CodecPipeline::new(method);

    let mut encoded = Vec::new();
    pipeline.compress(&input[..], &mut encoded).unwrap();

    let mut decoded = Vec::new();
    pipeline.decompress(&encoded[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
}

#[test]
fn delta_lz4_pipeline_roundtrip() {
    let input = b"FreeArc delta plus native LZ4 payload ".repeat(256);
    let method = Method::from_str("delta:1+lz4").unwrap();
    let mut pipeline = CodecPipeline::new(method);

    let mut encoded = Vec::new();
    pipeline.compress(&input[..], &mut encoded).unwrap();

    let mut decoded = Vec::new();
    pipeline.decompress(&encoded[..], &mut decoded).unwrap();

    assert_eq!(decoded, input);
}
