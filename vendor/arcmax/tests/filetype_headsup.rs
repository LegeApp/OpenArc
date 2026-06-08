//! Heads-up tests for file-type detection and pipeline suggestion.
//!
//! These tests serve two purposes:
//!
//! 1. **Correctness guards** — verify that detection returns the right
//!    [`FileTypeHint`] for known extensions and magic-byte signatures, and
//!    that [`suggest_pipeline`] produces a method with the expected structure.
//!
//! 2. **Quality gates** — verify that the suggested pipeline actually improves
//!    compression ratio versus a plain baseline on representative synthetic
//!    fixtures. If a future change breaks a filter or swaps to a worse codec
//!    for a file type, these tests will fail loudly.
//!
//! Run with: `cargo test --test filetype_headsup`

use arcmax::codec::lz4::Lz4Options;
use arcmax::filetype::{
    detect, detect_by_extension, detect_by_magic, sniff_tar_inner, suggest_pipeline,
    CompressionTarget, FileTypeHint, SuggestionConfig,
};
use arcmax::method::pipeline::CodecPipeline;
use arcmax::method::Method;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Compress `data` with the given pipeline and return the compressed size.
fn compressed_size(method: Method, data: &[u8]) -> usize {
    let mut out = Vec::new();
    CodecPipeline::new(method)
        .compress(std::io::Cursor::new(data), &mut out)
        .expect("compression failed");
    out.len()
}

