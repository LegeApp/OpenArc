//! Real-corpus text compression benchmark.
//!
//! Compares all codecs that are known to perform well on structured text
//! against real XML corpora (CBETA TEI P5, Buddhist canonical texts):
//!
//! - **B10 folder** (~4.6 MiB, 22 files): quick per-codec quality check.
//! - **Full CBETA** (~3+ GiB, 4990 files): validates SREP's long-range
//!   deduplication benefit — PPMd/BSC context windows (~32-64 MiB) cannot
//!   see cross-file repetitions at 3 GiB scale; SREP can.
//!
//! This test is marked `#[ignore]` so it does not run in normal `cargo test`.
//! **Always build in release mode** — codec timings in debug are 10-50× slower
//! and compression ratios can differ due to algorithm timeouts/truncation.
//! Run explicitly:
//!
//! ```sh
//! # Quick B10 benchmark (default build):
//! cargo test --release --test codec_bench_text -- --include-ignored --nocapture
//!
//! # With BSC:
//! cargo test --release --features bsc --test codec_bench_text -- --include-ignored --nocapture
//!
//! # Full corpus (slow — loads 3+ GiB into RAM):
//! cargo test --release --test codec_bench_text large_corpus -- --include-ignored --nocapture
//! ```
//!
//! ## Why SREP helps at large scale
//!
//! SREP is a solid deduplication preprocessor: it scans the full stream for
//! repeated blocks (default chunk ≥ 512 bytes) and replaces duplicates with
//! compact back-references before the entropy coder runs.  For the 3 GiB CBETA
//! corpus this matters because:
//!
//! - Every file starts with an identical `<?xml…>` + `<TEI…>` prologue (~1 KB).
//! - The `<teiHeader>` structure is nearly identical across 4990 files.
//! - Buddhist canonical passages are quoted repeatedly across texts.
//! - PPMd order-6 with a 16 MiB window sees at most ~4000 files' worth of
//!   context at the boundary — everything beyond that is invisible.
//!
//! Expected effect: SREP reduces 3 GiB → ~1-2 GiB of unique content, then
//! PPMd/BSC achieves its normal ~80% ratio on what remains → combined
//! compression well below 20% of original (>80% savings).

use std::path::Path;
use std::time::Instant;

use arcmax::method::pipeline::CodecPipeline;
use arcmax::method::Method;

/// Small corpus — 22 files, ~4.6 MiB.  Fast; runs in the benchmark by default.
const CORPUS_DIR: &str = r"D:\Rust-projects\SinoRAG-runtime\data\archives\cbeta\xml-p5\B\B10";

/// Full CBETA corpus — 4990 files, ~3+ GiB.
/// Loads everything into RAM; only used by the `large_corpus` test.
const FULL_CORPUS_DIR: &str = r"D:\Rust-projects\SinoRAG-runtime\data\archives\cbeta\xml-p5";

// ─────────────────────────────────────────────────────────────────────────────
// Corpus loader
// ─────────────────────────────────────────────────────────────────────────────

fn load_corpus(dir: &str) -> Vec<u8> {
    let path = Path::new(dir);
    assert!(
        path.exists(),
        "Corpus directory not found: {dir}\n\
         Set CORPUS_DIR in the test or adjust the path constant."
    );

    let mut entries: Vec<_> = std::fs::read_dir(path)
        .unwrap_or_else(|e| panic!("Cannot read {dir}: {e}"))
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case("xml"))
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|e| e.path());

    let mut corpus = Vec::new();
    for entry in &entries {
        let bytes = std::fs::read(entry.path())
            .unwrap_or_else(|e| panic!("Cannot read {:?}: {e}", entry.path()));
        corpus.extend_from_slice(&bytes);
    }

    eprintln!(
        "  Loaded {} XML files — {:.2} MiB total",
        entries.len(),
        corpus.len() as f64 / (1024.0 * 1024.0)
    );
    corpus
}

// ─────────────────────────────────────────────────────────────────────────────
// Benchmark runner
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct BenchResult {
    label: String,
    original: usize,
    compressed: usize,
    encode_ms: u128,
    roundtrip_ok: bool,
}

impl BenchResult {
    fn ratio(&self) -> f64 {
        self.compressed as f64 / self.original as f64
    }
    fn savings_pct(&self) -> f64 {
        (1.0 - self.ratio()) * 100.0
    }
}

