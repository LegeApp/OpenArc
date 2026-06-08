//! Long-running real-EXE compression benchmark.
//!
//! Goals:
//! - Compare max-compression LZMA2, PPMd-H, and Tornado with and without Dispack.
//! - Keep dictionary/window sizes tied to available memory instead of fixed presets.
//! - Print progress often enough for hour-scale runs.
//!
//! Run:
//! ```sh
//! cargo test --release --test exe_compression_bench -- --ignored --nocapture
//! ```
//!
//! Useful overrides:
//! - `ARCMAX_EXE_BENCH_PATH=D:\path\app.exe`
//! - `ARCMAX_EXE_BENCH_MEMORY_MIB=12000`
//! - `ARCMAX_EXE_BENCH_PROGRESS_MIB=16`
//! - `ARCMAX_EXE_BENCH_ROUNDTRIP=0`
//! - `ARCMAX_EXE_BENCH_STRICT=1`
//! - `ARCMAX_EXE_BENCH_7Z_DICT_MIB=128`
//! - `ARCMAX_EXE_BENCH_7Z_WORD=273`

use std::env;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use arcmax::codec::filters::DispackOptions;
use arcmax::codec::lzma::LzmaOptions;
use arcmax::codec::ppmd::{PpmdOptions, PpmdVariant};
use arcmax::codec::tornado::TornadoOptions;
use arcmax::method::pipeline::CodecPipeline;
use arcmax::method::Method;
use arcmax::Result;

const DEFAULT_EXE_PATH: &str = r"D:\Rust-projects\SinoRAG-runtime\sinorag.exe";
const MIB: usize = 1024 * 1024;

#[derive(Clone)]
struct Candidate {
    label: String,
    profile: String,
    method: Method,
    backend: Backend,
    uses_dispack: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Lzma2,
    PpmdH,
    Tornado,
}

struct BenchResult {
    label: String,
    profile: String,
    backend: Backend,
    uses_dispack: bool,
    compressed: usize,
    encode: Duration,
    decode: Option<Duration>,
    roundtrip_error: Option<String>,
}

