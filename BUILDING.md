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
MSYS2, C BPG, or bundled FFmpeg toolchain override is required.

## Outputs

- `target/release/openarc`
- `target/release/openarc.exe` on Windows
- `dist/openarc` or `dist/openarc.exe` when using `cargo dist`

## Archive Format

- Standard images are converted to BPG and stored in `media/`.
- Already-compressed videos are bundled into `misc.arc`.
- Videos needing external encoding are staged separately unless using no-zip
  folder output.
- Camera RAW files are copied losslessly into `raw.arc`.
- Miscellaneous files are copied losslessly into `misc.arc`.
- The default `.oarc` output is a `.tar.zst` container.