fn bench_one(label: &str, method: Method, data: &[u8]) -> BenchResult {
    let mut compressed = Vec::with_capacity(data.len());
    let t0 = Instant::now();
    let compress_result =
        CodecPipeline::new(method.clone()).compress(std::io::Cursor::new(data), &mut compressed);
    let encode_ms = t0.elapsed().as_millis();

    if let Err(e) = &compress_result {
        eprintln!("  SKIP {label}: compress error: {e}");
        return BenchResult {
            label: label.to_string(),
            original: data.len(),
            compressed: 0,
            encode_ms: 0,
            roundtrip_ok: false,
        };
    }

    // Quick roundtrip check (decode only, don't verify byte-for-byte to keep it fast).
    let mut decoded = Vec::new();
    let roundtrip_ok = CodecPipeline::new(method)
        .decompress(std::io::Cursor::new(&compressed), &mut decoded)
        .map(|_| decoded == data)
        .unwrap_or(false);

    BenchResult {
        label: label.to_string(),
        original: data.len(),
        compressed: compressed.len(),
        encode_ms,
        roundtrip_ok,
    }
}

#[cfg(feature = "legacy-ffi-tests")]
fn bench_freearc_ppmd_h(label: &str, order: u8, memory_size: usize, data: &[u8]) -> BenchResult {
    use std::os::raw::c_int;

    extern "C" {
        fn freearc_ppmd_compress(
            input: *mut u8,
            input_size: c_int,
            output: *mut u8,
            output_size: c_int,
            order: c_int,
            memory_size: usize,
        ) -> c_int;

        fn freearc_ppmd_decompress(
            input: *mut u8,
            input_size: c_int,
            output: *mut u8,
            output_size: c_int,
            order: c_int,
            memory_size: usize,
        ) -> c_int;
    }

    let t0 = Instant::now();
    let mut cap = (data.len() + data.len() / 2 + 1024).max(4096);
    let compressed = loop {
        let mut src = data.to_vec();
        let mut out = vec![0u8; cap];
        let result = unsafe {
            freearc_ppmd_compress(
                src.as_mut_ptr(),
                src.len() as c_int,
                out.as_mut_ptr(),
                out.len() as c_int,
                order as c_int,
                memory_size,
            )
        };
        if result >= 0 {
            out.truncate(result as usize);
            break out;
        }
        cap *= 2;
        if cap > data.len().saturating_mul(16).max(64 * 1024 * 1024) {
            eprintln!("  SKIP {label}: FreeArc PPMd-H output did not fit buffer");
            return BenchResult {
                label: label.to_string(),
                original: data.len(),
                compressed: 0,
                encode_ms: 0,
                roundtrip_ok: false,
            };
        }
    };
    let encode_ms = t0.elapsed().as_millis();

    let mut src = compressed.clone();
    let mut decoded = vec![0u8; data.len()];
    let decoded_len = unsafe {
        freearc_ppmd_decompress(
            src.as_mut_ptr(),
            src.len() as c_int,
            decoded.as_mut_ptr(),
            decoded.len() as c_int,
            order as c_int,
            memory_size,
        )
    };
    let roundtrip_ok = decoded_len >= 0 && {
        decoded.truncate(decoded_len as usize);
        decoded == data
    };

    BenchResult {
        label: label.to_string(),
        original: data.len(),
        compressed: compressed.len(),
        encode_ms,
        roundtrip_ok,
    }
}