/// Build a `SuggestionConfig` whose default codec is LZ4 (fast, so tests
/// complete quickly) rather than LZMA.
fn fast_cfg() -> SuggestionConfig {
    SuggestionConfig::default().with_default_codec(Method::Lz4(Lz4Options::default()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Synthetic fixtures
// ─────────────────────────────────────────────────────────────────────────────

/// 128 KiB of x86-like code: lots of CALL (0xE8) instructions followed by
/// plausible 32-bit signed offsets, padded with NOP sleds.
fn make_exe_fixture() -> Vec<u8> {
    let mut buf = Vec::with_capacity(128 * 1024);
    let mut rng: u32 = 0xDEAD_BEEF;
    while buf.len() < 128 * 1024 {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        // Emit a NOP sled then a CALL near with a forward-biased offset.
        for _ in 0..((rng >> 24) & 0xF) {
            buf.push(0x90); // NOP
        }
        buf.push(0xE8); // CALL near
                        // Offset biased toward small positive values (realistic for executable).
        let offset = ((rng as i32) >> 16) & 0x0000_FFFF;
        buf.extend_from_slice(&offset.to_le_bytes());
        // Occasional data bytes (push, mov, etc.)
        if rng & 0x3 == 0 {
            buf.push(0x55); // PUSH RBP
            buf.push(0x48);
            buf.push(0x89);
            buf.push(0xE5); // MOV RBP, RSP
        }
    }
    buf.truncate(128 * 1024);
    buf
}

/// 128 KiB of English-like text: pangrams + repeated words, high character
/// entropy, but strong high-order context — PPMd/BSC territory.
fn make_text_fixture() -> Vec<u8> {
    const CORPUS: &[&str] = &[
        "The quick brown fox jumps over the lazy dog. ",
        "Pack my box with five dozen liquor jugs. ",
        "How vexingly quick daft zebras jump. ",
        "fn compress(input: &[u8]) -> Result<Vec<u8>> { todo!() }\n",
        "pub struct Codec { options: CodecOptions, buf: Vec<u8> }\n",
        "// This is a comment explaining the algorithm below.\n",
        "use std::io::{Read, Write};\nuse anyhow::Result;\n",
        "SELECT id, name, value FROM table WHERE id = 42;\n",
        "error[E0502]: cannot borrow `list` as mutable because it is also borrowed\n",
    ];
    let mut buf = Vec::with_capacity(128 * 1024);
    let mut i = 0usize;
    while buf.len() < 128 * 1024 {
        buf.extend_from_slice(CORPUS[i % CORPUS.len()].as_bytes());
        i += 1;
    }
    buf.truncate(128 * 1024);
    buf
}

/// 128 KiB of synthetic 16-bit Bayer sensor data: slowly varying smooth
/// gradients per 2×2 RGGB quad, simulating a camera RAW frame.
fn make_raw_image_fixture() -> Vec<u8> {
    let n_pixels = 64 * 1024; // 64 K pixels × 2 bytes = 128 KiB
    let mut pixels: Vec<u16> = Vec::with_capacity(n_pixels);
    for i in 0..n_pixels {
        let x = (i % 256) as u16;
        let y = (i / 256) as u16;
        // RGGB pattern with slow spatial variation
        let base: u16 = 512 + (x / 4) + (y / 4);
        let channel_offset: u16 = match i % 4 {
            0 => 0,      // R
            1 | 2 => 12, // G (slightly brighter)
            _ => 8,      // B
        };
        pixels.push(base.wrapping_add(channel_offset));
    }
    pixels.iter().flat_map(|p| p.to_le_bytes()).collect()
}

/// 128 KiB of 16-bit PCM audio: Brownian (random-walk) noise, which models
/// real audio at the sample level — highly correlated between adjacent samples
/// but with wide-range byte values that defeat LZ4 without a delta filter.
///
/// A pure sine wave is a poor fixture here because LZ4 already exploits the
/// strict periodicity without any filter. Brownian noise is more representative
/// of real audio content and reliably demonstrates the delta-filter benefit.
fn make_pcm_fixture() -> Vec<u8> {
    let n_samples = 64 * 1024;
    let mut buf = Vec::with_capacity(n_samples * 2);
    let mut state: u32 = 0xDEAD_BEEF;
    let mut sample: i16 = 0;
    for _ in 0..n_samples {
        // xorshift32 for a small pseudo-random step
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        // Step is ±1..±8 (correlated noise, not pure white noise)
        let step = ((state & 0xF) as i16) - 7; // -7..+8
        sample = sample.saturating_add(step);
        buf.extend_from_slice(&sample.to_le_bytes());
    }
    buf
}

/// 128 KiB of pseudo-random bytes (simulating already-compressed data).
fn make_random_fixture() -> Vec<u8> {
    let mut buf = Vec::with_capacity(128 * 1024);
    let mut state: u64 = 0x123456789ABCDEF0;
    while buf.len() < 128 * 1024 {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        buf.extend_from_slice(&state.to_le_bytes());
    }
    buf.truncate(128 * 1024);
    buf
}

// ─────────────────────────────────────────────────────────────────────────────
// Detection — extension
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn detect_ext_exe_is_executable() {
    assert_eq!(detect_by_extension("program.exe"), FileTypeHint::Executable);
}

#[test]
fn detect_ext_dll_is_executable() {
    assert_eq!(detect_by_extension("library.dll"), FileTypeHint::Executable);
}

#[test]
fn detect_ext_so_is_executable() {
    assert_eq!(detect_by_extension("libfoo.so"), FileTypeHint::Executable);
}

#[test]
fn detect_ext_case_insensitive() {
    assert_eq!(detect_by_extension("SETUP.EXE"), FileTypeHint::Executable);
    assert_eq!(detect_by_extension("README.TXT"), FileTypeHint::Text);
}

#[test]
fn detect_ext_rust_source_is_text() {
    assert_eq!(detect_by_extension("main.rs"), FileTypeHint::Text);
}

#[test]
fn detect_ext_json_is_text() {
    assert_eq!(detect_by_extension("config.json"), FileTypeHint::Text);
}

#[test]
fn detect_ext_dng_is_raw_image() {
    assert_eq!(detect_by_extension("photo.dng"), FileTypeHint::RawImage);
}

#[test]
fn detect_ext_cr2_is_raw_image() {
    assert_eq!(detect_by_extension("shot.cr2"), FileTypeHint::RawImage);
}

#[test]
fn detect_ext_nef_is_raw_image() {
    assert_eq!(detect_by_extension("nikon.nef"), FileTypeHint::RawImage);
}

#[test]
fn detect_ext_jpg_is_compressed_image() {
    assert_eq!(
        detect_by_extension("photo.jpg"),
        FileTypeHint::CompressedImage
    );
}

#[test]
fn detect_ext_png_is_compressed_image() {
    assert_eq!(
        detect_by_extension("sprite.png"),
        FileTypeHint::CompressedImage
    );
}

#[test]
fn detect_ext_wav_is_audio_pcm() {
    assert_eq!(detect_by_extension("sound.wav"), FileTypeHint::AudioPcm);
}

#[test]
fn detect_ext_mp3_is_compressed_media() {
    assert_eq!(
        detect_by_extension("music.mp3"),
        FileTypeHint::CompressedMedia
    );
}

#[test]
fn detect_ext_mkv_is_compressed_media() {
    assert_eq!(
        detect_by_extension("film.mkv"),
        FileTypeHint::CompressedMedia
    );
}

#[test]
fn detect_ext_zip_is_archive() {
    assert_eq!(detect_by_extension("backup.zip"), FileTypeHint::Archive);
}

#[test]
fn detect_ext_7z_is_archive() {
    assert_eq!(detect_by_extension("data.7z"), FileTypeHint::Archive);
}

#[test]
fn detect_ext_zst_is_archive() {
    assert_eq!(detect_by_extension("snapshot.zst"), FileTypeHint::Archive);
}

#[test]
fn detect_ext_sqlite_is_database() {
    assert_eq!(detect_by_extension("app.sqlite"), FileTypeHint::Database);
}

#[test]
fn detect_ext_unknown_returns_unknown() {
    assert_eq!(detect_by_extension("file.xyz123"), FileTypeHint::Unknown);
    assert_eq!(detect_by_extension("no_extension"), FileTypeHint::Unknown);
}

// ─────────────────────────────────────────────────────────────────────────────
// Detection — magic bytes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn detect_magic_mz_is_executable() {
    let header = b"MZ\x90\x00\x03\x00\x00\x00";
    assert_eq!(detect_by_magic(header), FileTypeHint::Executable);
}

#[test]
fn detect_magic_elf_is_executable() {
    let header = b"\x7fELF\x02\x01\x01\x00";
    assert_eq!(detect_by_magic(header), FileTypeHint::Executable);
}

#[test]
fn detect_magic_jpeg_is_compressed_image() {
    let header = b"\xFF\xD8\xFF\xE0\x00\x10JFIF";
    assert_eq!(detect_by_magic(header), FileTypeHint::CompressedImage);
}

#[test]
fn detect_magic_png_is_compressed_image() {
    let header = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
    assert_eq!(detect_by_magic(header), FileTypeHint::CompressedImage);
}

#[test]
fn detect_magic_wave_is_audio_pcm() {
    // RIFF header (4) + size (4) + WAVE subtype (4) = 12 bytes minimum for RIFF sub-type detection.
    let header = b"RIFF\xdc\x3d\x00\x00WAVEfmt ";
    assert_eq!(detect_by_magic(header), FileTypeHint::AudioPcm);
}

#[test]
fn detect_magic_zip_is_archive() {
    let header = b"PK\x03\x04\x14\x00\x00\x00";
    assert_eq!(detect_by_magic(header), FileTypeHint::Archive);
}

#[test]
fn detect_magic_gzip_is_archive() {
    let header = b"\x1f\x8b\x08\x00\x00\x00\x00\x00";
    assert_eq!(detect_by_magic(header), FileTypeHint::Archive);
}

#[test]
fn detect_magic_sqlite_is_database() {
    let header = b"SQLite format 3\x00";
    assert_eq!(detect_by_magic(header), FileTypeHint::Database);
}

#[test]
fn detect_magic_short_header_is_unknown() {
    assert_eq!(detect_by_magic(b"\x4D\x5A"), FileTypeHint::Unknown); // only 2 bytes
}

// ─────────────────────────────────────────────────────────────────────────────
// Combined detection (extension + magic)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn detect_ext_wins_over_magic() {
    // A DNG file might start with TIFF magic (0x49 0x49 0x2A 0x00), but the
    // extension correctly identifies it as RawImage.
    let tiff_magic = b"\x49\x49\x2A\x00\x08\x00\x00\x00";
    assert_eq!(detect("capture.dng", tiff_magic), FileTypeHint::RawImage);
}

#[test]
fn detect_falls_back_to_magic_for_unknown_ext() {
    let pe_magic = b"MZ\x90\x00\x03\x00\x00\x00";
    assert_eq!(
        detect("suspicious_file.bin", pe_magic),
        FileTypeHint::Executable
    );
}

#[test]
fn detect_unknown_ext_and_no_magic_match() {
    assert_eq!(
        detect("data.xyz", b"\xAA\xBB\xCC\xDD"),
        FileTypeHint::Unknown
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Suggestion structure
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn suggest_executable_starts_with_dispack() {
    let m = suggest_pipeline(FileTypeHint::Executable, &fast_cfg());
    match m {
        Method::Pipeline(stages) => {
            assert!(
                matches!(stages.first(), Some(Method::Dispack(_))),
                "expected Dispack as first stage, got {:?}",
                stages.first()
            );
            assert_eq!(stages.len(), 2);
        }
        other => panic!("expected Pipeline, got {other:?}"),
    }
}

#[test]
fn suggest_raw_image_starts_with_rawbayer() {
    let m = suggest_pipeline(FileTypeHint::RawImage, &fast_cfg());
    match m {
        Method::Pipeline(stages) => {
            assert!(
                matches!(stages.first(), Some(Method::RawBayer(_))),
                "expected RawBayer as first stage"
            );
        }
        other => panic!("expected Pipeline, got {other:?}"),
    }
}

#[test]
fn suggest_pcm_starts_with_delta2() {
    let m = suggest_pipeline(FileTypeHint::AudioPcm, &fast_cfg());
    match m {
        Method::Pipeline(stages) => match stages.first() {
            Some(Method::Delta(opts)) => assert_eq!(opts.stride, 2),
            other => panic!("expected Delta(2) as first stage, got {other:?}"),
        },
        other => panic!("expected Pipeline, got {other:?}"),
    }
}

#[test]
fn suggest_archive_is_store() {
    assert!(matches!(
        suggest_pipeline(FileTypeHint::Archive, &fast_cfg()),
        Method::Store
    ));
}

#[test]
fn suggest_compressed_image_is_store() {
    assert!(matches!(
        suggest_pipeline(FileTypeHint::CompressedImage, &fast_cfg()),
        Method::Store
    ));
}

#[test]
fn suggest_compressed_media_is_store() {
    assert!(matches!(
        suggest_pipeline(FileTypeHint::CompressedMedia, &fast_cfg()),
        Method::Store
    ));
}

#[test]
fn suggest_text_with_default_cfg_uses_ppmd() {
    let m = suggest_pipeline(FileTypeHint::Text, &SuggestionConfig::default());
    assert!(matches!(m, Method::Ppmd(_)), "expected PPMd, got {m:?}");
}

#[test]
fn suggest_text_with_ppmd_explicit() {
    let m = suggest_pipeline(FileTypeHint::Text, &SuggestionConfig::with_ppmd_text());
    assert!(matches!(m, Method::Ppmd(_)));
}

#[test]
fn suggest_database_inherits_default_codec() {
    let cfg = fast_cfg();
    let m = suggest_pipeline(FileTypeHint::Database, &cfg);
    assert!(
        matches!(m, Method::Lz4(_)),
        "expected Lz4 (the fast_cfg default)"
    );
}

#[test]
fn suggest_unknown_inherits_default_codec() {
    let cfg = fast_cfg();
    let m = suggest_pipeline(FileTypeHint::Unknown, &cfg);
    assert!(matches!(m, Method::Lz4(_)));
}

// ─────────────────────────────────────────────────────────────────────────────
// Roundtrip correctness
// ─────────────────────────────────────────────────────────────────────────────

fn roundtrip(method: Method, data: &[u8]) {
    let mut compressed = Vec::new();
    let mut pipeline = CodecPipeline::new(method);
    pipeline
        .compress(std::io::Cursor::new(data), &mut compressed)
        .expect("compress failed");

    let mut decoded = Vec::new();
    pipeline
        .decompress(std::io::Cursor::new(&compressed), &mut decoded)
        .expect("decompress failed");

    assert_eq!(
        decoded,
        data,
        "roundtrip mismatch: {} bytes in, {} bytes out",
        data.len(),
        decoded.len()
    );
}

#[test]
fn roundtrip_exe_pipeline() {
    roundtrip(
        suggest_pipeline(FileTypeHint::Executable, &fast_cfg()),
        &make_exe_fixture(),
    );
}

#[test]
fn roundtrip_raw_image_pipeline() {
    roundtrip(
        suggest_pipeline(FileTypeHint::RawImage, &fast_cfg()),
        &make_raw_image_fixture(),
    );
}

#[test]
fn roundtrip_pcm_pipeline() {
    roundtrip(
        suggest_pipeline(FileTypeHint::AudioPcm, &fast_cfg()),
        &make_pcm_fixture(),
    );
}

#[test]
fn roundtrip_text_ppmd() {
    // Uses LZMA default_codec for PPMd path (text_codec = Ppmd, not Pipeline).
    roundtrip(
        suggest_pipeline(FileTypeHint::Text, &SuggestionConfig::with_ppmd_text()),
        &make_text_fixture(),
    );
}

#[test]
fn roundtrip_archive_is_store_identity() {
    // Store pipeline is a lossless identity transform (compress → decompress = input).
    let data = make_random_fixture();
    let mut compressed = Vec::new();
    let mut pipeline = CodecPipeline::new(Method::Store);
    pipeline
        .compress(std::io::Cursor::new(&data), &mut compressed)
        .unwrap();

    let mut decoded = Vec::new();
    pipeline
        .decompress(std::io::Cursor::new(&compressed), &mut decoded)
        .unwrap();
    assert_eq!(decoded, data);
}

// ─────────────────────────────────────────────────────────────────────────────
// Quality gates — the filter must demonstrably reduce entropy
// ─────────────────────────────────────────────────────────────────────────────
//
// Strategy: compare <filter> + store vs plain store.  The filter alone either
// increases or decreases byte entropy; if it's beneficial, LZ4 on the filtered
// data will be smaller than LZ4 on the raw data.  Store is used as the inner
// codec so the test measures *only* the filter effect, not codec interactions.

fn lz4_size(data: &[u8]) -> usize {
    compressed_size(Method::Lz4(Lz4Options::default()), data)
}

#[test]
fn rawbayer_filter_reduces_entropy_on_raw_pixels() {
    use arcmax::codec::filters::rawbayer::RawBayerFilter;
    use arcmax::codec::filters::{Filter, RawBayerOptions};

    let raw = make_raw_image_fixture();
    let mut filtered = Vec::new();
    RawBayerFilter::new(RawBayerOptions)
        .encode(&raw, &mut filtered)
        .unwrap();

    let raw_lz4 = lz4_size(&raw);
    let flt_lz4 = lz4_size(&filtered);

    assert!(
        flt_lz4 < raw_lz4,
        "rawbayer filter should reduce entropy: filtered {flt_lz4} bytes >= raw {raw_lz4} bytes"
    );
}

#[test]
fn delta_filter_reduces_entropy_on_pcm() {
    use arcmax::codec::filters::{DeltaFilter, DeltaOptions, Filter};

    let pcm = make_pcm_fixture();
    let mut filtered = Vec::new();
    DeltaFilter::new(DeltaOptions { stride: 2 })
        .unwrap()
        .encode(&pcm, &mut filtered)
        .unwrap();

    let raw_lz4 = lz4_size(&pcm);
    let flt_lz4 = lz4_size(&filtered);

    assert!(
        flt_lz4 < raw_lz4,
        "delta:2 filter should reduce entropy: filtered {flt_lz4} >= raw {raw_lz4}"
    );
}

#[test]
fn store_is_optimal_for_random_data() {
    // Verify that lz4 does not improve on Store for near-random data.
    // (lz4 may produce slightly larger output due to the framing header.)
    let random = make_random_fixture();
    let lz4_sz = lz4_size(&random);
    let store_sz = random.len(); // Store is identity

    // Allow up to 2 % overhead — the key property is that we don't get
    // a meaningful win from LZ on truly random data, so Store is sensible.
    let margin = store_sz / 50 + 256;
    assert!(
        lz4_sz >= store_sz.saturating_sub(margin),
        "lz4 on random data should not beat store by more than 2%: lz4={lz4_sz} store={store_sz}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// BSC — feature-gated tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "bsc")]
mod bsc_tests {
    use super::*;
    use arcmax::codec::BscOptions;
    use arcmax::filetype::TextCodec;

    #[test]
    fn suggest_text_with_bsc_returns_bsc_method() {
        let cfg = SuggestionConfig::with_bsc_text();
        let m = suggest_pipeline(FileTypeHint::Text, &cfg);
        assert!(matches!(m, Method::Bsc(_)), "expected Bsc, got {m:?}");
    }

    #[test]
    fn roundtrip_text_bsc() {
        let cfg = SuggestionConfig::with_bsc_text();
        roundtrip(
            suggest_pipeline(FileTypeHint::Text, &cfg),
            &make_text_fixture(),
        );
    }

    #[test]
    fn bsc_text_beats_lzma_on_text() {
        use arcmax::codec::lzma::LzmaOptions;

        let text = make_text_fixture();
        let bsc_size = compressed_size(Method::Bsc(BscOptions::default()), &text);
        let lzma_size = compressed_size(Method::Lzma(LzmaOptions::default()), &text);

        // BSC is expected to match or beat LZMA on compressible text.
        // We allow 5% slack because both codecs vary with block size.
        let threshold = lzma_size + lzma_size / 20;
        assert!(
            bsc_size <= threshold,
            "BSC ({bsc_size} B) should be within 5% of LZMA ({lzma_size} B) on text"
        );
    }

    #[test]
    fn bsc_parses_from_method_string() {
        use std::str::FromStr;
        let m = Method::from_str("bsc").unwrap();
        assert!(matches!(m, Method::Bsc(_)));
    }

    #[test]
    fn bsc_plan_is_supported() {
        use arcmax::method::planner::plan;
        let stages = plan(&Method::Bsc(BscOptions::default()));
        assert_eq!(stages.len(), 1);
        assert!(stages[0].is_supported());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SREP-friendly file types (VmImage, ColumnarData, TarBundle)
// ─────────────────────────────────────────────────────────────────────────────

mod srep_routing {
    use super::*;
    use arcmax::filetype::DEFAULT_SREP_THRESHOLD;

    // ── Extension detection ──────────────────────────────────────────────────

    #[test]
    fn vm_image_extensions_detected() {
        for ext in &[
            "vhd", "vhdx", "vmdk", "qcow", "qcow2", "vdi", "img", "raw", "iso", "dmg", "wim", "esd",
        ] {
            let name = format!("disk.{}", ext);
            assert_eq!(
                detect_by_extension(&name),
                FileTypeHint::VmImage,
                "{} should be VmImage",
                ext
            );
        }
    }

    #[test]
    fn columnar_extensions_detected() {
        for ext in &["parquet", "arrow", "feather", "orc", "avro"] {
            let name = format!("data.{}", ext);
            assert_eq!(
                detect_by_extension(&name),
                FileTypeHint::ColumnarData,
                "{} should be ColumnarData",
                ext
            );
        }
    }

    #[test]
    fn tar_bundle_extensions_detected() {
        for ext in &["tar", "cpio", "ar", "deb", "rpm", "pkg", "ipk"] {
            let name = format!("bundle.{}", ext);
            assert_eq!(
                detect_by_extension(&name),
                FileTypeHint::TarBundle,
                "{} should be TarBundle",
                ext
            );
        }
    }

    // ── Magic-byte detection ──────────────────────────────────────────────────

    #[test]
    fn vhdx_magic_detected() {
        let mut header = b"vhdxfile".to_vec();
        header.extend_from_slice(&[0u8; 8]);
        assert_eq!(detect_by_magic(&header), FileTypeHint::VmImage);
    }

    #[test]
    fn vmdk_magic_detected() {
        let header = b"KDMV\x01\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(detect_by_magic(header), FileTypeHint::VmImage);
    }

    #[test]
    fn qcow_magic_detected() {
        let header = b"QFI\xfb\x00\x00\x00\x03\x00\x00\x00\x00";
        assert_eq!(detect_by_magic(header), FileTypeHint::VmImage);
    }

    #[test]
    fn parquet_magic_detected() {
        let header = b"PAR1\x00\x00\x00\x00";
        assert_eq!(detect_by_magic(header), FileTypeHint::ColumnarData);
    }

    #[test]
    fn arrow_magic_detected() {
        let header = b"ARROW1\x00\x00\x00\x00";
        assert_eq!(detect_by_magic(header), FileTypeHint::ColumnarData);
    }

    // ── benefits_from_srep() classification ──────────────────────────────────

    #[test]
    fn srep_friendly_hints_marked() {
        assert!(FileTypeHint::VmImage.benefits_from_srep());
        assert!(FileTypeHint::ColumnarData.benefits_from_srep());
        assert!(FileTypeHint::TarBundle.benefits_from_srep());
        assert!(FileTypeHint::Binary.benefits_from_srep());
    }

    #[test]
    fn srep_unfriendly_hints_excluded() {
        for h in [
            FileTypeHint::Text,
            FileTypeHint::Archive,
            FileTypeHint::CompressedImage,
            FileTypeHint::CompressedMedia,
            FileTypeHint::AudioPcm,
            FileTypeHint::RawImage,
            FileTypeHint::Database,
            FileTypeHint::Executable,
            FileTypeHint::Unknown,
        ] {
            assert!(!h.benefits_from_srep(), "{:?} should not opt into SREP", h);
        }
    }

    // ── Size-aware routing ────────────────────────────────────────────────────

    fn pipeline_starts_with_srep(m: &Method) -> bool {
        match m {
            Method::Pipeline(stages) => matches!(stages.first(), Some(Method::Srep(_))),
            _ => false,
        }
    }

    fn pipeline_contains_srep(m: &Method) -> bool {
        match m {
            Method::Pipeline(stages) => stages.iter().any(|s| matches!(s, Method::Srep(_))),
            Method::Srep(_) => true,
            _ => false,
        }
    }

    #[test]
    fn vm_image_below_threshold_skips_srep() {
        let cfg = fast_cfg().with_input_size(100 * 1024 * 1024); // 100 MiB
        let m = suggest_pipeline(FileTypeHint::VmImage, &cfg);
        assert!(
            !pipeline_contains_srep(&m),
            "VmImage at 100 MiB should not insert SREP, got: {:?}",
            m
        );
    }

    #[test]
    fn vm_image_above_threshold_uses_srep() {
        let cfg = fast_cfg().with_input_size(2 * 1024 * 1024 * 1024); // 2 GiB
        let m = suggest_pipeline(FileTypeHint::VmImage, &cfg);
        assert!(
            pipeline_starts_with_srep(&m),
            "VmImage at 2 GiB should start with SREP, got: {:?}",
            m
        );
        // VmImage should pin LZMA as the back-end regardless of default_codec.
        if let Method::Pipeline(stages) = &m {
            assert!(
                matches!(stages.last(), Some(Method::Lzma(_))),
                "VmImage back-end should be LZMA"
            );
        }
    }

    #[test]
    fn columnar_above_threshold_uses_srep() {
        let cfg = fast_cfg().with_input_size(DEFAULT_SREP_THRESHOLD);
        let m = suggest_pipeline(FileTypeHint::ColumnarData, &cfg);
        assert!(
            pipeline_starts_with_srep(&m),
            "ColumnarData at threshold should start with SREP, got: {:?}",
            m
        );
    }

    #[test]
    fn tar_bundle_above_threshold_uses_srep() {
        let cfg = fast_cfg().with_input_size(DEFAULT_SREP_THRESHOLD);
        let m = suggest_pipeline(FileTypeHint::TarBundle, &cfg);
        assert!(
            pipeline_starts_with_srep(&m),
            "TarBundle at threshold should start with SREP, got: {:?}",
            m
        );
    }

    #[test]
    fn binary_above_threshold_uses_srep() {
        let cfg = fast_cfg().with_input_size(DEFAULT_SREP_THRESHOLD * 2);
        let m = suggest_pipeline(FileTypeHint::Binary, &cfg);
        assert!(
            pipeline_contains_srep(&m),
            "Large Binary should route through SREP, got: {:?}",
            m
        );
    }

    #[test]
    fn text_never_uses_srep_even_at_huge_size() {
        // Empirical finding: PPMd on 2 GiB CBETA XML = 0.143×; SREP+PPMd = 0.143×.
        // SREP must NOT be inserted for text regardless of size.
        let cfg = fast_cfg().with_input_size(10 * 1024 * 1024 * 1024); // 10 GiB
        let m = suggest_pipeline(FileTypeHint::Text, &cfg);
        assert!(
            !pipeline_contains_srep(&m),
            "Text must never route through SREP, got: {:?}",
            m
        );
    }

    #[test]
    fn unknown_size_skips_srep() {
        // input_size_hint = None → never insert SREP.
        let cfg = fast_cfg(); // no size hint
        for h in [
            FileTypeHint::VmImage,
            FileTypeHint::ColumnarData,
            FileTypeHint::TarBundle,
            FileTypeHint::Binary,
        ] {
            let m = suggest_pipeline(h, &cfg);
            assert!(
                !pipeline_contains_srep(&m),
                "{:?} without size hint should not insert SREP, got: {:?}",
                h,
                m
            );
        }
    }

    #[test]
    fn executable_above_threshold_uses_srep() {
        // Large executables (installers, single-binary games) benefit from SREP
        // after dispack normalises call/jump offsets.
        let cfg = fast_cfg().with_input_size(2 * 1024 * 1024 * 1024); // 2 GiB
        let m = suggest_pipeline(FileTypeHint::Executable, &cfg);
        assert!(
            pipeline_contains_srep(&m),
            "Large Executable should route through SREP, got: {:?}",
            m
        );
        // Must still start with Dispack.
        if let Method::Pipeline(stages) = &m {
            assert!(
                matches!(stages.first(), Some(Method::Dispack(_))),
                "Executable pipeline must start with Dispack"
            );
        }
    }

    // ── ArcMax archive detection ─────────────────────────────────────────────

    #[test]
    fn amx_extension_detected_as_arcmax_archive() {
        assert_eq!(
            detect_by_extension("backup.amx"),
            FileTypeHint::ArcmaxArchive
        );
    }

    #[test]
    fn arcmax_magic_detected() {
        let magic = b"ARCMAX\x01\x00rest of data here";
        assert_eq!(detect_by_magic(magic), FileTypeHint::ArcmaxArchive);
        assert_eq!(
            detect("noext", magic),
            FileTypeHint::ArcmaxArchive,
            "magic fallback should detect ArcMax"
        );
    }

    #[test]
    fn arcmax_archive_is_already_compressed() {
        assert!(FileTypeHint::ArcmaxArchive.is_already_compressed());
    }

    #[test]
    fn arcmax_archive_suggestion_is_store() {
        let m = suggest_pipeline(FileTypeHint::ArcmaxArchive, &fast_cfg());
        assert!(
            matches!(m, Method::Store),
            "ArcmaxArchive must suggest Store, got: {:?}",
            m
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 6.1 — sniff_tar_inner and recursive bundle routing
// ─────────────────────────────────────────────────────────────────────────────

mod bundle_sniff_tests {
    use super::*;

    /// Build a minimal POSIX ustar TAR with one file entry in memory.
    ///
    /// Each entry is a 512-byte header block.  File data is padded to
    /// 512-byte multiples.  Two zero blocks terminate the archive.
    fn build_tar(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, data) in entries {
            let mut header = [0u8; 512];
            // Filename (bytes 0–99)
            let name_bytes = name.as_bytes();
            let name_len = name_bytes.len().min(99);
            header[..name_len].copy_from_slice(&name_bytes[..name_len]);
            // File permissions: octal "0000644\0" (bytes 100–107)
            header[100..108].copy_from_slice(b"0000644\0");
            // File size: octal, null-padded, 12 bytes (bytes 124–135)
            let size_str = format!("{:011o}\0", data.len());
            header[124..136].copy_from_slice(size_str.as_bytes());
            // Type flag: '0' = regular file (byte 156)
            header[156] = b'0';
            // ustar magic (bytes 257–262)
            header[257..263].copy_from_slice(b"ustar\0");
            // Checksum field (bytes 148–155): sum of all header bytes
            // with checksum field treated as 8 spaces.
            header[148..156].fill(b' ');
            let chksum: u32 = header.iter().map(|&b| b as u32).sum();
            let chk_str = format!("{:06o}\0 ", chksum);
            header[148..156].copy_from_slice(chk_str.as_bytes());

            out.extend_from_slice(&header);
            out.extend_from_slice(data);
            // Pad data to 512-byte boundary.
            let pad = (512 - data.len() % 512) % 512;
            out.extend(std::iter::repeat(0).take(pad));
        }
        // Two zero-block end-of-archive marker.
        out.extend([0u8; 1024]);
        out
    }

    #[test]
    fn sniff_text_tar_returns_text() {
        let tar = build_tar(&[
            ("readme.txt", b"hello world"),
            ("notes.md", b"more text"),
            ("config.json", b"{}"),
        ]);
        assert_eq!(sniff_tar_inner(&tar), FileTypeHint::Text);
    }

    #[test]
    fn sniff_binary_tar_returns_binary_or_executable() {
        let tar = build_tar(&[
            ("tool.exe", b"\x4d\x5a payload"),
            ("lib.dll", b"\x4d\x5a payload"),
            ("helper.obj", b"object data"),
        ]);
        // All executables — hint should be Executable (not TarBundle).
        let hint = sniff_tar_inner(&tar);
        assert!(
            matches!(hint, FileTypeHint::Executable | FileTypeHint::Binary),
            "expected Executable or Binary, got {hint:?}"
        );
    }

    #[test]
    fn sniff_image_tar_returns_compressed_image() {
        let tar = build_tar(&[
            ("photo1.jpg", &[0xFF, 0xD8, 0xFF, 0xE0]),
            ("photo2.jpg", &[0xFF, 0xD8, 0xFF, 0xE1]),
            ("photo3.png", b"png data"),
        ]);
        assert_eq!(sniff_tar_inner(&tar), FileTypeHint::CompressedImage);
    }

    #[test]
    fn sniff_empty_tar_falls_back_to_tar_bundle() {
        let tar = build_tar(&[]);
        assert_eq!(sniff_tar_inner(&tar), FileTypeHint::TarBundle);
    }

    #[test]
    fn sniff_all_unknown_extensions_falls_back_to_tar_bundle() {
        let tar = build_tar(&[("data.xyz123", b"unknown"), ("file.qwerty", b"unknown")]);
        assert_eq!(sniff_tar_inner(&tar), FileTypeHint::TarBundle);
    }

    #[test]
    fn tar_bundle_with_sample_routes_by_inner_type() {
        // A TAR full of .rs source files → should route as Text.
        let tar = build_tar(&[
            ("src/main.rs", b"fn main() {}"),
            ("src/lib.rs", b"pub fn foo() {}"),
            ("README.md", b"# Project"),
        ]);
        let cfg = fast_cfg().with_bundle_sample(&tar);
        let method = suggest_pipeline(FileTypeHint::TarBundle, &cfg);
        // Text routing → PPMd (not LZ4, not Store).
        assert!(
            matches!(method, Method::Ppmd(_)),
            "TAR of source files should suggest PPMd, got {method:?}"
        );
    }

    #[test]
    fn tar_bundle_without_sample_uses_default_routing() {
        // Without a sample, TarBundle falls through to with_srep(default_codec).
        let cfg = fast_cfg(); // no bundle_sample
        let method = suggest_pipeline(FileTypeHint::TarBundle, &cfg);
        // Should be LZ4 (fast_cfg) or Pipeline(SREP, LZ4) — not PPMd.
        assert!(
            !matches!(method, Method::Ppmd(_)),
            "TarBundle without sample should not suggest PPMd, got {method:?}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 6.2 — CompressionTarget cost model
// ─────────────────────────────────────────────────────────────────────────────

mod target_tests {
    use super::*;

    #[test]
    fn speed_target_uses_lz4() {
        let cfg = SuggestionConfig::default().with_target(CompressionTarget::Speed);
        let m = cfg.default_codec.clone();
        assert!(
            matches!(m, Method::Lz4(_)),
            "Speed target should use LZ4, got {m:?}"
        );
    }

    #[test]
    fn balanced_target_uses_zstd() {
        let cfg = SuggestionConfig::default().with_target(CompressionTarget::Balanced);
        let m = cfg.default_codec.clone();
        assert!(
            matches!(m, Method::Zstd(_)),
            "Balanced target should use Zstd, got {m:?}"
        );
    }

    #[test]
    fn ratio_target_uses_lzma() {
        let cfg = SuggestionConfig::default().with_target(CompressionTarget::Ratio);
        let m = cfg.default_codec.clone();
        assert!(
            matches!(m, Method::Lzma(_)),
            "Ratio target should use LZMA, got {m:?}"
        );
    }

    #[test]
    fn speed_target_text_uses_default_codec() {
        // Speed target → TextCodec::Default → falls through to default_codec (LZ4).
        let cfg = SuggestionConfig::default().with_target(CompressionTarget::Speed);
        let m = suggest_pipeline(FileTypeHint::Text, &cfg);
        assert!(
            matches!(m, Method::Lz4(_)),
            "Speed target text should fall back to LZ4, got {m:?}"
        );
    }

    #[test]
    fn balanced_target_text_uses_ppmd_order4() {
        let cfg = SuggestionConfig::default().with_target(CompressionTarget::Balanced);
        let m = suggest_pipeline(FileTypeHint::Text, &cfg);
        if let Method::Ppmd(opts) = m {
            assert_eq!(opts.order, 4, "Balanced target should use PPMd order 4");
        } else {
            panic!("Balanced target text should suggest PPMd, got {m:?}");
        }
    }

    #[test]
    fn ratio_target_text_uses_ppmd_default_order() {
        let cfg = SuggestionConfig::default().with_target(CompressionTarget::Ratio);
        let m = suggest_pipeline(FileTypeHint::Text, &cfg);
        assert!(
            matches!(m, Method::Ppmd(_)),
            "Ratio target text should suggest PPMd, got {m:?}"
        );
    }

    #[test]
    fn speed_target_binary_roundtrips() {
        let input = (0u8..=255).cycle().take(4096).collect::<Vec<_>>();
        let cfg = SuggestionConfig::default().with_target(CompressionTarget::Speed);
        let m = suggest_pipeline(FileTypeHint::Binary, &cfg);
        roundtrip(m, &input);
    }

    #[test]
    fn balanced_target_binary_roundtrips() {
        let input = b"balanced binary roundtrip data ".repeat(256);
        let cfg = SuggestionConfig::default().with_target(CompressionTarget::Balanced);
        let m = suggest_pipeline(FileTypeHint::Binary, &cfg);
        roundtrip(m, &input);
    }

    #[test]
    fn with_target_then_override_codec_works() {
        // with_target sets default_codec, but a later with_default_codec wins.
        let cfg = SuggestionConfig::default()
            .with_target(CompressionTarget::Speed)
            .with_default_codec(Method::Store);
        assert!(
            matches!(cfg.default_codec, Method::Store),
            "explicit override after with_target should win"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Exe compression benchmark — real file, ignored by default
//
// Verifies that:
//   1. detect() routes sinorag.exe to FileTypeHint::Executable.
//   2. suggest_pipeline inserts Dispack before the main compressor.
//   3. The Dispack+LZMA pipeline beats plain LZMA (no filter) on a real exe.
//   4. Roundtrip is lossless.
//
// Run with:
//   cargo test --test filetype_headsup exe_compression -- --ignored --nocapture
// ─────────────────────────────────────────────────────────────────────────────

mod exe_benchmark {
    use super::*;
    use arcmax::codec::lzma::LzmaOptions;
    use arcmax::filetype::{detect, CompressionTarget, FileTypeHint, SuggestionConfig};
    use arcmax::method::pipeline::CodecPipeline;
    use arcmax::method::Method;
    use std::io::Cursor;
    use std::path::Path;
    use std::time::Instant;

    const EXE_PATH: &str = r"D:\Rust-projects\SinoRAG-runtime\sinorag.exe";
    /// 7-zip maximum compression output on the same file (reference baseline).
    const SEVENZIP_REFERENCE_BYTES: usize = 17 * 1024 * 1024;

    fn compress_to_vec(method: Method, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() / 4);
        CodecPipeline::new(method)
            .compress(Cursor::new(data), &mut out)
            .expect("compress failed");
        out
    }

    fn decompress_to_vec(method: Method, compressed: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        CodecPipeline::new(method)
            .decompress(Cursor::new(compressed), &mut out)
            .expect("decompress failed");
        out
    }

    fn ratio(compressed: usize, original: usize) -> f64 {
        compressed as f64 / original as f64
    }

    fn lzma_max(dict_mib: u32) -> Method {
        Method::Lzma(LzmaOptions {
            level: Some(9),
            dict_size: dict_mib * 1024 * 1024,
            lc: 3,
            lp: 0,
            pb: 2,
            nice_len: None,
            lzma2: true,
        })
    }

    /// Compression pipeline struct for the benchmark table.
    struct Candidate {
        label: &'static str,
        method: Method,
    }

    #[test]
    #[ignore = "requires D:\\Rust-projects\\SinoRAG-runtime\\sinorag.exe; \
                run with: cargo test --test filetype_headsup exe_compression -- --ignored --nocapture"]
    fn exe_compression_benchmark_sinorag() {
        let path = Path::new(EXE_PATH);
        if !path.exists() {
            eprintln!("SKIP: {EXE_PATH} not found");
            return;
        }

        let data = std::fs::read(path).expect("read exe");
        let original_len = data.len();
        println!(
            "\n=== sinorag.exe benchmark ({:.1} MiB) ===",
            original_len as f64 / (1024.0 * 1024.0)
        );
        println!(
            "  7-zip reference (max, custom dict): {:.2} MiB  ({:.1}%)",
            SEVENZIP_REFERENCE_BYTES as f64 / (1024.0 * 1024.0),
            ratio(SEVENZIP_REFERENCE_BYTES, original_len) * 100.0
        );

        // ── Detection ────────────────────────────────────────────────────────
        let header = &data[..data.len().min(16)];
        let hint = detect("sinorag.exe", header);
        assert_eq!(
            hint,
            FileTypeHint::Executable,
            "detect() should classify sinorag.exe as Executable"
        );

        // ── Pipeline structure ────────────────────────────────────────────────
        let ratio_cfg = SuggestionConfig::default()
            .with_target(CompressionTarget::Ratio)
            .with_default_codec(lzma_max(128));
        let suggested = suggest_pipeline(hint, &ratio_cfg);
        match &suggested {
            Method::Pipeline(stages) => {
                assert!(
                    matches!(stages.first(), Some(Method::Dispack(_))),
                    "suggest_pipeline(Executable) must start with Dispack, got: {suggested:?}"
                );
            }
            other => panic!("expected Pipeline from suggest_pipeline(Executable), got {other:?}"),
        }

        // ── Candidates ────────────────────────────────────────────────────────
        let candidates: Vec<Candidate> = vec![
            Candidate {
                label: "plain lzma:9:d64m  (no filter, baseline)",
                method: lzma_max(64),
            },
            Candidate {
                label: "plain lzma:9:d128m (no filter)",
                method: lzma_max(128),
            },
            Candidate {
                label: "exe(bcj_x86) + lzma:9:d64m",
                method: Method::Pipeline(vec![
                    Method::BcjX86(arcmax::codec::filters::BcjX86Options),
                    lzma_max(64),
                ]),
            },
            Candidate {
                label: "exe(bcj_x86) + lzma:9:d128m",
                method: Method::Pipeline(vec![
                    Method::BcjX86(arcmax::codec::filters::BcjX86Options),
                    lzma_max(128),
                ]),
            },
            Candidate {
                label: "dispack + lzma:9:d64m  (suggested pipeline)",
                method: Method::Pipeline(vec![
                    Method::Dispack(arcmax::codec::filters::DispackOptions::default()),
                    lzma_max(64),
                ]),
            },
            Candidate {
                label: "dispack + lzma:9:d128m (suggested pipeline)",
                method: Method::Pipeline(vec![
                    Method::Dispack(arcmax::codec::filters::DispackOptions::default()),
                    lzma_max(128),
                ]),
            },
            Candidate {
                label: "dispack + lzma:9:d256m",
                method: Method::Pipeline(vec![
                    Method::Dispack(arcmax::codec::filters::DispackOptions::default()),
                    lzma_max(256),
                ]),
            },
        ];

        // ── Run and print table ───────────────────────────────────────────────
        println!(
            "\n  {:<45} {:>10}  {:>8}  {:>8}",
            "Pipeline", "Size (MiB)", "Ratio", "Time (s)"
        );
        println!("  {}", "-".repeat(78));

        let mut dispack_lzma64_size = None;
        let mut baseline_size = None;
        let mut best_size = usize::MAX;

        for c in &candidates {
            let t0 = Instant::now();
            let compressed = compress_to_vec(c.method.clone(), &data);
            let elapsed = t0.elapsed();
            let sz = compressed.len();

            println!(
                "  {:<45} {:>10.2}  {:>7.1}%  {:>8.2}",
                c.label,
                sz as f64 / (1024.0 * 1024.0),
                ratio(sz, original_len) * 100.0,
                elapsed.as_secs_f64()
            );

            if c.label.starts_with("plain lzma:9:d64m") {
                baseline_size = Some(sz);
            }
            if c.label.starts_with("dispack + lzma:9:d64m") {
                dispack_lzma64_size = Some(sz);
            }
            if sz < best_size {
                best_size = sz;
            }
        }

        println!(
            "\n  Best arcmax:    {:.2} MiB  ({:.1}%)",
            best_size as f64 / (1024.0 * 1024.0),
            ratio(best_size, original_len) * 100.0
        );
        println!(
            "  7-zip ref:      {:.2} MiB  ({:.1}%)",
            SEVENZIP_REFERENCE_BYTES as f64 / (1024.0 * 1024.0),
            ratio(SEVENZIP_REFERENCE_BYTES, original_len) * 100.0
        );

        // ── Assertions ────────────────────────────────────────────────────────

        // Dispack+LZMA must beat plain LZMA (the filter must help).
        if let (Some(filtered), Some(plain)) = (dispack_lzma64_size, baseline_size) {
            assert!(
                filtered < plain,
                "dispack+lzma:9:d64m ({filtered} B) should compress better than \
                 plain lzma:9:d64m ({plain} B); exe filter is not helping"
            );
        }

        // ── Roundtrip correctness on a 1 MiB sample (full roundtrip would be slow) ──
        let sample = &data[..data.len().min(1024 * 1024)];
        let pipeline = Method::Pipeline(vec![
            Method::Dispack(arcmax::codec::filters::DispackOptions::default()),
            lzma_max(64),
        ]);
        let compressed = compress_to_vec(pipeline.clone(), sample);
        let decoded = decompress_to_vec(pipeline, &compressed);
        assert_eq!(
            decoded, sample,
            "dispack+lzma roundtrip mismatch on first 1 MiB of sinorag.exe"
        );
        println!("\n  Roundtrip OK (first 1 MiB)");
    }
}
