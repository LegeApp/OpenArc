# Building OpenArc

Single entrypoint: `cargo dist`.

It runs `cargo build --release` and stages the binary at `dist/openarc.exe` (Windows) or `dist/openarc` (Linux). OpenArc no longer links FFmpeg. Image encoding is handled by Rust `bpg-rs` crates, and video files are classified with `ffprobe` at runtime.

On Windows the repo pins the GNU (MinGW) Rust toolchain via `rust-toolchain.toml`.

BPG still-image encoding uses the Rust `bpg-rs` encoder at QP 28 by default and the `Best` effort tier.

## Prerequisites

### Windows (MinGW / GNU toolchain)

- Rust toolchain: the repo's `rust-toolchain.toml` pins `stable-x86_64-pc-windows-gnu`; rustup installs it automatically on first build.
- [MSYS2](https://www.msys2.org/) with the mingw64 toolchain and required image/runtime libraries:

  ```
  pacman -S --needed \
    mingw-w64-x86_64-gcc \
    mingw-w64-x86_64-libpng \
    mingw-w64-x86_64-libjpeg-turbo \
    mingw-w64-x86_64-lcms2 \
    mingw-w64-x86_64-zlib
  ```

  Also install `ffprobe` and make sure it is available on `PATH` (OpenArc uses it to classify video compression efficiency).

- MSYS2 location: the build auto-detects `C:\msys64`, `C:\msys2`, `D:\msys64`, `D:\msys2`, `E:\msys64`. For a different path, set `MSYS2_ROOT` (e.g. `setx MSYS2_ROOT F:\dev\msys64`).

### Linux

- Stable Rust toolchain. The `rust-toolchain.toml` pin names a Windows host triple; on Linux override it with `rustup override set stable` in the repo checkout.
- System packages for image/runtime libs and `ffprobe`. On Debian/Ubuntu:

  ```
  sudo apt install ffmpeg libpng-dev libjpeg-turbo8-dev liblcms2-dev zlib1g-dev
  ```

## Build

```
git clone https://github.com/LegeApp/OpenArc.git
cd OpenArc
cargo dist
```

On Windows no `--target` flag is needed — the pinned toolchain already targets `x86_64-pc-windows-gnu`, so `cargo build --release` produces the GNU binary directly.

First build takes a few minutes (FreeArc codecs + Rust crates). Incremental builds re-link only.

## Outputs

- `dist/openarc.exe` or `dist/openarc` — the distribution binary.
- `target/release/openarc.exe` — the same binary in the standard cargo location.

## Notes on the archive format

- Standard images are converted to BPG and stored in `media/`
- Already-compressed videos are bundled into `misc.arc`
- Uncompressed videos are staged outside the archive for external encoding and can be merged later as `videos.arc`
- Camera RAW files are copied losslessly into `raw.arc`
- Miscellaneous files are copied losslessly into `misc.arc`
- The outer `.oarc` file is a `.tar.zst` container

On extraction, `raw.arc` is expanded into `raw/` and `misc.arc` is expanded into `misc/`.
