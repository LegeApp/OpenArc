//! arcmax CLI — command dispatch and interactive wizard.

use anyhow::{bail, Context, Result};
use arcmax::archive::{ArchiveReader, ArchiveWriter, ExtractProgress, WriteProgress};
use arcmax::codec::ZstdOptions;
use arcmax::Method;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

// ============================================================================
// Color + terminal helpers
// ============================================================================

struct ColorConfig {
    pub dim_blue: &'static str,
    pub bright_green: &'static str,
    pub bright_cyan: &'static str,
    pub bright_yellow: &'static str,
    pub dim_cyan: &'static str,
    pub reset: &'static str,
}

const COLORS: ColorConfig = ColorConfig {
    dim_blue: "\x1b[2;34m",
    bright_green: "\x1b[92m",
    bright_cyan: "\x1b[96m",
    bright_yellow: "\x1b[93m",
    dim_cyan: "\x1b[2;36m",
    reset: "\x1b[0m",
};

fn timestamp() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs();
    let h = (total_secs % 86400) / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("[{h:02}:{m:02}:{s:02}]")
}

fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let ms = d.subsec_millis();
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{h}h {m:02}m {s:02}s")
    } else if m > 0 {
        format!("{m:02}m {s:02}s")
    } else if ms > 0 {
        format!("{s:02}.{ms:03}s")
    } else {
        format!("{s:02}s")
    }
}

fn format_bytes(n: u64) -> String {
    if n >= 1 << 30 {
        format!("{:.2} GiB", n as f64 / (1u64 << 30) as f64)
    } else if n >= 1 << 20 {
        format!("{:.1} MiB", n as f64 / (1u64 << 20) as f64)
    } else if n >= 1 << 10 {
        format!("{:.1} KiB", n as f64 / (1u64 << 10) as f64)
    } else {
        format!("{n} B")
    }
}

// ============================================================================
// 3-line progress renderer (sync, channel-driven)
// ============================================================================

enum ProgressEvent {
    Writing {
        block_id: u32,
        total_blocks_hint: u32,
        files_in_block: usize,
        ratio: f64,
        bytes_done: u64,
        total_bytes: u64,
    },
    Extracting {
        file_index: usize,
        total_files: usize,
        path: String,
        bytes_done: u64,
        total_bytes: u64,
        block_id: u32,
        total_blocks: u32,
    },
    Done {
        elapsed: Duration,
        summary: String,
    },
    Error(String),
}

/// Spawn a thread that reads `ProgressEvent`s and redraws a 3-line status block.
/// Returns a sender for progress events and a join handle.
fn spawn_progress_renderer() -> (mpsc::SyncSender<ProgressEvent>, std::thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::sync_channel::<ProgressEvent>(64);
    let handle = std::thread::spawn(move || {
        run_renderer(rx);
    });
    (tx, handle)
}

