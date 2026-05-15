# OpenArc

OpenArc is a CLI-first media archiver for photo and video folders. Point it at one or more folders, and it will convert standard images to BPG, recompress videos when that helps, preserve camera RAW files losslessly, and bundle the result into a single archive with manifests and hashes.

## Archive layout

- `media/`: converted images and processed videos
- `raw.arc`: preserved camera-source files packed with FreeArc
- `misc.arc`: other preserved files packed with FreeArc
- `OPENARC_METADATA.json`: image restoration metadata
- `MANIFEST.txt`: user-facing archive contents
- `HASHES.sha256`: integrity hashes

## Quick start

### Linux

```bash
git clone https://github.com/LegeApp/OpenArc.git
cd OpenArc
./build-linux-backend.sh --release
./target/$(rustc -vV | sed -n 's/^host: //p')/release/openarc create -o archive.oarc ~/Pictures ~/Videos
```

### Windows

```powershell
git clone https://github.com/LegeApp/OpenArc.git
cd OpenArc
.\build-all.ps1 -Release
.\dist\cli-runtime\openarc.exe create -o archive.oarc C:\Photos C:\Videos
```

## CLI usage

```bash
openarc
openarc create -o my-archive.oarc ~/Pictures ~/Videos
openarc extract -i my-archive.oarc -o restored
openarc list my-archive.oarc
```

At the end of archive creation, the CLI prints how many RAW files were preserved separately and their total size.

## Build entrypoints

- Windows: [`build-all.ps1`](/mnt/Samsung980_1TB/Rust-projects/openarc/build-all.ps1)
- Linux: [`build-linux-backend.sh`](/mnt/Samsung980_1TB/Rust-projects/openarc/build-linux-backend.sh)

See [`BUILDING.md`](/mnt/Samsung980_1TB/Rust-projects/openarc/BUILDING.md) for prerequisites and outputs.

## Repository shape

```text
OpenArc/
├── src/                    # CLI and archive orchestration
├── crates/
│   ├── arcmax/            # FreeArc implementation
│   ├── codecs/            # BPG, ffmpeg, HEIC, camera-source support
│   ├── zune-image/        # vendored image workspace
│   └── winmtp/            # Windows MTP helper
├── native/               # vendored native dependencies and build trees
├── openjp2/              # JPEG2000 tooling
└── xtask/                # small cargo helper tasks
```
