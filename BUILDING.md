# Building OpenArc

Single entrypoint: `cargo dist`.

It runs `cargo build --release` and stages the binary at `dist/openarc.exe` (Windows) or `dist/openarc` (Linux). All codec C/C++ source is compiled in-tree by `build.rs` scripts — no separate codec build step. FFmpeg, x265, x264, BPG, libpng, libjpeg, lcms2, libraw, and the C++ runtime are statically linked into the binary.

On Windows the repo pins the GNU (MinGW) Rust toolchain via `rust-toolchain.toml`, so rustc, build scripts, and all in-tree C/C++ use the same MSYS2 GCC family — one toolchain, one `cargo build --release`, no separate DLL build step. (MSVC cannot consume the GCC-built MSYS2 archives, which is what previously forced the out-of-band `openarc_bpg.dll` build.)

One caveat: MSYS2's static FFmpeg archives were compiled against a few DLL-only libraries (the GLib/cairo/rsvg stack, libhwy, shaderc), so the exe keeps DLL imports for those. `cargo dist` detects them from the binary's import table and copies them from `mingw64/bin` into `dist/` automatically. A bare `target/release/openarc.exe` runs as long as `C:\msys64\mingw64\bin` is on `PATH`. To eliminate these DLLs entirely, build a custom static FFmpeg (see below).

Both BPG HEVC encoders are compiled in by default: x265 (`encoder_type` 0, fast) and JCTVC (`encoder_type` 1, the HM reference encoder — slower, ~25% better compression, up to 14-bit). JCTVC can be opted out with `cargo build --no-default-features` on the codecs crate.

## Optional: fully static FFmpeg (no DLL sidecars)

The DLL imports above come from features the MSYS2 FFmpeg package enables (librsvg, libjxl, libplacebo) that OpenArc never uses. A custom FFmpeg without them produces an exe with zero MSYS2 DLL imports:

```
# from an MSYS2 mingw64 shell
pacman -S --needed base-devel nasm mingw-w64-x86_64-gcc
./scripts/build-static-ffmpeg.sh
```

Then point the build at it and rebuild (the codecs build script re-runs automatically when the variable changes):

```powershell
$env:OPENARC_FFMPEG_PREFIX = "C:\ffmpeg-openarc"
cargo dist
```

The prefix's headers, static libraries, and pkg-config files take precedence over MSYS2's; everything FFmpeg doesn't provide (x264, x265, libpng, libraw, ...) still resolves from the MSYS2 tree.

## Prerequisites

### Windows (MinGW / GNU toolchain)

- Rust toolchain: the repo's `rust-toolchain.toml` pins `stable-x86_64-pc-windows-gnu`; rustup installs it automatically on first build.
- [MSYS2](https://www.msys2.org/) with the mingw64 toolchain plus codec libraries:

  ```
  pacman -S --needed \
    mingw-w64-x86_64-gcc \
    mingw-w64-x86_64-x265 \
    mingw-w64-x86_64-x264 \
    mingw-w64-x86_64-libpng \
    mingw-w64-x86_64-libjpeg-turbo \
    mingw-w64-x86_64-libraw \
    mingw-w64-x86_64-lcms2 \
    mingw-w64-x86_64-zlib \
    mingw-w64-x86_64-ffmpeg
  ```

  These supply both headers (under `mingw64/include`) and **static archives** (`*.a` under `mingw64/lib`) — both are required.

- MSYS2 location: the build auto-detects `C:\msys64`, `C:\msys2`, `D:\msys64`, `D:\msys2`, `E:\msys64`. For a different path, set `MSYS2_ROOT` (e.g. `setx MSYS2_ROOT F:\dev\msys64`).

### Linux

- Stable Rust toolchain. The `rust-toolchain.toml` pin names a Windows host triple; on Linux override it with `rustup override set stable` in the repo checkout.
- System packages providing headers and static libs for: ffmpeg (libavcodec/libavformat/libavutil/libswscale/libswresample), x264, x265, libpng, libjpeg-turbo, libraw, lcms2, zlib. On Debian/Ubuntu:

  ```
  sudo apt install libavformat-dev libavcodec-dev libavutil-dev libswscale-dev \
                   libswresample-dev libx264-dev libx265-dev \
                   libpng-dev libjpeg-turbo8-dev libraw-dev liblcms2-dev zlib1g-dev
  ```

  (Most distros ship static `.a` archives alongside the shared libs in the `-dev` packages.)

## Build

```
git clone https://github.com/LegeApp/OpenArc.git
cd OpenArc
cargo dist
```

On Windows no `--target` flag is needed — the pinned toolchain already targets `x86_64-pc-windows-gnu`, so `cargo build --release` produces the GNU binary directly.

First build takes ~5 minutes (FreeArc C++ codecs, BPG decoder + encoder, ffmpeg wrapper). Incremental builds re-link only.

## Outputs

- `dist/openarc.exe` or `dist/openarc` — the distribution binary.
- On Windows, `dist/` also contains the handful of MSYS2 runtime DLLs the exe imports (GLib stack, libhwy, shaderc — see above). Ship `dist/` as a unit.
- `target/release/openarc.exe` — the same binary in the standard cargo location.

## Notes on the archive format

- Standard images are converted to BPG and stored in `media/`
- Videos are copied or recompressed into `media/`
- Camera RAW files are copied losslessly into `raw.arc`
- Miscellaneous files are copied losslessly into `misc.arc`
- The outer `.oarc` file is a `.tar.zst` container

On extraction, `raw.arc` is expanded into `raw/` and `misc.arc` is expanded into `misc/`.