fn run_renderer(rx: mpsc::Receiver<ProgressEvent>) {
    struct CursorGuard;
    impl Drop for CursorGuard {
        fn drop(&mut self) {
            print!("\x1b[?25h");
            let _ = io::stdout().flush();
        }
    }
    let _cg = CursorGuard;
    print!("\x1b[?25l");
    println!();
    println!();
    println!(); // reserve 3 lines
    let _ = io::stdout().flush();

    let mut last = ("".to_string(), "".to_string(), "".to_string());

    loop {
        let event = match rx.recv() {
            Ok(e) => e,
            Err(_) => break,
        };

        let (l0, l1, l2, color) = match event {
            ProgressEvent::Writing {
                block_id,
                total_blocks_hint,
                files_in_block,
                ratio,
                bytes_done,
                total_bytes,
            } => {
                let pct = if total_bytes > 0 {
                    bytes_done * 100 / total_bytes
                } else {
                    0
                };
                let block_disp = if total_blocks_hint > 0 {
                    format!("{}/{}", block_id + 1, total_blocks_hint)
                } else {
                    format!("{}", block_id + 1)
                };
                (
                    format!("Creating archive — block {block_disp} ({files_in_block} files)"),
                    format!(
                        "{} / {} ({pct}%)",
                        format_bytes(bytes_done),
                        format_bytes(total_bytes)
                    ),
                    format!("Compression ratio: {:.1}%", ratio * 100.0),
                    COLORS.bright_cyan,
                )
            }
            ProgressEvent::Extracting {
                file_index,
                total_files,
                path,
                bytes_done,
                total_bytes,
                block_id,
                total_blocks,
            } => {
                let pct = if total_bytes > 0 {
                    bytes_done * 100 / total_bytes
                } else {
                    0
                };
                (
                    format!(
                        "Extracting — file {}/{total_files} (block {}/{total_blocks})",
                        file_index + 1,
                        block_id + 1
                    ),
                    format!(
                        "{} / {} ({pct}%)",
                        format_bytes(bytes_done),
                        format_bytes(total_bytes)
                    ),
                    path,
                    COLORS.bright_cyan,
                )
            }
            ProgressEvent::Done { elapsed, summary } => {
                let ts = timestamp();
                print!("\x1b[3F\x1b[0J");
                println!("{ts} {}Status{} Done", COLORS.bright_green, COLORS.reset);
                println!(
                    "{ts} {}Detail{} {}",
                    COLORS.bright_green, COLORS.reset, summary
                );
                println!(
                    "{ts} {}Detail{} Completed in {}.",
                    COLORS.bright_green,
                    COLORS.reset,
                    format_duration(elapsed)
                );
                let _ = io::stdout().flush();
                break;
            }
            ProgressEvent::Error(msg) => {
                let ts = timestamp();
                print!("\x1b[3F\x1b[0J");
                println!("{ts} {}Status{} Error", COLORS.dim_blue, COLORS.reset);
                println!("{ts} {}Detail{} {msg}", COLORS.dim_cyan, COLORS.reset);
                println!(
                    "{ts} {}Detail{} Operation failed.",
                    COLORS.dim_cyan, COLORS.reset
                );
                let _ = io::stdout().flush();
                break;
            }
        };

        // Only redraw when lines actually changed.
        let new = (l0, l1, l2);
        if (new.0.as_str(), new.1.as_str(), new.2.as_str())
            != (last.0.as_str(), last.1.as_str(), last.2.as_str())
        {
            let ts = timestamp();
            print!("\x1b[3F\x1b[0J");
            println!("{ts} {color}Status{} {}", COLORS.reset, new.0);
            println!(
                "{ts} {color}Detail{} {}{}{}",
                COLORS.reset, color, new.1, COLORS.reset
            );
            println!("{ts} {}Detail{} {}", COLORS.dim_cyan, COLORS.reset, new.2);
            let _ = io::stdout().flush();
            last = new;
        }
    }
}

// ============================================================================
// Argument parsing
// ============================================================================

struct CreateOpts {
    output: PathBuf,
    inputs: Vec<PathBuf>,
    method: Method,
    block_threshold: usize,
}

struct ExtractOpts {
    input: PathBuf,
    dest: PathBuf,
}

struct ListOpts {
    input: PathBuf,
}

struct VerifyOpts {
    input: PathBuf,
}

enum Command {
    Create(CreateOpts),
    Extract(ExtractOpts),
    List(ListOpts),
    Verify(VerifyOpts),
    Interactive,
    Help,
    Version,
}

fn parse_args(args: &[String]) -> Result<Command> {
    if args.len() <= 1 {
        return Ok(Command::Interactive);
    }

    match args[1].as_str() {
        "--help" | "-h" | "help" => return Ok(Command::Help),
        "--version" | "-V" | "version" => return Ok(Command::Version),
        _ => {}
    }

    let sub = args[1].as_str();
    let rest = &args[2..];

    match sub {
        "create" | "c" => parse_create(rest),
        "extract" | "x" | "e" => parse_extract(rest),
        "list" | "l" | "t" => parse_list(rest),
        "verify" | "v" => parse_verify(rest),
        _ => bail!(
            "Unknown subcommand '{}'. Use: create, extract, list, verify, or run without args for interactive mode.",
            sub
        ),
    }
}