#[test]
#[ignore = "long-running real-EXE benchmark; run explicitly with --ignored --nocapture"]
fn exe_compression_quality_speed_matrix() {
    let path = exe_path();
    if !path.exists() {
        eprintln!(
            "SKIP: input exe not found: {}\nSet ARCMAX_EXE_BENCH_PATH to a real .exe.",
            path.display()
        );
        return;
    }

    let progress_mib = env_usize("ARCMAX_EXE_BENCH_PROGRESS_MIB")
        .unwrap_or(16)
        .max(1);
    let data = read_file_with_progress(&path, progress_mib * MIB);
    let original = data.len();
    assert!(original > 0, "input exe is empty: {}", path.display());

    let available_mib = env_usize("ARCMAX_EXE_BENCH_MEMORY_MIB")
        .or_else(available_memory_mib)
        .unwrap_or(4096);
    let budget_mib = (available_mib * 70 / 100).max(256);
    let sizing = AdaptiveSizing::new(original, budget_mib);

    eprintln!();
    eprintln!("=== EXE compression benchmark ===");
    eprintln!("input: {}", path.display());
    eprintln!(
        "size: {} ({:.2} MiB)",
        fmt_bytes(original),
        original as f64 / MIB as f64
    );
    eprintln!(
        "memory: available={} MiB, benchmark budget={} MiB",
        available_mib, budget_mib
    );
    eprintln!(
        "adaptive: lzma2_dict={} MiB, ppmd_mem={} MiB, tornado_window={} MiB",
        sizing.lzma2_dict_mib, sizing.ppmd_mem_mib, sizing.tornado_window_mib
    );
    let seven_zip = SevenZipLzmaProfile::from_env();
    eprintln!(
        "7z ultra profile: lzma2_dict={} MiB, word/nice_len={}",
        seven_zip.dict_mib, seven_zip.nice_len
    );
    eprintln!(
        "adaptive note: LZMA2/Tornado windows are capped at the useful whole-file window; \
         larger windows cannot find longer-distance matches in a single file."
    );
    eprintln!(
        "roundtrip: {}",
        if roundtrip_enabled() {
            "enabled"
        } else {
            "disabled"
        }
    );
    eprintln!(
        "strict failures: {}",
        if strict_enabled() {
            "enabled"
        } else {
            "disabled"
        }
    );
    eprintln!();

    let candidates = candidates(&sizing);
    let progress = ProgressHeartbeat::start();
    let mut results = Vec::new();

    for (idx, candidate) in candidates.iter().enumerate() {
        progress.set(format!(
            "{}/{} {}",
            idx + 1,
            candidates.len(),
            candidate.label
        ));
        eprintln!(
            "[candidate {}/{}] begin {}",
            idx + 1,
            candidates.len(),
            candidate.label
        );
        eprintln!(
            "[candidate {}/{}] whole-file block 1/1: {:.2} MiB",
            idx + 1,
            candidates.len(),
            original as f64 / MIB as f64
        );

        let started = Instant::now();
        let compressed = compress_whole_file(candidate.method.clone(), &data);
        let encode = started.elapsed();
        let compressed_len = compressed.len();
        eprintln!(
            "[candidate {}/{}] compressed {} ({:.3}x, {:.2}%) in {:.2}s",
            idx + 1,
            candidates.len(),
            fmt_bytes(compressed_len),
            compressed_len as f64 / original as f64,
            compressed_len as f64 / original as f64 * 100.0,
            encode.as_secs_f64()
        );

        let mut roundtrip_error = None;
        let decode = if roundtrip_enabled() {
            let decode_started = Instant::now();
            let decoded = decompress_whole_file(candidate.method.clone(), &compressed);
            let elapsed = decode_started.elapsed();
            if let Err(err) = decoded {
                let summary = format!("decompression error: {err}");
                eprintln!(
                    "[candidate {}/{}] ROUNDTRIP FAIL in {:.2}s: {}",
                    idx + 1,
                    candidates.len(),
                    elapsed.as_secs_f64(),
                    summary
                );
                roundtrip_error = Some(summary);
            } else if let Some(summary) = first_mismatch(&decoded.unwrap(), &data) {
                eprintln!(
                    "[candidate {}/{}] ROUNDTRIP FAIL in {:.2}s: {}",
                    idx + 1,
                    candidates.len(),
                    elapsed.as_secs_f64(),
                    summary
                );
                roundtrip_error = Some(summary);
            } else {
                eprintln!(
                    "[candidate {}/{}] roundtrip OK in {:.2}s",
                    idx + 1,
                    candidates.len(),
                    elapsed.as_secs_f64()
                );
            }
            Some(elapsed)
        } else {
            None
        };

        results.push(BenchResult {
            label: candidate.label.clone(),
            profile: candidate.profile.clone(),
            backend: candidate.backend,
            uses_dispack: candidate.uses_dispack,
            compressed: compressed_len,
            encode,
            decode,
            roundtrip_error,
        });
    }

    progress.stop();
    print_summary(original, &results);

    let failures: Vec<_> = results
        .iter()
        .filter_map(|r| {
            r.roundtrip_error
                .as_ref()
                .map(|err| format!("{}: {}", r.label, err))
        })
        .collect();
    if !failures.is_empty() {
        eprintln!();
        eprintln!("invalid candidates:");
        for failure in &failures {
            eprintln!("  {failure}");
        }
        assert!(
            !strict_enabled(),
            "roundtrip failures:\n{}",
            failures.join("\n")
        );
    }
}

