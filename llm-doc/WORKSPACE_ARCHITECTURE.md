# OpenArc Workspace Architecture

## Overview

OpenArc is structured as a Cargo workspace with multiple subcrates, each serving a specific purpose in the intelligent media archiving system.

## Workspace Structure

```
D:\misc\arc\openarc/
├── Cargo.toml                    # Workspace root configuration
├── README.md                     # User documentation
├── WORKSPACE_ARCHITECTURE.md     # This file
│
├── arcmax/                       # FreeARC Rust port
│   ├── Cargo.toml
│   ├── build.rs                  # C++ codec compilation
│   ├── freearc_cpp_lib/          # Original FreeARC C++ code
│   ├── codec_staging/            # Pre-built GCC libraries
│   └── src/
│       ├── lib.rs                # Library interface
│       ├── main.rs               # CLI tool
│       └── ...                   # Codec implementations
│
├── zstd-archive/                 # Zstandard archiving
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                # Zstd compression wrapper
│
├── codecs/                       # Media codec implementations
│   ├── Cargo.toml
│   ├── mod.rs                    # Module exports
│   ├── bpg.rs                    # BPG image codec (FFI)
│   ├── ffmpeg.rs                 # FFmpeg video codec (FFI)
│   ├── ffmpeg_wrapper.c          # C wrapper for FFmpeg
│   ├── raw.rs                    # RAW image support
│   └── libraw_sys.rs             # LibRAW FFI bindings
│
├── DocBrakeGUI/                  # C# GUI component
│   ├── DocBrakeGUI.csproj
│   ├── App.xaml / App.xaml.cs
│   ├── MainWindow.xaml / .cs
│   ├── reduced_lege_ffi.dll      # FFI export DLL
│   └── ...                       # Other C# files
│
└── BPG/                          # BPG library source
    └── libbpg-0.9.8/             # BPG codec source
```

## Subcrate Dependencies

├── arcmax (FreeARC compression)
├── zstd-archive (Zstandard compression)
├── codecs (BPG + FFmpeg)
└── DocBrakeGUI (via FFI)

arcmax
└── (standalone, no internal deps)

zstd-archive
└── (standalone, no internal deps)

codecs
└── (standalone, no internal deps)
```

## Data Flow

```
User Input (GUI)
    ↓
openarc-gui (File Selection + Config)
    ↓
File Type Detection
    ↓
    ├─→ Images (JPG, PNG, RAW) → codecs::bpg
    ├─→ Videos (MP4, MOV) → codecs::ffmpeg
    └─→ Other Files → arcmax or zstd-archive
    ↓
Archive Creation (.oarc format)
    ↓
Output File
```

## Technology Stack

### Rust Components
- **openarc-gui**: egui (immediate mode GUI framework)
- **arcmax**: C++ FFI via cc crate, FreeARC codecs
- **zstd-archive**: Pure Rust with zstd crate
- **codecs**: FFI to native libraries (libbpg, libavcodec)

### C# Component
- **DocBrakeGUI**: WPF/XAML with .NET 8.0
- **FFI**: Exposed via `reduced_lege_ffi.dll`

### Native Libraries
- **libbpg**: BPG image codec (C)
- **FFmpeg**: Video encoding/decoding (C)
- **FreeARC**: Compression codecs (C++)

## Build System

### Rust Build
- Workspace uses unified dependency versions
- Each subcrate can be built independently
- `build.rs` scripts handle native library compilation

### C# Build
- Separate .NET project in `DocBrakeGUI/`
- Builds to DLL for FFI integration
- Must be built before running openarc-gui

## FFI Integration Points

### Rust → C++ (arcmax)
- FreeARC codecs compiled via `build.rs`
- Static linking of `libfreearc.a`
- Rust wrapper provides safe API

### Rust → C (codecs)
- BPG: Direct FFI to `libbpg.a`
- FFmpeg: FFI via `ffmpeg_wrapper.c`
- LibRAW: FFI for RAW image support

### Rust → C# (openarc-gui → DocBrakeGUI)
- Dynamic loading of `reduced_lege_ffi.dll`
- Function pointers via libloading crate
- Marshaling handled in `ffi.rs`

## Configuration Management

### User Configuration
- Stored in: `%APPDATA%\openarc\config.json`
- Format: JSON with serde
- Contains: compression presets, default paths

### Build Configuration
- Workspace dependencies in root `Cargo.toml`
- Per-crate features in subcrate `Cargo.toml`
- Native library paths in `build.rs`

## File Format: .oarc Archive

```
OpenArc Archive (.oarc)
├── Header (64 bytes)
│   ├── Magic: "OARC" (4 bytes)
│   ├── Version: 1 (2 bytes)
│   ├── File count: N (4 bytes)
│   └── ...
├── File Table (variable)
│   ├── File 1 metadata
│   │   ├── Filename
│   │   ├── Original size
│   │   ├── Compressed size
│   │   ├── Codec type (BPG/FFmpeg/ArcMax/Zstd)
│   │   └── Data offset
│   └── ...
└── Data Streams
    ├── File 1 compressed data
    └── ...
```

## Development Workflow

### Adding a New Subcrate
1. Create directory under workspace root
2. Add to `members` in root `Cargo.toml`
3. Create `Cargo.toml` with `version.workspace = true`
4. Implement functionality
5. Add dependency in `openarc-gui/Cargo.toml`

### Adding Codec Support
1. Add FFI bindings in `codecs/`
2. Update file type detection
3. Add routing logic in processor
4. Update UI configuration options

## Testing Strategy

### Unit Tests
- Each subcrate has its own tests
- Run with `cargo test -p <crate-name>`

### Integration Tests
- Test codec integration in `codecs/`
- Test archive creation/extraction
- Test GUI workflow (manual)

### Performance Tests
- Compression ratio measurements
- Speed benchmarks
- Memory usage profiling

## Future Enhancements

### Planned Subcrates
- `openarc-cli`: Command-line interface
- `openarc-core`: Shared core functionality
- `openarc-formats`: Archive format definitions


## Notes

- All subcrates use workspace-level dependencies where possible
- Each subcrate is independently buildable and testable
- FFI boundaries are clearly defined and documented
- Configuration is centralized in openarc-gui
- Native libraries are statically linked where possible