fn parse_create(args: &[String]) -> Result<Command> {
    if args.is_empty() {
        bail!("create requires an output path.\nUsage: arcmax create <output.arc> <path...> [--method zstd] [--level 22] [--block 268435456]");
    }

    let mut output: Option<PathBuf> = None;
    let mut inputs: Vec<PathBuf> = Vec::new();
    let mut method_str = "zstd".to_string();
    let mut level: i32 = 22;
    let mut block_threshold: usize = 256 * 1024 * 1024;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--method" | "-m" => {
                i += 1;
                method_str = args
                    .get(i)
                    .ok_or_else(|| anyhow::anyhow!("Missing value after --method"))?
                    .clone();
            }
            "--level" | "-l" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| anyhow::anyhow!("Missing value after --level"))?;
                level = v
                    .parse()
                    .with_context(|| format!("Invalid --level '{v}'"))?;
            }
            "--block" | "-b" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| anyhow::anyhow!("Missing value after --block"))?;
                block_threshold =
                    parse_size(v).with_context(|| format!("Invalid --block '{v}'"))?;
            }
            arg if !arg.starts_with('-') => {
                if output.is_none() {
                    output = Some(PathBuf::from(arg));
                } else {
                    inputs.push(PathBuf::from(arg));
                }
            }
            other => bail!("Unknown flag '{other}' for create"),
        }
        i += 1;
    }

    let output = output.ok_or_else(|| anyhow::anyhow!("Missing output path for create"))?;
    if inputs.is_empty() {
        bail!("create requires at least one input path");
    }

    let method = build_method(&method_str, level)?;
    Ok(Command::Create(CreateOpts {
        output,
        inputs,
        method,
        block_threshold,
    }))
}

fn parse_extract(args: &[String]) -> Result<Command> {
    if args.is_empty() {
        bail!("extract requires an archive path.\nUsage: arcmax extract <input.arc> [dest-dir]");
    }
    let input = PathBuf::from(&args[0]);
    let dest = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(Command::Extract(ExtractOpts { input, dest }))
}

fn parse_list(args: &[String]) -> Result<Command> {
    if args.is_empty() {
        bail!("list requires an archive path.\nUsage: arcmax list <input.arc>");
    }
    Ok(Command::List(ListOpts {
        input: PathBuf::from(&args[0]),
    }))
}

fn parse_verify(args: &[String]) -> Result<Command> {
    if args.is_empty() {
        bail!("verify requires an archive path.\nUsage: arcmax verify <input.arc>");
    }
    Ok(Command::Verify(VerifyOpts {
        input: PathBuf::from(&args[0]),
    }))
}

/// Parse a size string: accepts plain integers or suffixes K/M/G (case-insensitive).
fn parse_size(s: &str) -> Result<usize> {
    let s = s.trim();
    let (digits, suffix) = s
        .find(|c: char| c.is_ascii_alphabetic())
        .map(|i| (&s[..i], &s[i..]))
        .unwrap_or((s, ""));
    let n: u64 = digits
        .parse()
        .with_context(|| format!("Not a number: '{digits}'"))?;
    let mul: u64 = match suffix.to_ascii_uppercase().as_str() {
        "" => 1,
        "K" | "KB" | "KIB" => 1 << 10,
        "M" | "MB" | "MIB" => 1 << 20,
        "G" | "GB" | "GIB" => 1 << 30,
        other => bail!("Unknown size suffix '{other}'. Use K, M, or G."),
    };
    Ok((n * mul) as usize)
}

fn build_method(name: &str, level: i32) -> Result<Method> {
    // Handle the common cases with level injection, then fall through to
    // the full method string parser so any codec (tornado, lzma, ppmd, …)
    // works without having to enumerate them here.
    match name.trim().to_ascii_lowercase().as_str() {
        "store" => return Ok(Method::Store),
        "lz4" => return Ok(Method::Lz4(arcmax::codec::Lz4Options::default())),
        "zstd" => return Ok(Method::Zstd(ZstdOptions { level })),
        _ => {}
    }
    // Delegate to the full parser (supports tornado, lzma, ppmd, brotli,
    // pipeline chains like "lz4+zstd:3", etc.)
    name.parse::<Method>()
        .map_err(|e| anyhow::anyhow!("Unknown method '{}': {}", name, e))
}

// ============================================================================
// Interactive wizard
// ============================================================================