fn candidates(sizing: &AdaptiveSizing) -> Vec<Candidate> {
    let lzma = lzma2_method(sizing.lzma2_dict_mib, None);
    let seven_zip = SevenZipLzmaProfile::from_env();
    let seven_zip_lzma = lzma2_method(seven_zip.dict_mib, Some(seven_zip.nice_len));
    let ppmd = Method::Ppmd(PpmdOptions {
        order: 16,
        memory_size: sizing.ppmd_mem_mib * MIB,
        variant: PpmdVariant::H,
    });
    let mut tornado_opts = TornadoOptions::preset(16).expect("tornado:16 preset is valid");
    tornado_opts.buffer_size = sizing.tornado_window_mib * MIB;
    let tornado = Method::Tornado(tornado_opts);

    let seven_zip_label = format!(
        "lzma2:7z-ultra:d{}m:fb{}",
        seven_zip.dict_mib, seven_zip.nice_len
    );
    let plain = vec![
        ("lzma2:max".to_string(), "lzma2:max", lzma, Backend::Lzma2),
        (
            seven_zip_label,
            "lzma2:7z-ultra",
            seven_zip_lzma,
            Backend::Lzma2,
        ),
        ("ppmd-h:o16".to_string(), "ppmd-h:o16", ppmd, Backend::PpmdH),
        (
            "tornado:16".to_string(),
            "tornado:16",
            tornado,
            Backend::Tornado,
        ),
    ];

    let mut out = Vec::new();
    for (label, profile, method, backend) in plain {
        out.push(Candidate {
            label: label.clone(),
            profile: profile.to_string(),
            method: method.clone(),
            backend,
            uses_dispack: false,
        });
        out.push(Candidate {
            label: format!("dispack+{label}"),
            profile: profile.to_string(),
            method: Method::Pipeline(vec![Method::Dispack(DispackOptions::default()), method]),
            backend,
            uses_dispack: true,
        });
    }
    out
}

fn lzma2_method(dict_mib: usize, nice_len: Option<u32>) -> Method {
    Method::Lzma(LzmaOptions {
        level: Some(9),
        dict_size: (dict_mib * MIB) as u32,
        lc: 3,
        lp: 0,
        pb: 2,
        nice_len,
        lzma2: true,
    })
}

struct AdaptiveSizing {
    lzma2_dict_mib: usize,
    ppmd_mem_mib: usize,
    tornado_window_mib: usize,
}

#[derive(Clone, Copy)]
struct SevenZipLzmaProfile {
    dict_mib: usize,
    nice_len: u32,
}

impl SevenZipLzmaProfile {
    fn from_env() -> Self {
        Self {
            dict_mib: env_usize("ARCMAX_EXE_BENCH_7Z_DICT_MIB")
                .unwrap_or(128)
                .max(1),
            nice_len: env_usize("ARCMAX_EXE_BENCH_7Z_WORD")
                .unwrap_or(273)
                .clamp(LZMA_NICE_LEN_MIN as usize, LZMA_NICE_LEN_MAX as usize)
                as u32,
        }
    }
}

const LZMA_NICE_LEN_MIN: u32 = 8;
const LZMA_NICE_LEN_MAX: u32 = 273;

impl AdaptiveSizing {
    fn new(input_len: usize, budget_mib: usize) -> Self {
        let input_mib = input_len.div_ceil(MIB).max(1);
        let useful_window_mib = input_mib.next_power_of_two().max(16);

        // LZMA2 encoder memory is roughly several times dictionary size. Keep
        // the default aggressive, but cap it to the useful whole-file window.
        // lzma-rust2 also asserts internally with dictionaries far larger than
        // the input, while such windows cannot improve a one-file benchmark.
        let lzma2_dict_mib = pow2_floor((budget_mib / 3).max(64))
            .min(1536)
            .min(useful_window_mib);

        // PPMd-H uses the model memory directly; leave room for buffers and OS.
        let ppmd_mem_mib = pow2_floor((budget_mib / 2).max(64)).min(2048);

        // Tornado level 16 allocates binary-tree state proportional to window.
        // It is much more memory-expensive than LZMA2 per dictionary byte.
        let tornado_window_mib = pow2_floor((budget_mib / 10).max(16))
            .min(1024)
            .min(useful_window_mib);

        Self {
            lzma2_dict_mib,
            ppmd_mem_mib,
            tornado_window_mib,
        }
    }
}

