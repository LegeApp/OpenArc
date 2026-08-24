# Building OpenArc

OpenArc now builds as a normal Rust workspace. The repository pins the portable
`stable` Rust channel in `rust-toolchain.toml`, so the default command is:

```sh
cargo build --release
```

For a staged distribution binary:

```sh
cargo dist
```

`cargo dist` runs the release build for the `openarc` package and copies the
result to `dist/openarc` on Linux/macOS or `dist/openarc.exe` on Windows.

## Prerequisites

- Stable Rust via rustup.
- `ffprobe` on `PATH` if you want video compression analysis.
- System image libraries required by transitive image crates. On Debian/Ubuntu:

```sh
sudo apt install ffmpeg libpng-dev libjpeg-turbo8-dev liblcms2-dev zlib1g-dev
```

Windows uses the normal stable Rust host toolchain. No project-level MinGW,
MSYS2, C/C++, or bundled FFmpeg toolchain override is required: the image codec
is the pure-Rust JPEG XL implementation in the sibling `jpegXL-rs` repository,
so there is no native compilation step at all.

### Sibling repositories

`crates/codecs` has path dependencies on two sibling checkouts, which must sit
next to this one:

- `../jpegXL-rs` - the JPEG XL encoder and decoder (`JPXL/crates/jpxl-*`). This
  is the image codec.
- `../bpg-rs` - used only for its HEIF/ISOBMFF parser (how HEIC sources are
  read) and for decoding `.bpg` entries in archives written before the JPEG XL
  switch. Nothing encodes BPG any more.

## Outputs

- `target/release/openarc`
- `target/release/openarc.exe` on Windows
- `dist/openarc` or `dist/openarc.exe` when using `cargo dist`

## Archive Format

- Standard images are converted to JPEG XL (`.jxl`) and stored in `media/`.
  Colour is 4:4:4 throughout - JPEG XL has no chroma subsampling - and each
  image keeps its source bit depth, up to 16 bits.
- An image whose alpha channel is actually used is stored unchanged instead,
  because the encoder cannot yet carry extra channels.
- Already-compressed videos are stored directly under `media/`.
- Videos needing external encoding are staged beside the requested output.
  Archive mode waits for a user-supplied encoded-output folder before the final
  `.oarc` path is completed.
- Camera RAW files are copied losslessly into an LZMA2-compressed tar stream
  named `raw.arc`.
- Miscellaneous files are copied losslessly into an LZMA2-compressed tar stream
  named `misc.arc`.
- The default `.oarc` output is a `.tar.zst` container.