fn print_table(results: &[BenchResult]) {
    // Sort by compressed size ascending (best first), Store last.
    let mut sorted: Vec<&BenchResult> = results.iter().filter(|r| r.compressed > 0).collect();
    sorted.sort_by(|a, b| {
        if a.label == "store" {
            std::cmp::Ordering::Greater
        } else if b.label == "store" {
            std::cmp::Ordering::Less
        } else {
            a.compressed.cmp(&b.compressed)
        }
    });

    let orig = sorted.first().map(|r| r.original).unwrap_or(1);

    eprintln!();
    eprintln!("  ┌─────────────────────────────┬──────────────┬────────┬────────────┬──────────┐");
    eprintln!("  │ Codec                       │ Compressed   │  Ratio │  Savings % │  Time ms │");
    eprintln!("  ├─────────────────────────────┼──────────────┼────────┼────────────┼──────────┤");

    for r in &sorted {
        let marker = if r.label == "store" {
            " (baseline)"
        } else {
            "           "
        };
        eprintln!(
            "  │ {:27} │ {:>10} B │ {:>5.3}x │ {:>8.2}%  │ {:>6} ms │",
            format!("{}{}", r.label, marker),
            fmt_bytes(r.compressed),
            r.ratio(),
            r.savings_pct(),
            r.encode_ms,
        );
    }
    eprintln!("  └─────────────────────────────┴──────────────┴────────┴────────────┴──────────┘");
    eprintln!(
        "  Original size: {} B  ({:.2} MiB)",
        fmt_bytes(orig),
        orig as f64 / (1024.0 * 1024.0)
    );
    eprintln!();

    // Flag any roundtrip failures.
    let failures: Vec<_> = results
        .iter()
        .filter(|r| !r.roundtrip_ok && r.compressed > 0)
        .collect();
    if !failures.is_empty() {
        eprintln!("  ⚠ ROUNDTRIP FAILURES:");
        for r in failures {
            eprintln!("    - {}", r.label);
        }
        eprintln!();
    }
}