fn interactive_wizard() -> Result<()> {
    println!("arcmax — FreeARC-compatible archive utility");
    println!();
    println!("Operations:");
    println!("  1) create   — pack files/directories into a new .arc archive");
    println!("  2) extract  — unpack an archive to disk");
    println!("  3) list     — display archive contents");
    println!("  4) verify   — verify archive integrity");
    println!();
    print!("Select operation [1-4]: ");
    io::stdout().flush()?;

    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let choice = line.trim();

    match choice {
        "1" | "create" | "c" => wizard_create(),
        "2" | "extract" | "x" | "e" => wizard_extract(),
        "3" | "list" | "l" | "t" => wizard_list(),
        "4" | "verify" | "v" => wizard_verify(),
        "" => {
            println!("No operation selected. Exiting.");
            Ok(())
        }
        other => bail!("Unknown selection '{other}'"),
    }
}

fn prompt(msg: &str) -> Result<String> {
    print!("{msg}");
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

fn wizard_create() -> Result<()> {
    let output_str = prompt("Output archive path [output.arc]: ")?;
    let output = PathBuf::from(if output_str.is_empty() {
        "output.arc".to_string()
    } else {
        output_str
    });

    let inputs_str = prompt("Input paths (space-separated): ")?;
    let inputs: Vec<PathBuf> = inputs_str.split_whitespace().map(PathBuf::from).collect();
    if inputs.is_empty() {
        bail!("No input paths provided");
    }

    let method_str = prompt("Compression method [zstd/lz4/tornado/store, default zstd]: ")?;
    let method_str = if method_str.is_empty() {
        "zstd".to_string()
    } else {
        method_str
    };

    let level_hint = match method_str
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "zstd" => "1-22, default 22",
        "tornado" => "0-16, default 5",
        "lz4" | "store" => "",
        _ => "codec-specific",
    };
    let level = if level_hint.is_empty() {
        // method has no level concept
        0
    } else {
        let l = prompt(&format!("Compression level [{level_hint}]: "))?;
        if l.is_empty() {
            match method_str
                .split(':')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
                .as_str()
            {
                "tornado" => 5,
                _ => 22,
            }
        } else {
            l.parse::<i32>()
                .with_context(|| format!("Invalid level '{l}'"))?
        }
    };

    let method = build_method(&method_str, level)?;
    cmd_create(CreateOpts {
        output,
        inputs,
        method,
        block_threshold: 256 * 1024 * 1024,
    })
}

fn wizard_extract() -> Result<()> {
    let input_str = prompt("Archive path: ")?;
    if input_str.is_empty() {
        bail!("No archive path provided");
    }
    let dest_str = prompt("Destination directory [. (current dir)]: ")?;
    let dest = PathBuf::from(if dest_str.is_empty() {
        ".".to_string()
    } else {
        dest_str
    });
    cmd_extract(ExtractOpts {
        input: PathBuf::from(input_str),
        dest,
    })
}

fn wizard_list() -> Result<()> {
    let input_str = prompt("Archive path: ")?;
    if input_str.is_empty() {
        bail!("No archive path provided");
    }
    cmd_list(ListOpts {
        input: PathBuf::from(input_str),
    })
}

fn wizard_verify() -> Result<()> {
    let input_str = prompt("Archive path: ")?;
    if input_str.is_empty() {
        bail!("No archive path provided");
    }
    cmd_verify(VerifyOpts {
        input: PathBuf::from(input_str),
    })
}

// ============================================================================
// Command implementations
// ============================================================================

fn cmd_create(opts: CreateOpts) -> Result<()> {
    // Pre-scan to get total uncompressed bytes for progress.
    let mut total_bytes: u64 = 0;
    let mut total_files: u64 = 0;
    for input in &opts.inputs {
        if !input.exists() {
            bail!("Input not found: {}", input.display());
        }
        let (n, b) = scan_path(input)?;
        total_files += n;
        total_bytes += b;
    }
    // Estimate block count (rough — solid blocks at threshold).
    let est_blocks = ((total_bytes as usize).saturating_sub(1) / opts.block_threshold + 1) as u32;

    if let Some(parent) = opts.output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }

    let file = std::fs::File::create(&opts.output)
        .with_context(|| format!("creating {}", opts.output.display()))?;

    // Print summary before spawning the renderer so it doesn't clobber the 3-line block.
    println!(
        "Packing {total_files} files ({}) into {}",
        format_bytes(total_bytes),
        opts.output.display()
    );

    let (tx, renderer) = spawn_progress_renderer();
    let tx2 = tx.clone();
    let start = Instant::now();

    let mut writer = ArchiveWriter::new(file)
        .context("initializing archive writer")?
        .with_method(opts.method)
        .with_block_threshold(opts.block_threshold)
        .with_progress(move |p: WriteProgress| {
            let _ = tx2.try_send(ProgressEvent::Writing {
                block_id: p.block_id,
                total_blocks_hint: est_blocks,
                files_in_block: p.files_in_block,
                ratio: p.block_ratio,
                bytes_done: p.total_uncompressed_bytes,
                total_bytes,
            });
        });

    for input in &opts.inputs {
        add_path(&mut writer, input)?;
    }
    writer.finish().context("finalizing archive")?;

    let elapsed = start.elapsed();
    let arc_size = std::fs::metadata(&opts.output)
        .map(|m| m.len())
        .unwrap_or(0);
    let ratio_pct = arc_size as f64 / total_bytes.max(1) as f64 * 100.0;

    let summary = format!(
        "{total_files} files | {} → {} ({ratio_pct:.1}%)",
        format_bytes(total_bytes),
        format_bytes(arc_size),
    );
    let _ = tx.send(ProgressEvent::Done { elapsed, summary });
    let _ = renderer.join();

    Ok(())
}

