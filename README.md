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

That builds the Rust workspace plus remaining native codec wrappers and produces a self-contained binary at `dist/openarc.exe` (Windows) or `dist/openarc` (Linux). See [`BUILDING.md`](BUILDING.md) for prerequisites (MSYS2 packages on Windows, `-dev` packages on Linux).

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

Single command: `cargo dist`. See [`BUILDING.md`](BUILDING.md) for prerequisites.

## Repository shape

```text
OpenArc/
├── src/                    # CLI and archive orchestration
├── crates/
│   ├── arcmax/             # FreeArc implementation (C++ compiled by build.rs)
│   ├── codecs/             # BPG-rs facade + FFmpeg wrappers
│   ├── zune-image/         # vendored image workspace
│   └── winmtp/             # Windows MTP helper
├── native/                 # bundled native components such as jp2lam
├── zenzstd/                # Rust Zstandard implementation
├── xtask/                  # `cargo dist` helper
└── dist/                   # build output: openarc.exe
```
