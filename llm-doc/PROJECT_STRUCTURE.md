# OpenArc Project Structure

## Visual Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     OpenArc Windows GUI                          │
│                    (openarc-gui - Rust/egui)                     │
│                                                                   │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────┐   │
│  │ File Select │  │ Config Panel │  │ Progress Tracking   │   │
│  │ (Drag/Drop) │  │ (Settings)   │  │ (Real-time Status)  │   │
│  └─────────────┘  └──────────────┘  └─────────────────────┘   │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │   File Type Detection & Routing       │
        │   (Automatic codec selection)         │
        └───────────┬───────────────────────────┘
                    │
        ┌───────────┼───────────┬───────────────┐
        │           │           │               │
        ▼           ▼           ▼               ▼
┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐
│   BPG    │ │  FFmpeg  │ │  ArcMax  │ │    Zstd      │
│  Codec   │ │  Codec   │ │ (FreeARC)│ │   Archive    │
├──────────┤ ├──────────┤ ├──────────┤ ├──────────────┤
│ Images:  │ │ Videos:  │ │ General: │ │ Fast:        │
│ JPG, PNG │ │ MP4, MOV │ │ Docs,    │ │ Quick        │
│ TIFF,RAW │ │ AVI, MKV │ │ Archives │ │ Compression  │
└──────────┘ └──────────┘ └──────────┘ └──────────────┘
     │            │            │               │
     └────────────┴────────────┴───────────────┘
                    │
                    ▼
        ┌───────────────────────┐
        │  .oarc Archive Format │
        │  (Unified container)  │
        └───────────────────────┘
                    │
                    ▼
        ┌───────────────────────┐
        │   DocBrakeGUI (C#)    │
        │   FFI Integration     │
        │   (Document Processing)│
        └───────────────────────┘
```

## Directory Layout

```
D:\misc\arc\openarc\
│
├── 📄 Cargo.toml                      # Workspace root configuration
├── 📄 README.md                       # User documentation
├── 📄 WORKSPACE_ARCHITECTURE.md       # Technical architecture
├── 📄 BUILD_GUIDE.md                  # Build instructions
├── 📄 QUICKSTART.md                   # Quick start guide
├── 📄 PROJECT_STRUCTURE.md            # This file
│
├── 📁 openarc-gui/                    # Main Windows GUI Application
│   ├── 📄 Cargo.toml                  # GUI dependencies
│   ├── 📄 build.rs                    # Build script (icon embedding)
│   ├── 📄 openarc.rc                  # Windows resource file
│   └── 📁 src/
│       ├── 📄 main.rs                 # Entry point, window setup
│       ├── 📄 app.rs                  # Main UI logic (egui)
│       ├── 📄 config.rs               # Configuration management
│       ├── 📄 processor.rs            # File processing orchestration
│       └── 📄 ffi.rs                  # C# FFI integration layer
│
├── 📁 arcmax/                         # FreeARC Rust Port
│   ├── 📄 Cargo.toml                  # ArcMax dependencies
│   ├── 📄 build.rs                    # C++ codec compilation
│   ├── 📄 build_codecs.bat            # GCC build script
│   ├── 📁 freearc_cpp_lib/            # Original FreeARC C++ source
│   │   ├── 📁 Compression/
│   │   │   ├── LZMA2/                 # LZMA2 codec
│   │   │   ├── LZP/                   # LZP codec
│   │   │   ├── Tornado/               # Tornado codec
│   │   │   ├── Delta/                 # Delta codec
│   │   │   └── ...
│   │   └── freearc_wrapper.cpp        # FFI wrapper
│   ├── 📁 codec_staging/              # Pre-built GCC libraries
│   │   └── libfreearc.a               # Static library
│   └── 📁 src/
│       ├── 📄 lib.rs                  # Library interface
│       ├── 📄 main.rs                 # CLI tool
│       └── ...                        # Codec implementations
│
├── 📁 zstd-archive/                   # Zstandard Archiving
│   ├── 📄 Cargo.toml                  # Zstd dependencies
│   └── 📁 src/
│       └── 📄 lib.rs                  # Zstd wrapper (compress/decompress)
│
├── 📁 codecs/                         # Media Codec Implementations
│   ├── 📄 Cargo.toml                  # Codec dependencies
│   ├── 📄 mod.rs                      # Module exports
│   ├── 📄 bpg.rs                      # BPG image codec (FFI)
│   ├── 📄 ffmpeg.rs                   # FFmpeg video codec (FFI)
│   ├── 📄 ffmpeg_wrapper.c            # C wrapper for FFmpeg
│   ├── 📄 raw.rs                      # RAW image support
│   ├── 📄 libraw_sys.rs               # LibRAW FFI bindings
│   └── 📄 raw_tests.rs                # RAW codec tests
│
├── 📁 DocBrakeGUI/                    # C# GUI Component
│   ├── 📄 DocBrakeGUI.csproj          # .NET project file
│   ├── 📄 App.xaml                    # WPF application
│   ├── 📄 App.xaml.cs
│   ├── 📄 MainWindow.xaml             # Main window UI
│   ├── 📄 MainWindow.xaml.cs
│   ├── 📄 icon.ico                    # Application icon
│   ├── 📄 reduced_lege_ffi.dll        # FFI export DLL
│   ├── 📁 Commands/                   # WPF commands
│   ├── 📁 Controls/                   # Custom controls
│   ├── 📁 Converters/                 # Value converters
│   ├── 📁 Models/                     # Data models
│   ├── 📁 ViewModels/                 # MVVM view models
│   ├── 📁 Views/                      # Additional views
│   ├── 📁 Services/                   # Business logic
│   └── 📁 NativeInterop/              # FFI exports
│
├── 📁 BPG/                            # BPG Library Source
│   └── 📁 libbpg-0.9.8/               # BPG codec source
│       ├── libbpg.a                   # Static library
│       ├── libbpg.h                   # Header file
│       └── ...                        # Source files
│
├── 📁 ffmpeg-8.0.1/                   # FFmpeg Source (optional)
│   └── ...                            # FFmpeg source files
│
└── 📁 target/                         # Build output (generated)
    ├── 📁 debug/                      # Debug builds
    └── 📁 release/                    # Release builds
        └── openarc-gui.exe            # Main executable
```

## Component Relationships

### Dependency Graph

```
openarc-gui
    ├─→ arcmax (FreeARC compression)
    ├─→ zstd-archive (Zstandard compression)
    ├─→ codecs (BPG + FFmpeg)
    └─→ DocBrakeGUI (via FFI DLL)

arcmax
    └─→ freearc_cpp_lib (C++ codecs via FFI)

zstd-archive
    └─→ zstd crate (Rust binding)

codecs
    ├─→ libbpg (C library via FFI)
    ├─→ FFmpeg (C library via FFI)
    └─→ LibRAW (C++ library via FFI)

DocBrakeGUI
    └─→ .NET 8.0 runtime
```

### Data Flow

```
User Input
    ↓
[openarc-gui] File Selection
    ↓
[openarc-gui] File Type Detection
    ↓
    ├─→ Image? → [codecs::bpg] → BPG compressed
    ├─→ Video? → [codecs::ffmpeg] → H.264/H.265 compressed
    ├─→ Other? → [arcmax] or [zstd-archive] → Compressed
    └─→ Document? → [DocBrakeGUI via FFI] → Processed
    ↓
[openarc-gui] Archive Creation
    ↓
.oarc Archive File
```

## File Size Breakdown

### Source Code
- `openarc-gui/src/`: ~1,500 lines Rust
- `arcmax/src/`: ~5,000 lines Rust
- `zstd-archive/src/`: ~100 lines Rust
- `codecs/`: ~2,000 lines Rust
- `DocBrakeGUI/`: ~3,000 lines C#

### Native Libraries
- `arcmax/codec_staging/libfreearc.a`: ~2.5 MB
- `BPG/libbpg-0.9.8/libbpg.a`: ~2.2 MB
- FFmpeg libraries: ~50-100 MB (if statically linked)

### Build Output
- `openarc-gui.exe`: ~5-10 MB (release)
- `reduced_lege_ffi.dll`: ~2 MB

## Technology Stack Summary

| Component | Language | Framework/Library | Purpose |
|-----------|----------|-------------------|---------|
| openarc-gui | Rust | egui, eframe | Windows GUI |
| arcmax | Rust + C++ | FreeARC codecs | General compression |
| zstd-archive | Rust | zstd crate | Fast archiving |
| codecs | Rust + C | libbpg, FFmpeg | Media codecs |
| DocBrakeGUI | C# | WPF/XAML | Document processing |

## Build Artifacts

### Debug Build
```
target/debug/
├── openarc-gui.exe          # Debug executable (~20 MB)
├── openarc-gui.pdb          # Debug symbols
├── arcmax.dll               # Debug library
├── zstd_archive.dll         # Debug library
└── codecs.dll               # Debug library
```

### Release Build
```
target/release/
├── openarc-gui.exe          # Release executable (~5-10 MB)
├── arcmax.rlib              # Static library
├── zstd_archive.rlib        # Static library
└── codecs.rlib              # Static library
```

## Configuration Files

### User Configuration
- Location: `%APPDATA%\openarc\config.json`
- Format: JSON
- Contents: Compression settings, default paths, presets

### Build Configuration
- `Cargo.toml` (workspace): Shared dependencies
- `Cargo.toml` (per crate): Crate-specific settings
- `build.rs`: Native library compilation
- `.cargo/config.toml`: Cargo settings (optional)

## Runtime Dependencies

### Required DLLs (Windows)
- `reduced_lege_ffi.dll` (DocBrakeGUI)
- MSVC runtime (if using MSVC toolchain)
- .NET 8.0 runtime (for C# component)

### Optional DLLs
- FFmpeg DLLs (if using dynamic linking)
- BPG DLLs (if using dynamic linking)

## Development Workflow

### 1. Initial Setup
```
git clone <repo>
cd openarc
cargo build --workspace
```

### 2. Modify GUI
```
cd openarc-gui
# Edit src/app.rs
cargo run
```

### 3. Modify Codecs
```
cd codecs
# Edit bpg.rs or ffmpeg.rs
cargo test
```

### 4. Modify ArcMax
```
cd arcmax
# Edit src/lib.rs
cargo build
```

### 5. Full Rebuild
```
cargo clean
cargo build --workspace --release
```

## Testing Structure

```
tests/
├── integration/
│   ├── test_bpg_codec.rs
│   ├── test_ffmpeg_codec.rs
│   ├── test_arcmax.rs
│   └── test_archive_format.rs
├── fixtures/
│   ├── sample.jpg
│   ├── sample.mp4
│   └── sample.txt
└── benchmarks/
    ├── compression_speed.rs
    └── compression_ratio.rs
```

## Performance Characteristics

### Compression Speed (Typical)
- Images (BPG): 1-5 MB/s
- Videos (FFmpeg): 0.5-2x realtime
- Files (ArcMax): 10-50 MB/s
- Files (Zstd): 50-200 MB/s

### Compression Ratio (Typical)
- JPG → BPG: 20-50% reduction
- PNG → BPG: 30-70% reduction
- MP4 → MP4 (re-encode): 30-60% reduction
- Documents (ArcMax): 50-90% reduction
- Documents (Zstd): 40-70% reduction

## Memory Usage

### Typical Usage
- GUI: 50-100 MB
- Image processing: 100-500 MB
- Video processing: 500-2000 MB
- Archive creation: 100-300 MB

### Peak Usage
- Large video (4K): Up to 4 GB
- Batch processing: Scales with parallelism

## Future Expansion Points

### New Subcrates (Planned)
- `openarc-cli`: Command-line interface
- `openarc-core`: Shared core functionality
- `openarc-formats`: Archive format definitions
- `openarc-cloud`: Cloud storage integration

### New Features (Planned)
- Hardware acceleration (NVENC, QSV)
- Multi-threaded compression
- Incremental backups
- Archive encryption
- Metadata preservation
- Deduplication

## Summary

OpenArc is a well-structured Cargo workspace with:
- **4 Rust subcrates** (openarc-gui, arcmax, zstd-archive, codecs)
- **1 C# component** (DocBrakeGUI)
- **Clear separation of concerns** (GUI, codecs, compression)
- **FFI integration** (Rust ↔ C++ ↔ C ↔ C#)
- **Automatic codec routing** based on file type
- **Unified archive format** (.oarc)
- **Windows GUI** with modern UI (egui)