fn cmd_extract(opts: ExtractOpts) -> Result<()> {
    let file = std::fs::File::open(&opts.input)
        .with_context(|| format!("opening {}", opts.input.display()))?;
    let mut reader =
        ArchiveReader::open(file).with_context(|| format!("reading {}", opts.input.display()))?;

    let total_files = reader.entries().len();
    let total_bytes: u64 = reader.entries().iter().map(|e| e.original_size).sum();
    println!(
        "Extracting {total_files} files ({}) from {}",
        format_bytes(total_bytes),
        opts.input.display()
    );

    let (tx, renderer) = spawn_progress_renderer();
    let tx2 = tx.clone();
    let start = Instant::now();

    reader
        .extract_all(
            |progress: ExtractProgress, data: Vec<u8>| -> anyhow::Result<()> {
                let dest = opts.dest.join(&progress.path);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("creating {}", parent.display()))?;
                }
                std::fs::write(&dest, &data)
                    .with_context(|| format!("writing {}", dest.display()))?;

                let _ = tx2.try_send(ProgressEvent::Extracting {
                    file_index: progress.file_index,
                    total_files: progress.total_files,
                    path: progress.path,
                    bytes_done: progress.total_bytes_done,
                    total_bytes: progress.total_bytes,
                    block_id: progress.block_id,
                    total_blocks: progress.total_blocks,
                });
                Ok(())
            },
        )
        .with_context(|| format!("extracting {}", opts.input.display()))?;

    let elapsed = start.elapsed();
    let summary = format!("{total_files} files extracted to {}", opts.dest.display());
    let _ = tx.send(ProgressEvent::Done { elapsed, summary });
    let _ = renderer.join();

    Ok(())
}

fn cmd_list(opts: ListOpts) -> Result<()> {
    let file = std::fs::File::open(&opts.input)
        .with_context(|| format!("opening {}", opts.input.display()))?;
    let reader =
        ArchiveReader::open(file).with_context(|| format!("reading {}", opts.input.display()))?;

    let entries = reader.entries();
    let total_bytes: u64 = entries.iter().map(|e| e.original_size).sum();

    println!("{:<8}  {:<12}  {}", "Block", "Size", "Path");
    println!("{}", "-".repeat(60));
    for entry in entries {
        println!(
            "{:<8}  {:<12}  {}",
            entry.block_id,
            format_bytes(entry.original_size),
            entry.path
        );
    }
    println!("{}", "-".repeat(60));
    println!(
        "{} files, {} total",
        entries.len(),
        format_bytes(total_bytes)
    );

    Ok(())
}

