# OpenArc

OpenArc is a media archiver for photo and video folders with both interactive and command-line workflows. Point it at one or more folders, and it will convert standard images to BPG, recompress videos when that helps, preserve camera RAW files losslessly, and bundle the result into a single archive with manifests and hashes.

## Archive layout

- `media/`: converted images and processed videos
- `raw.arc`: preserved camera-source files packed with FreeArc
- `misc.arc`: other preserved files packed with FreeArc
- `OPENARC_METADATA.json`: image restoration metadata
- `MANIFEST.txt`: user-facing archive contents
- `HASHES.sha256`: integrity hashes

## Quick start

```bash
git clone https://github.com/LegeApp/OpenArc.git
cd OpenArc
cargo dist
```

That builds the Rust workspace and stages a binary at `dist/openarc.exe` (Windows) or `dist/openarc` (Linux/macOS). See [`BUILDING.md`](BUILDING.md) for the short prerequisite list.

```bash
./dist/openarc create -o archive.oarc ~/Pictures ~/Videos
```

## CLI usage

```bash
openarc
openarc interactive
openarc create -o my-archive.oarc ~/Pictures ~/Videos
openarc extract -i my-archive.oarc -o restored
openarc list my-archive.oarc
```

Running `openarc` with no arguments launches interactive mode. Use `openarc create`, `openarc extract`, and `openarc list` for non-interactive runs.

At the end of archive creation, the CLI prints how many RAW files were preserved separately and their total size.

## Build

OpenArc uses the normal stable Rust host toolchain:

```bash
cargo build --release
```

Use `cargo dist` when you also want the binary copied into `dist/`.

## Repository shape

```text
OpenArc/
├── src/                    # CLI and archive orchestration
├── crates/
│   ├── arcmax/             # FreeArc implementation (C++ compiled by build.rs)
│   ├── codecs/             # BPG-rs facade, HEIC/JP2 handling, and video analysis
│   ├── zune-image/         # vendored image workspace
│   └── winmtp/             # Windows MTP helper
├── jp2lam/                 # vendored JPEG 2000 crate
├── xtask/                  # `cargo dist` helper
└── dist/                   # staged build output
```