fn compress_whole_file(method: Method, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 4);
    CodecPipeline::new(method)
        .with_block_size(data.len().max(1))
        .compress(Cursor::new(data), &mut out)
        .expect("compression failed");
    out
}

fn decompress_whole_file(method: Method, compressed: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    CodecPipeline::new(method).decompress(Cursor::new(compressed), &mut out)?;
    Ok(out)
}

fn print_summary(original: usize, results: &[BenchResult]) {
    let mut sorted: Vec<&BenchResult> = results.iter().collect();
    sorted.sort_by_key(|r| r.compressed);
    let mut valid: Vec<&BenchResult> = results
        .iter()
        .filter(|r| r.roundtrip_error.is_none())
        .collect();
    valid.sort_by_key(|r| r.compressed);

    eprintln!();
    eprintln!(
        "{:<38} {:>14} {:>9} {:>11} {:>11} status",
        "candidate", "compressed", "ratio", "enc sec", "dec sec"
    );
    eprintln!("{}", "-".repeat(90));
    for r in &sorted {
        eprintln!(
            "{:<38} {:>14} {:>8.3}x {:>10.2} {:>11} {}",
            r.label,
            fmt_bytes(r.compressed),
            r.compressed as f64 / original as f64,
            r.encode.as_secs_f64(),
            r.decode
                .map(|d| format!("{:.2}", d.as_secs_f64()))
                .unwrap_or_else(|| "skipped".to_string()),
            if r.roundtrip_error.is_some() {
                "ROUNDTRIP_FAIL"
            } else {
                ""
            }
        );
    }

    eprintln!();
    for plain in results.iter().filter(|r| !r.uses_dispack) {
        let filtered = results
            .iter()
            .find(|r| r.backend == plain.backend && r.profile == plain.profile && r.uses_dispack);
        if let Some(filtered) = filtered {
            let delta = filtered.compressed as i64 - plain.compressed as i64;
            let pct = delta as f64 / plain.compressed as f64 * 100.0;
            if filtered.roundtrip_error.is_some() {
                eprintln!(
                    "dispack delta for {}: {:+} bytes ({:+.2}%) but INVALID roundtrip",
                    plain.profile, delta, pct
                );
            } else if plain.roundtrip_error.is_some() {
                eprintln!(
                    "dispack delta for {}: cannot compare; plain baseline failed roundtrip",
                    plain.profile
                );
            } else {
                eprintln!(
                    "dispack delta for {}: {:+} bytes ({:+.2}%)",
                    plain.profile, delta, pct
                );
            }
        }
    }

    let lzma2 = results
        .iter()
        .find(|r| r.backend == Backend::Lzma2 && r.profile == "lzma2:max" && !r.uses_dispack)
        .expect("missing lzma2 baseline");
    let best = valid.first().expect("no valid benchmark results");
    eprintln!(
        "best valid: {} at {} ({:.3}x)",
        best.label,
        fmt_bytes(best.compressed),
        best.compressed as f64 / original as f64
    );
    eprintln!(
        "best valid vs plain LZMA2: {:+} bytes ({:+.2}%)",
        best.compressed as i64 - lzma2.compressed as i64,
        (best.compressed as f64 - lzma2.compressed as f64) / lzma2.compressed as f64 * 100.0
    );
}

