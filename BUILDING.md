# Building OpenArc

OpenArc now has one supported build entrypoint per host OS:

- Windows: `build-all.ps1`
- Linux: `build-linux-backend.sh`

Both produce the CLI path only. The GUI projects are not part of the supported build flow.

## Components

- `openarc`: main CLI binary
- `crates/codecs`: BPG, ffmpeg, HEIC, RAW support used by the CLI
- `crates/arcmax`: FreeArc implementation used for `raw.arc` and `misc.arc`
- `native/BPG/libbpg-0.9.8`: native BPG encoder/decoder build tree
- `dist/cli-runtime` on Windows: staged runtime bundle for `openarc.exe`

## Linux

Run:

```bash
./build-linux-backend.sh --release
```

Prerequisites checked by the script:

- `cargo`, `rustc`, `make`, `gcc`, `g++`, `ar`, `ranlib`, `pkg-config`
- `x265`
- `libpng`
- `libjpeg`
- `lcms2`
- FFmpeg development packages: `libavcodec`, `libavformat`, `libavutil`, `libswscale`
- Rust 1.92 or newer

Outputs:

- `target/<host-triple>/release/openarc`
- `dist/linux/openarc_ffmpeg.so`
- `dist/linux/opj_decompress`

## Windows

Run:

```powershell
.\build-all.ps1 -Release
```

What it does:

1. builds the native BPG dependency tree
2. builds ArcMax codec staging libraries
3. builds `openarc.exe`
4. stages a runnable CLI bundle in `dist\cli-runtime`

Expected tools:

- Rust toolchain
- PowerShell
- MSYS2 MinGW toolchain for `gcc`/`objdump`
- FFmpeg/x264/x265 import libraries reachable by `build-cli-runtime.ps1`

Outputs:

- `target\<target>\release\openarc.exe`
- `dist\cli-runtime\openarc.exe`
- `dist\cli-runtime\openarc_bpg.dll`
- `dist\cli-runtime\openarc_ffmpeg.dll`
- dependent MinGW DLLs copied into `dist\cli-runtime`

## Notes on the archive format

- Standard images are converted to BPG and stored in `media/`
- Videos are copied or recompressed into `media/`
- Camera RAW files are copied losslessly into `raw.arc`
- Miscellaneous files are copied losslessly into `misc.arc`
- The outer `.oarc` file is a `.tar.zst` container

On extraction, `raw.arc` is expanded into `raw/` and `misc.arc` is expanded into `misc/`.