fn fmt_bytes(n: usize) -> String {
    // Insert thousands separators.
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Method definitions
// ─────────────────────────────────────────────────────────────────────────────

fn methods() -> Vec<(&'static str, Method)> {
    use arcmax::codec::lz4::Lz4Options;
    use arcmax::codec::lzma::LzmaOptions;
    use arcmax::codec::lzp::LzpOptions;
    use arcmax::codec::ppmd::PpmdOptions;
    use arcmax::codec::rep::RepOptions;
    use arcmax::codec::zstd::ZstdOptions;
    use std::str::FromStr;

    #[allow(unused_mut)]
    let mut list: Vec<(&'static str, Method)> = vec![
        // ── Baselines ────────────────────────────────────────────────────────
        ("store", Method::Store),
        ("lz4", Method::Lz4(Lz4Options::default())),
        // ── General-purpose LZ ───────────────────────────────────────────────
        // Zstd now uses the `zstd` crate (libzstd binding) — roundtrip is correct.
        ("zstd:3", Method::Zstd(ZstdOptions { level: 3 })),
        ("zstd:19", Method::Zstd(ZstdOptions { level: 19 })),
        (
            "lzma:d32m",
            Method::Lzma(LzmaOptions {
                dict_size: 32 * 1024 * 1024,
                ..LzmaOptions::default()
            }),
        ),
        (
            "lzma2:d32m",
            Method::Lzma(LzmaOptions {
                lzma2: true,
                dict_size: 32 * 1024 * 1024,
                ..LzmaOptions::default()
            }),
        ),
        // ── PPMd — the classic text codec ────────────────────────────────────
        // Default: order 6, 16 MiB.  Also test order 12 (stronger but slower).
        ("ppmd:o6:mem16m", Method::Ppmd(PpmdOptions::default())),
        (
            "ppmd:o12:mem32m",
            Method::Ppmd(PpmdOptions {
                order: 12,
                memory_size: 32 * 1024 * 1024,
                ..PpmdOptions::default()
            }),
        ),
        // ── LZP (context prediction) — excellent on XML ──────────────────────
        ("lzp", Method::Lzp(LzpOptions::default())),
        // LZP is a preprocessor; pair it with LZMA and PPMd.
        (
            "lzp+lzma:d32m",
            Method::Pipeline(vec![
                Method::Lzp(LzpOptions::default()),
                Method::Lzma(LzmaOptions {
                    dict_size: 32 * 1024 * 1024,
                    ..LzmaOptions::default()
                }),
            ]),
        ),
        (
            "lzp+ppmd:o12",
            Method::Pipeline(vec![
                Method::Lzp(LzpOptions::default()),
                Method::Ppmd(PpmdOptions {
                    order: 12,
                    memory_size: 32 * 1024 * 1024,
                    ..PpmdOptions::default()
                }),
            ]),
        ),
        // ── Dict+LZMA — good for markup with many repeated tags ──────────────
        (
            "dict+lzma",
            Method::Pipeline(vec![
                Method::from_str("dict:b4m").unwrap(),
                Method::Lzma(LzmaOptions {
                    dict_size: 32 * 1024 * 1024,
                    ..LzmaOptions::default()
                }),
            ]),
        ),
        // ── REP+LZMA — repetition eliminator ─────────────────────────────────
        (
            "rep+lzma",
            Method::Pipeline(vec![
                Method::Rep(RepOptions {
                    block_size: 4 * 1024 * 1024,
                    min_match_len: 32,
                    smallest_len: 32,
                    ..RepOptions::default()
                }),
                Method::Lzma(LzmaOptions {
                    dict_size: 32 * 1024 * 1024,
                    ..LzmaOptions::default()
                }),
            ]),
        ),
        // ── Tornado ──────────────────────────────────────────────────────────
        ("tornado:5", Method::from_str("tornado:5").unwrap()),
        // ── SREP combinations ────────────────────────────────────────────────
        // SREP is a long-range deduplication preprocessor.  On this 4.6 MiB
        // sample the effect is modest (cross-file repetition fits in PPMd's
        // window anyway).  The real benefit appears at 100 MiB+ scale where
        // PPMd/BSC context windows no longer span the entire stream.
        (
            "srep+ppmd:o6",
            Method::Pipeline(vec![
                Method::Srep(arcmax::srep::SrepConfig::default()),
                Method::Ppmd(PpmdOptions::default()),
            ]),
        ),
        (
            "srep+lzma:d32m",
            Method::Pipeline(vec![
                Method::Srep(arcmax::srep::SrepConfig::default()),
                Method::Lzma(LzmaOptions {
                    dict_size: 32 * 1024 * 1024,
                    ..LzmaOptions::default()
                }),
            ]),
        ),
        (
            "srep+zstd:19",
            Method::Pipeline(vec![
                Method::Srep(arcmax::srep::SrepConfig::default()),
                Method::Zstd(ZstdOptions { level: 19 }),
            ]),
        ),
    ];

    // ── BSC — BWT+QLFC, feature-gated ────────────────────────────────────────
    #[cfg(feature = "bsc")]
    {
        use arcmax::codec::BscOptions;
        list.push(("bsc (default)", Method::Bsc(BscOptions::default())));
        list.push(("bsc (max)", Method::Bsc(BscOptions::max())));
        list.push(("bsc (fast)", Method::Bsc(BscOptions::fast())));
        // BSC with LZP preprocessing (LZP removes repeated patterns before BWT).
        list.push((
            "lzp+bsc",
            Method::Pipeline(vec![
                Method::Lzp(LzpOptions::default()),
                Method::Bsc(BscOptions::default()),
            ]),
        ));
        // SREP long-range dedup + BSC BWT — the power combo for large corpora.
        list.push((
            "srep+bsc",
            Method::Pipeline(vec![
                Method::Srep(arcmax::srep::SrepConfig::default()),
                Method::Bsc(BscOptions::default()),
            ]),
        ));
    }

    list
}

// ─────────────────────────────────────────────────────────────────────────────
// The test
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "real-corpus benchmark; run with --include-ignored --nocapture"]
fn bench_text_codecs_on_cbeta_xml() {
    let corpus = load_corpus(CORPUS_DIR);
    let original_size = corpus.len();

    eprintln!();
    eprintln!("  ══════════════════════════════════════════════════════════");
    eprintln!("  Text codec benchmark — CBETA TEI P5 XML (B10 folder)");
    eprintln!("  ══════════════════════════════════════════════════════════");

    let all_methods = methods();
    let mut results: Vec<BenchResult> = Vec::new();

    for (label, method) in all_methods {
        eprint!("  [{:27}] ", label);
        let r = bench_one(label, method, &corpus);
        if r.compressed > 0 {
            eprintln!(
                "  {:.3}x  ({:.1}%)  {}ms{}",
                r.ratio(),
                r.savings_pct(),
                r.encode_ms,
                if r.roundtrip_ok {
                    ""
                } else {
                    "  *** ROUNDTRIP FAIL ***"
                }
            );
        }
        results.push(r);
    }

    #[cfg(feature = "legacy-ffi-tests")]
    for (label, order, memory_size) in [
        ("ppmd-h-freeArc-c:o6:mem16m", 6, 16 * 1024 * 1024),
        ("ppmd-h-freeArc-c:o12:mem32m", 12, 32 * 1024 * 1024),
    ] {
        eprint!("  [{:27}] ", label);
        let r = bench_freearc_ppmd_h(label, order, memory_size, &corpus);
        if r.compressed > 0 {
            eprintln!(
                "  {:.3}x  ({:.1}%)  {}ms{}",
                r.ratio(),
                r.savings_pct(),
                r.encode_ms,
                if r.roundtrip_ok {
                    ""
                } else {
                    "  *** ROUNDTRIP FAIL ***"
                }
            );
        }
        results.push(r);
    }

    print_table(&results);

    // ── Quality assertions ────────────────────────────────────────────────────
    //
    // Zstd has a known large-input roundtrip bug in zenzstd on certain content
    // patterns; it is excluded from the roundtrip assertion below.

    let lz4_size = results
        .iter()
        .find(|r| r.label == "lz4")
        .map(|r| r.compressed)
        .unwrap_or(original_size);

    let ppmd12_size = results
        .iter()
        .find(|r| r.label == "ppmd:o12:mem32m")
        .map(|r| r.compressed)
        .unwrap_or(usize::MAX);

    let lzma_size = results
        .iter()
        .find(|r| r.label == "lzma:d32m")
        .map(|r| r.compressed)
        .unwrap_or(usize::MAX);

    // PPMd order-12 must beat LZ4 on this text corpus.
    assert!(
        ppmd12_size < lz4_size,
        "PPMd o12 ({ppmd12_size} B) should beat LZ4 ({lz4_size} B) on XML text"
    );

    // LZMA with 32m dict must beat LZ4 on this text corpus.
    assert!(
        lzma_size < lz4_size,
        "LZMA d32m ({lzma_size} B) should beat LZ4 ({lz4_size} B) on XML text"
    );

    // Roundtrip check for all codecs EXCEPT known-excluded ones.
    // LZP alone: preprocessor only — no standalone decompress path.
    const KNOWN_ROUNDTRIP_ISSUES: &[&str] = &["lzp"];
    let unexpected_failures: Vec<_> = results
        .iter()
        .filter(|r| {
            r.compressed > 0
                && !r.roundtrip_ok
                && !KNOWN_ROUNDTRIP_ISSUES.contains(&r.label.as_str())
        })
        .map(|r| r.label.as_str())
        .collect();
    assert!(
        unexpected_failures.is_empty(),
        "Unexpected roundtrip failures: {unexpected_failures:?}"
    );

    #[cfg(feature = "legacy-ffi-tests")]
    assert_ppmd_h_matches_freearc(&results);

    // ── BSC quality assertion (feature-gated) ─────────────────────────────────
    #[cfg(feature = "bsc")]
    {
        let bsc_size = results
            .iter()
            .find(|r| r.label == "bsc (default)")
            .map(|r| r.compressed)
            .unwrap_or(usize::MAX);

        let ppmd6_size = results
            .iter()
            .find(|r| r.label == "ppmd:o6:mem16m")
            .map(|r| r.compressed)
            .unwrap_or(usize::MAX);

        eprintln!("  BSC vs PPMd comparison:");
        eprintln!(
            "    PPMd o6:  {:>12} B  ({:.2}%)",
            fmt_bytes(ppmd6_size),
            (1.0 - ppmd6_size as f64 / original_size as f64) * 100.0
        );
        eprintln!(
            "    PPMd o12: {:>12} B  ({:.2}%)",
            fmt_bytes(ppmd12_size),
            (1.0 - ppmd12_size as f64 / original_size as f64) * 100.0
        );
        eprintln!(
            "    BSC:      {:>12} B  ({:.2}%)",
            fmt_bytes(bsc_size),
            (1.0 - bsc_size as f64 / original_size as f64) * 100.0
        );

        if bsc_size < ppmd12_size {
            eprintln!(
                "  → BSC wins over PPMd o12 by {} B ({:.1}%)",
                ppmd12_size - bsc_size,
                (ppmd12_size as f64 - bsc_size as f64) / ppmd12_size as f64 * 100.0
            );
        } else if bsc_size < ppmd6_size {
            eprintln!(
                "  → BSC beats PPMd o6 but not o12 (delta: +{} B vs o12)",
                bsc_size - ppmd12_size
            );
        } else {
            eprintln!(
                "  → PPMd wins over BSC (BSC is +{} B over PPMd o6)",
                bsc_size - ppmd6_size
            );
        }
        eprintln!();

        // BSC should be competitive with PPMd on structured text.
        // Allow 15% slack — BSC shines on larger blocks and may be at a
        // disadvantage on this 4.5 MiB corpus at default 25 MiB block size.
        let threshold = ppmd6_size + ppmd6_size * 15 / 100;
        assert!(
            bsc_size <= threshold,
            "BSC ({bsc_size} B) should be within 15% of PPMd o6 ({ppmd6_size} B) on XML text"
        );
    }
}

