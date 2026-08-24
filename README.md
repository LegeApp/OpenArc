# OpenArc

OpenArc is a media archiver for photo and video folders with both interactive
and command-line workflows. It converts standard images to JPEG XL, automatically
develops supported camera RAW files and encodes their 16-bit sRGB output to JPEG
XL, stages inefficient videos for external encoding, and bundles the result into
a Zstandard-compressed archive with manifests and hashes. For a directory-local
JPEG+RAW camera pair with the same stem, only the RAW-derived image is archived.

### Image quality

Every image output is JPEG XL, and the pipeline is built to carry through as
much of the original as the codec can hold:

- **No chroma subsampling, ever.** JPEG XL VarDCT works in XYB, so there is no
  4:2:0 step to lose three quarters of the chroma the way the previous BPG path
  did.
- **Source bit depth is preserved.** A 16-bit TIFF, a 10-bit HEIC and a
  developed RAW are encoded *and declared* at their own depth, so they decode
  back at that depth instead of being flattened to 8-bit.
- **Greyscale stays greyscale**, and is encoded losslessly.
- **`--jxl-effort lossless`** encodes exact samples through the JPEG XL modular
  track when nothing may be lost at all.

The quality dial is a **bitrate** (`--jxl-bpp`), not a quantizer or a
perceptual distance: JPEG XL rate control here targets a size. The lossy
presets (`best`, `fast`) name archival-leaning bitrates; `best` uses JPXL's
production Balanced controller and `fast` uses its Fast controller. Override
the bitrate with `--jxl-bpp` once you have measured your own corpus.

Images whose alpha channel is actually in use are stored unchanged rather than
encoded, because the encoder does not yet implement extra channels - so no
image silently loses its transparency.

## Archive layout

- `media/`: converted images as `.jxl` (including developed RAW files) and processed videos
- `misc.arc`: other preserved files in an LZMA2-compressed tar stream
- `OPENARC_METADATA.json`: image restoration metadata
- `OPENARC_INDEX.json`: compact machine-readable source/stored paths, sizes, classes, and SHA-256 identities
- `MANIFEST.txt`: user-facing archive contents
- `HASHES.sha256`: integrity hashes

The extractor still accepts `raw.arc` from archives produced by older OpenArc
versions.

OpenArc keeps cross-job file history in one per-user database (`%APPDATA%/OpenArc/tracking.db` on Windows, the platform data directory elsewhere). It stores one current tracking row per unique content hash and one current archive mapping per output path; archives remain standalone and never omit a path merely because it appeared in an earlier job.

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

The BPG production preset is `best` (default); `fast` is the only other public
effort tier. Adaptive quantization is off by default. Supplying `--bpg-aq`
without a value enables the recommended measured two-pass mode; explicit
choices are `off`, `two-pass`, `perceptual`, and `perceptual-chroma`.

When archive mode finds a video that needs external encoding, OpenArc stages the
original and waits. The requested `.oarc` path is not finalized until the user
provides a clean folder containing one encoded output per staged video. Those
outputs are stored directly under `media/`, not recompressed with LZMA2.

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
│   ├── arcmax/             # LZMA2/Zstandard compression engine
│   ├── codecs/             # BPG-rs facade, HEIC/JP2 handling, and video analysis
│   ├── zune-image/         # vendored image workspace
│   └── winmtp/             # Windows MTP helper
├── xtask/                  # `cargo dist` helper
└── dist/                   # staged build output
```

JPEG 2000 support is fetched directly from the latest `LegeApp/jp2lam`
revision on GitHub; OpenArc does not override it with a local checkout.