struct ProgressHeartbeat {
    done: Arc<AtomicBool>,
    label: Arc<Mutex<String>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ProgressHeartbeat {
    fn start() -> Self {
        let done = Arc::new(AtomicBool::new(false));
        let label = Arc::new(Mutex::new(String::from("starting")));
        let thread_done = done.clone();
        let thread_label = label.clone();
        let handle = thread::spawn(move || {
            let started = Instant::now();
            while !thread_done.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(30));
                if thread_done.load(Ordering::Relaxed) {
                    break;
                }
                let label = thread_label.lock().map(|s| s.clone()).unwrap_or_default();
                eprintln!(
                    "[progress] elapsed {:.1} min; running {}",
                    started.elapsed().as_secs_f64() / 60.0,
                    label
                );
            }
        });
        Self {
            done,
            label,
            handle: Some(handle),
        }
    }

    fn set(&self, label: String) {
        if let Ok(mut slot) = self.label.lock() {
            *slot = label;
        }
    }

    fn stop(mut self) {
        self.done.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn read_file_with_progress(path: &Path, chunk_size: usize) -> Vec<u8> {
    let total = std::fs::metadata(path)
        .map(|m| m.len() as usize)
        .expect("metadata failed");
    let chunks = total.div_ceil(chunk_size).max(1);
    let mut file = File::open(path).expect("open exe");
    let mut data = Vec::with_capacity(total);
    let mut buf = vec![0u8; chunk_size];
    let mut chunk = 0usize;
    loop {
        let n = file.read(&mut buf).expect("read exe chunk");
        if n == 0 {
            break;
        }
        chunk += 1;
        data.extend_from_slice(&buf[..n]);
        eprintln!(
            "[load chunk {}/{}] {} / {} ({:.1}%)",
            chunk,
            chunks,
            fmt_bytes(data.len()),
            fmt_bytes(total),
            data.len() as f64 / total as f64 * 100.0
        );
    }
    data
}

fn first_mismatch(actual: &[u8], expected: &[u8]) -> Option<String> {
    let common = actual.len().min(expected.len());
    for i in 0..common {
        if actual[i] != expected[i] {
            let start = i.saturating_sub(8);
            let end = (i + 8).min(common);
            return Some(format!(
                "first mismatch at byte {i}: decoded=0x{:02x}, expected=0x{:02x}, decoded_window={:02x?}, expected_window={:02x?}",
                actual[i],
                expected[i],
                &actual[start..end],
                &expected[start..end]
            ));
        }
    }

    if actual.len() != expected.len() {
        Some(format!(
            "length mismatch: decoded={} bytes, expected={} bytes",
            actual.len(),
            expected.len()
        ))
    } else {
        None
    }
}

fn exe_path() -> PathBuf {
    env::var_os("ARCMAX_EXE_BENCH_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_EXE_PATH))
}

fn roundtrip_enabled() -> bool {
    env::var("ARCMAX_EXE_BENCH_ROUNDTRIP")
        .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}

fn strict_enabled() -> bool {
    env::var("ARCMAX_EXE_BENCH_STRICT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn env_usize(name: &str) -> Option<usize> {
    env::var(name).ok()?.parse().ok()
}

fn pow2_floor(n: usize) -> usize {
    if n <= 1 {
        1
    } else {
        1usize << (usize::BITS - 1 - n.leading_zeros())
    }
}

fn fmt_bytes(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

#[cfg(target_os = "linux")]
fn available_memory_mib() -> Option<usize> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            let kb = rest.split_whitespace().next()?.parse::<usize>().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn available_memory_mib() -> Option<usize> {
    #[repr(C)]
    struct MemoryStatusEx {
        dw_length: u32,
        dw_memory_load: u32,
        ull_total_phys: u64,
        ull_avail_phys: u64,
        ull_total_page_file: u64,
        ull_avail_page_file: u64,
        ull_total_virtual: u64,
        ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }

    extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }

    let mut status = MemoryStatusEx {
        dw_length: std::mem::size_of::<MemoryStatusEx>() as u32,
        dw_memory_load: 0,
        ull_total_phys: 0,
        ull_avail_phys: 0,
        ull_total_page_file: 0,
        ull_avail_page_file: 0,
        ull_total_virtual: 0,
        ull_avail_virtual: 0,
        ull_avail_extended_virtual: 0,
    };

    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok == 0 {
        None
    } else {
        Some((status.ull_avail_phys as usize) / MIB)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn available_memory_mib() -> Option<usize> {
    None
}