fn cmd_verify(opts: VerifyOpts) -> Result<()> {
    let file = std::fs::File::open(&opts.input)
        .with_context(|| format!("opening {}", opts.input.display()))?;
    let mut reader =
        ArchiveReader::open(file).with_context(|| format!("reading {}", opts.input.display()))?;

    let total_files = reader.entries().len();
    let total_bytes: u64 = reader.entries().iter().map(|e| e.original_size).sum();
    println!(
        "Verifying {total_files} files ({}) in {}",
        format_bytes(total_bytes),
        opts.input.display()
    );

    let (tx, renderer) = spawn_progress_renderer();
    let tx2 = tx.clone();
    let start = Instant::now();
    let mut verified = 0usize;

    reader
        .extract_all(
            |progress: ExtractProgress, _data: Vec<u8>| -> anyhow::Result<()> {
                verified += 1;
                let _ = tx2.try_send(ProgressEvent::Extracting {
                    file_index: progress.file_index,
                    total_files: progress.total_files,
                    path: progress.path,
                    bytes_done: progress.total_bytes_done,
                    total_bytes: progress.total_bytes,
                    block_id: progress.block_id,
                    total_blocks: progress.total_blocks,
                });
                Ok(())
            },
        )
        .with_context(|| format!("verifying {}", opts.input.display()))?;

    let elapsed = start.elapsed();
    let summary = format!("{verified}/{total_files} files verified OK");
    let _ = tx.send(ProgressEvent::Done { elapsed, summary });
    let _ = renderer.join();

    Ok(())
}

// ============================================================================
// Filesystem helpers
// ============================================================================

/// Recursively walk a path and add all files to the writer.
/// Directories are stored with the directory name as the archive prefix.
/// Plain files are stored under their filename only.
fn add_path(writer: &mut ArchiveWriter<std::fs::File>, path: &Path) -> Result<()> {
    if path.is_file() {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .with_context(|| format!("non-UTF-8 filename: {}", path.display()))?;
        let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        writer
            .add_file(name, &data)
            .with_context(|| format!("adding {name}"))?;
        return Ok(());
    }

    // Directory: use directory name as prefix.
    let base_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .with_context(|| format!("non-UTF-8 directory name: {}", path.display()))?;

    for entry in WalkDir::new(path).sort_by_file_name() {
        let entry = entry.with_context(|| format!("walking {}", path.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(path)
            .expect("walkdir always returns paths under root");
        let rel_str = rel
            .to_str()
            .with_context(|| format!("non-UTF-8 path: {}", rel.display()))?;
        let arc_path = format!("{base_name}/{}", rel_str.replace('\\', "/"));
        let data = std::fs::read(entry.path())
            .with_context(|| format!("reading {}", entry.path().display()))?;
        writer
            .add_file(&arc_path, &data)
            .with_context(|| format!("adding {arc_path}"))?;
    }
    Ok(())
}

/// Return (file_count, total_bytes) for a file or directory tree.
fn scan_path(path: &Path) -> Result<(u64, u64)> {
    let mut count = 0u64;
    let mut bytes = 0u64;
    for entry in WalkDir::new(path).sort_by_file_name() {
        let entry = entry.with_context(|| format!("scanning {}", path.display()))?;
        if entry.file_type().is_file() {
            bytes += entry.metadata()?.len();
            count += 1;
        }
    }
    Ok((count, bytes))
}

// ============================================================================
// Help / version
// ============================================================================

fn print_help() {
    println!(
        "arcmax {version} — FreeARC-compatible archive utility",
        version = env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("USAGE:");
    println!("  arcmax create  <output.arc> <path...> [OPTIONS]  — create archive");
    println!("  arcmax extract <input.arc>  [dest-dir]           — extract archive");
    println!("  arcmax list    <input.arc>                       — list contents");
    println!("  arcmax verify  <input.arc>                       — verify integrity");
    println!("  arcmax                                           — interactive wizard");
    println!();
    println!("CREATE OPTIONS:");
    println!("  --method  zstd|lz4|tornado|store|…   Compression algorithm (default: zstd)");
    println!("  --level   N                Zstd level 1-22 (default: 22)");
    println!("  --block   N[K|M|G]         Solid block size (default: 256M)");
    println!();
    println!("ALIASES: create=c, extract=x/e, list=l/t, verify=v");
}

fn print_version() {
    println!("arcmax {}", env!("CARGO_PKG_VERSION"));
}

// ============================================================================
// Entry point
// ============================================================================

pub fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match parse_args(&args)? {
        Command::Create(opts) => cmd_create(opts),
        Command::Extract(opts) => cmd_extract(opts),
        Command::List(opts) => cmd_list(opts),
        Command::Verify(opts) => cmd_verify(opts),
        Command::Interactive => interactive_wizard(),
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::Version => {
            print_version();
            Ok(())
        }
    }
}
