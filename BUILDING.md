# Building OpenArc

Single entrypoint: `cargo dist`.

It runs `cargo build --release` and stages the binary at `dist/openarc.exe` (Windows) or `dist/openarc` (Linux). All codec C/C++ source is compiled in-tree by `build.rs` scripts — no separate codec build step. FFmpeg, x265, x264, BPG, libpng, libjpeg, lcms2, libraw, and the C++ runtime are statically linked into the binary, so the resulting `dist/openarc.exe` has no DLL sidecars.

If you only want the binary at `target/release/openarc.exe` without the dist staging, `cargo build --release` works directly.

## Prerequisites

### Windows (MinGW / GNU toolchain)

- Rust toolchain (stable). The repo's `rust-toolchain.toml` pins the version.
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

- Same Rust toolchain.
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

First build takes ~5 minutes (FreeArc C++ codecs, BPG decoder + encoder, ffmpeg wrapper). Incremental builds re-link only.

## Outputs

- `dist/openarc.exe` or `dist/openarc` — the only file needed for distribution.
- `target/release/openarc.exe` — the same binary in the standard cargo location.

Nothing else is produced. No DLLs, no separate codec libraries.

## Notes on the archive format

- Standard images are converted to BPG and stored in `media/`
- Videos are copied or recompressed into `media/`
- Camera RAW files are copied losslessly into `raw.arc`
- Miscellaneous files are copied losslessly into `misc.arc`
- The outer `.oarc` file is a `.tar.zst` container

On extraction, `raw.arc` is expanded into `raw/` and `misc.arc` is expanded into `misc/`.