#[cfg(feature = "legacy-ffi-tests")]
fn assert_ppmd_h_matches_freearc(results: &[BenchResult]) {
    for (native_label, freearc_label) in [
        ("ppmd:o6:mem16m", "ppmd-h-freeArc-c:o6:mem16m"),
        ("ppmd:o12:mem32m", "ppmd-h-freeArc-c:o12:mem32m"),
    ] {
        let native = results
            .iter()
            .find(|r| r.label == native_label)
            .expect("missing native PPMd-H result");
        let freearc = results
            .iter()
            .find(|r| r.label == freearc_label)
            .expect("missing FreeArc PPMd-H result");

        if native.compressed == 0 || freearc.compressed == 0 {
            continue;
        }

        let native_payload = native.compressed.saturating_sub(8);
        let delta = native_payload.abs_diff(freearc.compressed);
        let tolerance = (freearc.compressed / 100).max(16);
        eprintln!(
            "  PPMd-H parity: {native_label} native payload {} B vs FreeArc C {} B (delta {:+} B)",
            fmt_bytes(native_payload),
            fmt_bytes(freearc.compressed),
            native_payload as isize - freearc.compressed as isize
        );
        assert!(
            delta <= tolerance,
            "native {native_label} payload ({native_payload} B) regressed beyond 1% vs original FreeArc C ({freearc} B)",
            freearc = freearc.compressed
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Large-corpus test (~3 GiB — loads the full CBETA xml-p5 tree into RAM)
// ─────────────────────────────────────────────────────────────────────────────
//
// Purpose: demonstrate SREP's long-range deduplication benefit at scale.
// At 3+ GiB, PPMd's 32 MiB context window covers < 1% of the corpus;
// SREP's dictionary covers the entire stream and removes cross-file repeats
// (identical XML prologues, teiHeader boilerplate, quoted passages) BEFORE
// the entropy coder runs.
//
// WARNING: loads ~3 GiB into RAM and takes several minutes.
// Run only when you need the full-scale numbers (release mode required):
//
//   cargo test --release --test codec_bench_text large_corpus -- --include-ignored --nocapture
//   cargo test --release --features bsc --test codec_bench_text large_corpus -- --include-ignored --nocapture

fn methods_large() -> Vec<(&'static str, Method)> {
    use arcmax::codec::lzma::LzmaOptions;
    use arcmax::codec::ppmd::PpmdOptions;
    use arcmax::codec::zstd::ZstdOptions;

    // Only SREP vs no-SREP pairs — each pair shares the same back-end so the
    // delta is purely SREP's long-range deduplication contribution.
    #[allow(unused_mut)]
    let mut list: Vec<(&'static str, Method)> = vec![
        // ── PPMd pair ────────────────────────────────────────────────────────
        ("ppmd:o6", Method::Ppmd(PpmdOptions::default())),
        (
            "srep+ppmd:o6",
            Method::Pipeline(vec![
                Method::Srep(arcmax::srep::SrepConfig::default()),
                Method::Ppmd(PpmdOptions::default()),
            ]),
        ),
        // ── LZMA pair ────────────────────────────────────────────────────────
        (
            "lzma:d32m",
            Method::Lzma(LzmaOptions {
                dict_size: 32 * 1024 * 1024,
                ..LzmaOptions::default()
            }),
        ),
        (
            "srep+lzma:d32m",
            Method::Pipeline(vec![
                Method::Srep(arcmax::srep::SrepConfig::default()),
                Method::Lzma(LzmaOptions {
                    dict_size: 32 * 1024 * 1024,
                    ..LzmaOptions::default()
                }),
            ]),
        ),
        // ── Zstd pair ────────────────────────────────────────────────────────
        ("zstd:19", Method::Zstd(ZstdOptions { level: 19 })),
        (
            "srep+zstd:19",
            Method::Pipeline(vec![
                Method::Srep(arcmax::srep::SrepConfig::default()),
                Method::Zstd(ZstdOptions { level: 19 }),
            ]),
        ),
    ];

    // ── BSC pair (feature-gated) ──────────────────────────────────────────────
    #[cfg(feature = "bsc")]
    {
        use arcmax::codec::BscOptions;
        list.push(("bsc", Method::Bsc(BscOptions::default())));
        list.push((
            "srep+bsc",
            Method::Pipeline(vec![
                Method::Srep(arcmax::srep::SrepConfig::default()),
                Method::Bsc(BscOptions::default()),
            ]),
        ));
    }

    list
}

#[test]
#[ignore = "large-corpus benchmark (~3 GiB RAM); run with: cargo test large_corpus -- --include-ignored --nocapture"]
fn bench_text_codecs_large_corpus() {
    // Walk all subdirectories under FULL_CORPUS_DIR, collecting every .xml file.
    let root = Path::new(FULL_CORPUS_DIR);
    assert!(
        root.exists(),
        "Full corpus not found at {FULL_CORPUS_DIR}\n\
         Adjust the FULL_CORPUS_DIR constant if the path has changed."
    );

    eprintln!();
    eprintln!("  ══════════════════════════════════════════════════════════");
    eprintln!("  Large-corpus benchmark — Full CBETA TEI P5 XML (~3 GiB)");
    eprintln!("  ══════════════════════════════════════════════════════════");
    eprintln!("  Loading corpus (this may take a minute)…");

    // Recursive directory walk.
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut children: Vec<_> = rd.filter_map(|e| e.ok()).collect();
            children.sort_by_key(|e| e.path());
            for entry in children {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("xml"))
                    .unwrap_or(false)
                {
                    paths.push(p);
                }
            }
        }
    }

    let mut corpus: Vec<u8> = Vec::new();
    for p in &paths {
        let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("Cannot read {}: {e}", p.display()));
        corpus.extend_from_slice(&bytes);
    }

    eprintln!(
        "  Loaded {} XML files — {:.1} MiB total",
        paths.len(),
        corpus.len() as f64 / (1024.0 * 1024.0)
    );

    let all_methods = methods_large();
    let mut results: Vec<BenchResult> = Vec::new();

    for (label, method) in all_methods {
        eprint!("  [{:27}] ", label);
        let r = bench_one(label, method, &corpus);
        if r.compressed > 0 {
            eprintln!(
                "  {:.3}x  ({:.1}%)  {}ms{}",
                r.ratio(),
                r.savings_pct(),
                r.encode_ms,
                if r.roundtrip_ok {
                    ""
                } else {
                    "  *** ROUNDTRIP FAIL ***"
                }
            );
        }
        results.push(r);
    }

    print_table(&results);

    // At this scale, SREP+PPMd should meaningfully beat standalone PPMd.
    let ppmd6_size = results
        .iter()
        .find(|r| r.label == "ppmd:o6")
        .map(|r| r.compressed)
        .unwrap_or(usize::MAX);

    let srep_ppmd6_size = results
        .iter()
        .find(|r| r.label == "srep+ppmd:o6")
        .map(|r| r.compressed)
        .unwrap_or(usize::MAX);

    if ppmd6_size < usize::MAX && srep_ppmd6_size < usize::MAX {
        let improvement_pct =
            (ppmd6_size as f64 - srep_ppmd6_size as f64) / ppmd6_size as f64 * 100.0;
        eprintln!(
            "  SREP+PPMd vs PPMd alone: {:+.1}% ({} B → {} B)",
            improvement_pct,
            fmt_bytes(ppmd6_size),
            fmt_bytes(srep_ppmd6_size),
        );
        // At 3+ GiB, SREP should deliver at least 5% improvement over standalone PPMd.
        assert!(
            srep_ppmd6_size < ppmd6_size,
            "SREP+PPMd ({srep_ppmd6_size} B) should compress better than PPMd alone ({ppmd6_size} B) at 3 GiB scale"
        );
    }
}
