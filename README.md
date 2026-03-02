# OpenARC

![OpenARC Screenshot](./Screenshot%202026-01-28%20094946.png)

**OpenARC** is a media archival tool designed to automatically compress and archive phone photos and videos with minimal quality loss. It converts images to BPG format (superior to HEIC) and compresses videos using ffmpeg, creating efficient archives that can reduce a 2.5MB JPEG to just 200KB with no perceptible quality loss.

## Features

### 🎨 Media Processing
- **Advanced Image Compression**: Converts images to BPG format using HEIC codec
  - Supports: JPEG, PNG, HEIC/HEIF, RAW (DNG, CR2, NEF, ARW, ORF), TIFF, BMP, WebP, JPEG2000
  - 90%+ size reduction with no visible quality loss
  - Configurable quality levels and bit depths (8-14 bit)

- **Video Compression**: Automatic H.264/H.265 encoding with ffmpeg
  - Smart detection of already-compressed videos
  - Configurable CRF and speed presets
  - Preserves metadata

### 🗂️ Archive Management
- **Dual Archive Formats**:
  - `.oarc` - ArcMax format with FreeArc codecs (high compression)
  - `.zstd` - Fast ZSTD compression for mixed content
- **Deduplication**: Hash-based duplicate detection across archives
- **Catalog System**: SQLite-based tracking of archived files
- **Phone Auto-Detection**: Automatic MTP device detection and staging

### 🖥️ User Interface
- **Modern WPF GUI** (DocBrakeGUI):
  - Media browser with thumbnail/stats toggle
  - Drag-and-drop support
  - Real-time processing queue
  - Settings management
  - MTP device integration

- **Stats Mode**: NEW! Disable thumbnails to view file metadata
  - 📄 Filename and format
  - 📐 Resolution
  - 💾 File size
  - 📅 Last modified
  - ⏱️ Duration (videos)
  - 📊 Bitrate (videos)

- **Interactive CLI Wizard**: Drag-and-drop files, configure settings, and process with just 4 Enter presses
- **Advanced CLI**: Full command-line interface for batch processing, automation, and scripting

## Why OpenARC?

Modern smartphones (Android/iPhone) use poor compression in their camera apps. A typical photo library can balloon to hundreds of gigabytes. OpenARC solves this by:

1. **Massive Space Savings**: 80-95% reduction in archive size
2. **Quality Preservation**: Visually lossless compression
3. **Automation**: Set it and forget it - no manual steps
4. **Flexibility**: Both GUI and CLI for different workflows

## Installation

### Prerequisites

**Required**:
- Windows 10/11 (x64)
- .NET 8.0 SDK (for building from source)
- Rust 1.70+ (for building from source)
- Visual Studio Build Tools (C++ components)

**Optional**:
- ffmpeg/ffprobe (for video processing and stats extraction)
- Git (for cloning repository)

### Pre-built Releases

