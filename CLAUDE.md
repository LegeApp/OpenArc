# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

OpenArc is a media archiver for photo/camera/video folders. It converts standard
images to JPEG XL, stages inefficient videos for external encoding, preserves
camera RAW losslessly, and bundles everything into a single `.oarc` (a
`.tar.zst` container) with manifests and hashes. It ships as both a library
(`openarc`) and a CLI binary (`openarc`), plus an interactive terminal wizard.

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

`crates/codecs` has non-optional path dependencies on two **sibling checkouts**
that must sit next to this repo:

- **`../jpegXL-rs`** — the JPEG XL codec, OpenArc's image encoder. Used as
  `JPXL/crates/jpxl-{core,encode,encode-policy,decode}`.
- **`../bpg-rs`** — used only for `bpg-decode`: its HEIF/ISOBMFF parser is how
  HEIC sources are read, and it decodes `.bpg` entries in archives written
  before the JPEG XL switch. Nothing encodes BPG any more.

Everything is pure Rust — **no CMake, no C/C++ toolchain, no build script**.
The former `crates/codecs/build.rs` (which compiled libbpg + vendored x265) and
the BPG encoder modules were retired to `unused/bpg/`.

- System libs for transitive image crates (Debian/Ubuntu):
  `sudo apt install ffmpeg libpng-dev libjpeg-turbo8-dev liblcms2-dev zlib1g-dev`
- `ffprobe` on PATH enables video compression analysis (optional; degrades
  gracefully).

### Feature flags

There are none for the image codec. The old `bpg-c` / `bpg-rs` backend
selectors are gone: there is one JPEG XL implementation and no compile-time
choice to make.

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
   - **Image** → `image_source::load` (one decoder per source format, at the
     widest precision the file carries) → `codecs::jxl::encode`. There is no
     fallback codec: JPEG XL is the only image output. The JPEG 2000 fallback
     was removed after measurement found it unnecessary.
   - **Video** → `analyze_video_compression` (ffprobe, 5s timeout); efficient
     videos pass through under `media/`; videos needing external encoding are
     staged. Archive mode waits for the user-provided encoded-output directory,
     merges those files directly under `media/`, and only then finalizes the
     requested archive path (`append_external_video_bundle`).
   - **Raw** → stored losslessly in `raw.arc` (LZMA2-compressed tar stream).
   - **Misc** → `misc.arc` (LZMA2-compressed tar stream).
5. **Bundle** into the `.oarc` (`.tar.zst`) container.

Restoration metadata (`OPENARC_METADATA.json`, `ImageMetadata` /
`OriginalImageFormat`) records each image's original format so extraction can
reverse the conversion. `ImageMetadata::encoded_filename` carries a
`#[serde(alias = "bpg_filename")]` so archives written before the switch still
extract. Output layout: `media/`, `raw.arc`, `misc.arc`,
`OPENARC_METADATA.json`, `MANIFEST.txt`, `HASHES.sha256`.

Codec work runs on dedicated threads with a 16 MiB stack
(`run_with_codec_stack` in `main.rs`, mirrored in the orchestrator) because the
codecs are stack-hungry.

### Image quality invariants

These are the point of the JPEG XL switch and should not be regressed:

- **No chroma subsampling anywhere.** VarDCT is XYB; there is no chroma-format
  setting and none is applied. (The BPG path defaulted to 4:2:0.)
- **Source bit depth is carried and declared.** `image_source` reads the depth
  off the decoded buffer rather than guessing from the format, and the encoder
  writes it into `ImageMetadata.bit_depth`, so a 16-bit source decodes back at
  16-bit. This required extending the JPXL API (see below).
- **Greyscale stays one channel** and takes the lossless modular track.
- **Transparency is never silently dropped.** A fully-opaque alpha channel is
  discarded (it carries nothing); an alpha channel actually in use causes the
  source file to be stored unchanged, because the JPXL encoder does not yet
  implement extra channels.

### Modules (`src/`)

- `main.rs` / `cli.rs` — clap CLI. Subcommands: `interactive`, `create`,
  `extract`, `list`, `convert-jxl`, `batch-jxl` (aliased from the old
  `convert-bpg` / `batch-bpg`), and `arc-*` (raw ArcMax compress/extract/test),
  `phone-detect`. No args → interactive mode.
- `interactive.rs` / `interactive_menu.rs` — crossterm TUI wizard.
- `jxl_wrapper.rs` — public-facing JPEG XL presets (`JxlEffort`, `JxlConfig`):
  `best` (default), `fast`, `lossless`. `best` maps to JPXL Balanced and `fast`
  maps to JPXL Fast; the exhaustive JPXL Quality controller is not exposed.
  The quality dial is a **bitrate** (`--jxl-bpp`), not a QP — JPEG XL rate
  control targets a size, not a perceptual distance.
- `image_source.rs` — loads any source at the widest precision it carries, and
  reports whether alpha is actually in use. Every image reaching the encoder
  goes through here.
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

- `vendor/arcmax` — the compression engine. OpenArc uses **LZMA2 and Zstd
  only**; it does not create FreeARC-format archives. Used via
  `arcmax::{compress_with, decompress, Method, CompressionOptions}`. Referenced
  as a path dep from the root Cargo.toml.
- `crates/codecs` — `jxl` (JPEG XL encode/decode, the image codec), HEIC decode
  via `bpg-decode`'s HEIF parser (*not* libheif), `jpeg2000` (`jp2lam`,
  **decode only** — it is an input format, no longer an encode fallback),
  `bpg_legacy` (decode only, for pre-switch archives), and `video_analyzer`.
  `mod.rs` is the crate root (`path = "mod.rs"`).
- `crates/winmtp` — Windows MTP device access (only built `cfg(windows)`).
- `crates/zune-image`, `vendor/arcmax/vendor` — vendored image/codec libraries.
  `jp2lam` is fetched directly from GitHub; `openjp2` and `dng-rs` are excluded
  from the workspace.

### Native code

None. `native/BPG/libbpg-0.9.8/` (libbpg + a vendored x265 4.1 tree) is dead
weight kept on disk only for history; nothing builds it, and the build script
that used to has moved to `unused/bpg/codecs-build.rs`.

### JPXL API extensions made for this project

The sibling `jpegXL-rs` repo was extended so the lossy path could carry more
than 8-bit, which it previously could not:

- `jpxl-encode`: `vardct::headers::write_image_headers_with_depth` writes a
  non-`all_default` `ImageMetadata` when the depth is not 8 (byte-identical to
  the old two-bit header at 8), and `vardct::plan::FrameDecision` gained
  `bits_per_sample`.
- `jpxl-encode-policy`: `PreparedFrame::from_srgb16{,_with}`,
  `EncodeRequest::bits_per_sample`, and the public
  `encode_srgb16_to_target` / `encode_srgb16_vardct` entry points.

The 8-bit path is unchanged and pinned byte-identical by
`the_wide_entry_point_agrees_with_the_8_bit_one_on_8_bit_input`.

## Conventions worth knowing

- `.oarc` = `.tar.zst`. The container ZSTD level (`--compression-level`, default
  3) wraps already-compressed data, so keep it low; LZMA2 does the real work
  inside `raw.arc`/`misc.arc` (`--misc-compression-level`, default 6).
- Image bit depth follows the source: 8-bit sources stay 8-bit, and 9..=16-bit
  sources are encoded and declared at their own depth (JPEG XL's integer
  ceiling is 16).
- Release profile uses thin LTO + `codegen-units = 1`; there's also a
  `release-lto` profile.
