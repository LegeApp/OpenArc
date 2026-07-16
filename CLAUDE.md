# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

OpenArc is a media archiver for photo/camera/video folders. It converts standard
images to BPG, optionally recompresses videos, preserves camera RAW losslessly,
and bundles everything into a single `.oarc` (a `.tar.zst` container) with
manifests and hashes. It ships as both a library (`openarc`) and a CLI binary
(`openarc`), plus an interactive terminal wizard.

## Build & run

```sh
cargo build --release          # normal stable host toolchain (pinned: stable)
cargo dist                     # release build + copy binary into dist/ (xtask alias)
cargo run -- create -o out.oarc ~/Pictures   # run the CLI in dev
```

- `cargo dist` is a cargo alias for the `xtask` package (`.cargo/config.toml`),
  which builds `--release --package openarc` and stages the binary to
  `dist/openarc[.exe]`.
- The workspace `default-members` is `["."]`, so plain `cargo build` builds only
  the root `openarc` package, not the vendored crates.

### Build prerequisites (not optional)

- **Sibling `bpg-rs` repo must exist at `../bpg-rs`.** `crates/codecs` has
  non-optional path dependencies on `../../../bpg-rs/crates/{bpg-decode,...}`.
  Without it, nothing compiles.
- **Default `bpg-rs` feature builds pure Rust — no CMake/C++ toolchain needed.**
  The legacy `bpg-c` feature (opt-in) compiles native libbpg + vendored x265 4.1
  via `crates/codecs/build.rs` (needs CMake + a C/C++ toolchain) and produces
  8/10/12-bit x265 static libs under `OUT_DIR`.
- System libs for transitive image crates (Debian/Ubuntu):
  `sudo apt install ffmpeg libpng-dev libjpeg-turbo8-dev liblcms2-dev zlib1g-dev`
- `ffprobe` on PATH enables video compression analysis (optional; degrades
  gracefully).

### BPG backend feature flags

`crates/codecs` (and the root crate that re-exports it) selects the BPG codec at
compile time. Features are additive, so `bpg-rs` *overrides* the default:

- `bpg-rs` (default): pure-Rust encoder/decoder from the sibling `bpg-rs` crates
  (`bpg_rs.rs`). Its presence suppresses all C/C++ compilation in build.rs.
- `bpg-c` (legacy, opt-in): native libbpg + vendored x265 (`bpg_c.rs`, build.rs
  compiles C/C++). Build with
  `cargo build --no-default-features --features bpg-c` (root) — note the root
  crate forwards these as `codecs/bpg-c` / `codecs/bpg-rs`.

## Tests

```sh
cargo test                                   # root package tests
cargo test -p codecs                         # codec crate (raw_tests.rs, heic migration)
cargo test -p arcmax                         # compression engine
cargo test <name> -- --nocapture             # single test by name
```

Inline `#[cfg(test)]` modules live in most `src/*.rs` and `crates/codecs/*.rs`.
Integration tests are under `crates/codecs/tests/`, `vendor/arcmax/tests/`, and
`crates/winmtp/tests/`.

## Architecture

### Pipeline (the core of the app)

`src/orchestrator.rs` is the heart — `create_archive()` and
`extract_archive_with_decoding()` drive everything. The create flow:

1. **Discover** files (`collect_files`).
2. **Hash** every file once (SHA-256), reused by both tracking and dedup.
3. **Classify** each file (`classify_file` → `FileClass`: `Image | Video | Raw | Misc`).
4. **Process per class** (parallel, via tokio `JoinSet` + a custom
   `MemoryBudgetLimiter` that throttles concurrency by estimated peak RAM so
   large-image encodes don't OOM):
   - **Image** → decode → BPG encode. If BPG output trips bitrate criteria, a
     **JPEG 2000 q85 fallback** is tried and kept only if smaller.
   - **Video** → `analyze_video_compression` (ffprobe, 5s timeout); recompress
     or pass through; videos needing external encoding are *staged* for the user
     to encode and merge back (`append_external_video_bundle`).
   - **Raw** → stored losslessly in `raw.arc` (LZMA2 level 9, 128 MiB dict).
   - **Misc** → `misc.arc` (LZMA2).
5. **Bundle** into the `.oarc` (`.tar.zst`) container.

Restoration metadata (`OPENARC_METADATA.json`, `ImageMetadata` /
`OriginalImageFormat`) records each image's original format so extraction can
reverse the conversion. Output layout: `media/`, `raw.arc`, `misc.arc`,
`OPENARC_METADATA.json`, `MANIFEST.txt`, `HASHES.sha256`.

Codec work runs on dedicated threads with a 16 MiB stack
(`run_with_codec_stack` in `main.rs`, mirrored in the orchestrator) because the
codecs are stack-hungry.

### Modules (`src/`)

- `main.rs` / `cli.rs` — clap CLI. Subcommands: `interactive`, `create`,
  `extract`, `list`, `convert-bpg`, `batch-bpg`, and `arc-*` (raw ArcMax
  compress/extract/test), `phone-detect`. No args → interactive mode.
- `interactive.rs` / `interactive_menu.rs` — crossterm TUI wizard.
- `bpg_wrapper.rs` — public-facing BPG presets (`BpgEffort`, `BpgAq`,
  `BpgConfig`); maps friendly names (balanced/good/best/placebo) to encoder params.
- `image_loader.rs` / `jpeg_decoder.rs` — decode via zune-jpeg (JPEG) and the
  `image` crate (everything else).
- `backup_catalog.rs` / `file_tracker.rs` / `archive_tracker.rs` — SQLite
  (rusqlite, bundled) for incremental-backup catalog and cross-run dedup; DB
  lives in the user's AppData/config dir. Disabled via `--no-catalog`,
  `--no-dedup`, `--no-tracking`.
- `phone_backup.rs` — detect phones (MTP on Windows via the `winmtp` crate;
  mounted filesystems elsewhere).
- `hash.rs` — SHA-256 helpers.

### Workspace crates

- `vendor/arcmax` — the compression engine. Pure-Rust port of FreeArc codecs;
  in practice OpenArc uses **LZMA2 and Zstd only** (the rest of FreeArc was
  abandoned). Used via `arcmax::{compress_with, decompress, Method,
  CompressionOptions}`. Referenced as a path dep from the root Cargo.toml.
- `crates/codecs` — BPG (C or Rust backend), HEIC decode (pure-Rust
  `heic-decoder-rs`, *not* libheif), JPEG 2000 (`jp2lam`), RAW (`libraw_sys`),
  and `video_analyzer`. `mod.rs` is the crate root (`path = "mod.rs"`).
- `crates/winmtp` — Windows MTP device access (only built `cfg(windows)`).
- `crates/zune-image`, `jp2lam`, `vendor/arcmax/vendor` — vendored image/codec
  libraries. `openjp2`, `jp2lam`, `dng-rs` are **excluded** from the workspace.

### Native code

`native/BPG/libbpg-0.9.8/` holds libbpg + a full vendored x265 4.1 tree,
compiled by `crates/codecs/build.rs` for the default `bpg-c` path. Env overrides:
`OPENARC_BPG_X265=system` (use pkg-config x265 instead of vendored),
`BPG_X265_SINGLE_THREAD`, `BPG_X265_PARAMS`. There is also `ucrt_compat.c` to fix
a UCRT linker issue on Windows.

## Conventions worth knowing

- `.oarc` = `.tar.zst`. The container ZSTD level (`--compression-level`, default
  3) wraps already-compressed data, so keep it low; LZMA2 does the real work
  inside `raw.arc`/`misc.arc` (`--misc-compression-level`, default 6).
- BPG bit depth is adaptive: 8-bit sources stay 8-bit; higher-depth sources go up
  to 12-bit.
- Release profile uses thin LTO + `codegen-units = 1`; there's also a
  `release-lto` profile.