Download the latest release from [Releases](https://github.com/LegeApp/OpenArc/releases):
- `DocBrakeGUI.exe` - Self-contained GUI (no .NET runtime needed)
- `openarc.exe` - CLI tool
- Required DLLs: `openarc_ffi.dll`, `bpg_viewer.dll`, `openjp2.dll`, `openarc_bpg.dll`

Extract all files to a folder and run `DocBrakeGUI.exe` or `openarc.exe`.

### Building from Source

Use the automated build script:

```powershell
# Clone the repository
git clone https://github.com/LegeApp/OpenArc.git
cd OpenArc

# Build everything (Debug mode)
.\build-all.ps1

# Build Release version
.\build-all.ps1 -Release
```

The script will:
1. Build BPG codec dependencies
2. Build FreeArc compression codecs
3. Build Rust workspace (CLI, core libraries, FFI bindings)
4. Build DocBrakeGUI as self-contained executable

**Build outputs**: `Release/` folder
- `DocBrakeGUI.exe` - GUI application (~150MB self-contained)
- `openarc.exe` - CLI tool (~30MB)

## Usage

### GUI Application

1. **Launch DocBrakeGUI.exe**
2. **Browse Media**:
   - Click "Open Folder" or "Open File"
   - Or drag-and-drop folders/files
3. **Select Files**: Check boxes next to files/folders to process
4. **Configure Settings** (gear icon):
   - Photo: BPG quality, bit depth, encoder type
   - Video: Codec (H.264/H.265), CRF, speed preset
   - Archive: Compression level, catalog, deduplication
   - UI: Toggle thumbnails on/off
5. **Process**: Click "Start Processing" to create archive

### CLI Usage

**Interactive Mode** (Recommended for beginners):
```bash
# Launch wizard - just run with no arguments
openarc

# Follow prompts:
# 1. Drag-and-drop or paste file/folder paths
# 2. Configure settings (or press Enter for defaults)
# 3. Select mode (encode-only or encode+archive)
# 4. Choose output location
# 5. Press Enter to start!
```

**Command Line Mode** (For automation/scripting):
```bash
# Create archive with defaults
openarc create -o my-archive.oarc ~/Pictures ~/Videos

# Custom settings
openarc create \
  -o archive.oarc \
  --bpg-quality 25 \
  --video-preset 1 \
  --compression-level 22 \
  ~/Photos

# Extract archive
openarc extract -i archive.oarc -o output_folder

# List contents
openarc list archive.oarc
```

See **[CLI_GUIDE.md](CLI_GUIDE.md)** for detailed documentation and examples.

## Project Structure

```
OpenArc/
├── DocBrakeGUI/           # WPF GUI application (C#)
│   ├── MediaBrowser/      # Media browsing and thumbnails
│   ├── Services/          # Core services (MTP, processing, etc.)
│   └── ViewModels/        # MVVM view models
├── src/                   # Rust CLI application
├── openarc-ffi/          # FFI bindings for C# interop
├── bpg-viewer/           # BPG/HEIC/RAW image decoder
├── arcmax/               # FreeArc compression library
├── codecs/               # Native codec bindings
├── zune-image/           # Image processing library (submodule)
├── dng-rs/               # RAW image processing
└── openjp2/              # JPEG2000 codec (submodule)
```

## Configuration

### Settings File

Settings are stored in:
- **GUI**: `%APPDATA%/DocBrake/settings.json`
- **CLI**: `~/.openarc/config.toml`

### Available Settings

| Setting | Description | Default |
|---------|-------------|---------|
| `bpg_quality` | BPG quality (0-51, lower = better) | 28 |
| `bpg_bit_depth` | Bit depth (8-14) | 8 |
| `bpg_encoder_type` | 0=x265 (fast), 1=JCTVC (slow/best) | 0 |
| `video_codec` | H264 or H265 | H264 |
| `video_crf` | Video quality (0-51, lower = better) | 23 |
| `video_speed` | Fast, Medium, Slow | Medium |
| `compression_level` | Archive compression (1-22) | 22 |
| `enable_catalog` | Track files in SQLite DB | true |
| `enable_dedup` | Deduplicate files | true |
| `enable_thumbnails` | Show thumbnails vs stats | true |

## Technical Details

### Supported Formats

**Input Formats**:
- **Images**: JPEG, PNG, BPG, HEIC/HEIF, TIFF, BMP, GIF, WebP, JPEG2000 (JP2/J2K)
- **RAW**: DNG, CR2 (Canon), NEF (Nikon), ARW (Sony), ORF (Olympus), RW2, RAF, etc.
- **Videos**: MP4, MOV, AVI, MKV, WebM, M4V, WMV

**Output Formats**:
- **Images**: BPG (HEIC-based)
- **Videos**: MP4 (H.264/H.265)
- **Archives**: .oarc (ArcMax) or .zstd

### Compression Codecs

- **BPG**: Custom-built libbpg with x265/JCTVC encoders
- **ArcMax**: FreeArc compression (LZMA2, PPMD, Tornado, LZP)
- **ZSTD**: Fast compression for mixed content
- **Video**: ffmpeg with libx264/libx265

### Performance

- **Multi-threaded**: Parallel processing of files
- **GPU Thumbnails**: Optional GPU-accelerated thumbnail generation
- **Streaming**: Memory-efficient streaming for large files
- **Caching**: Disk-based thumbnail cache

## Troubleshooting

### Common Issues

**"Thumbnails causing crashes"**:
- Disable thumbnails in Settings → Archive & UI → Uncheck "Enable Thumbnails"
- Stats mode provides metadata without thumbnail generation

**"Video stats not showing"**:
- Install ffmpeg/ffprobe and add to PATH
- Download from: https://ffmpeg.org/download.html

**"MTP device not detected"**:
- Enable "Auto-detect phones" in Settings
- Restart application after connecting device
- Check Windows Device Manager for MTP driver

**Build failures**:
- Ensure Visual Studio Build Tools (C++) installed
- Run build script as Administrator if permission errors
- Check Rust toolchain: `rustup update`

## Contributing

Contributions welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Setup

```powershell
# Clone with submodules
git clone --recursive https://github.com/LegeApp/OpenArc.git
cd OpenArc

# Update submodules
git submodule update --init --recursive

# Build in debug mode
.\build-all.ps1

# Run tests
cargo test --workspace
dotnet test DocBrakeGUI/DocBrakeGUI.csproj
```

## License

This project is dual-licensed under:
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

You may choose either license for your use.

## Credits

- **BPG Codec**: Based on Fabrice Bellard's libbpg
- **FreeArc**: Bulat Ziganshin's compression library
- **libheif**: HEIF/HEIC decoder
- **OpenJPEG**: JPEG2000 codec
- **zune-image**: Fast image processing library
- **rawloader**: RAW image decoder

---

**Star ⭐ this repository** if you find it useful!

**Report issues**: https://github.com/LegeApp/OpenArc/issues

## Linux Backend Build (CLI Only)

The WPF GUI (`DocBrakeGUI`) is Windows-only. On Linux, build and use the CLI/backend only.

Use the Linux build helper:

```bash
cd OpenArc
./build-linux-backend.sh --release
```

This script intentionally:
- Builds backend native libs only (`BPG/libbpg-0.9.8`, `arcmax` codec staging)
- Builds `openarc` CLI only (skips `DocBrakeGUI` and `openarc-ffi`/MTP)
- Checks required native dependencies (`x265`, `libpng`, `libjpeg`, `lcms2`)
- Validates Rust toolchain compatibility (`rustc >= 1.92`)

Notes:
- If your Rust toolchain is older, update with `rustup update stable`.
- If you only want to rebuild Rust crates and keep existing native libs, use:
  `./build-linux-backend.sh --release --skip-codecs`
