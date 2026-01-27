# OpenArc - Intelligent Media Archiver

OpenArc is a Windows GUI application for intelligently compressing and archiving phone images, videos, and miscellaneous files using specialized codecs.

## Project Architecture

This is a Cargo workspace with the following structure:

```
openarc/
├── openarc-gui/          # Main Windows GUI application (Rust + egui)
├── arcmax/               # FreeARC Rust port for general file compression
├── zstd-archive/         # Zstandard archiving library
├── codecs/               # BPG (images) and FFmpeg (videos) codec implementations
└── DocBrakeGUI/          # C# GUI component (FFI integration)
```

## Subcrates

### 1. openarc-gui (Main Application)
- **Purpose**: Windows GUI application for user interaction
- **Technology**: Rust + egui (immediate mode GUI)
- **Features**:
  - Drag-and-drop file/folder selection
  - Configuration UI for compression settings
  - Progress tracking and status display
  - FFI integration with DocBrakeGUI C# component
  - Automatic file type detection and codec routing

### 2. arcmax (FreeARC Port)
- **Purpose**: General-purpose file compression using FreeARC codecs
- **Technology**: Rust with C++ FFI bindings
- **Codecs**: LZMA2, LZP, Tornado, and other FreeARC compression methods
- **Use Case**: Compress miscellaneous files (documents, archives, etc.)

### 3. zstd-archive
- **Purpose**: Zstandard compression for fast archiving
- **Technology**: Pure Rust using zstd crate
- **Features**:
  - Fast compression/decompression
  - Configurable compression levels
  - File and in-memory compression support

### 4. codecs
- **Purpose**: Specialized codecs for media files
- **Components**:
  - **BPG**: Image compression (better than JPEG, supports lossless)
  - **FFmpeg**: Video compression with optimized presets
  - **RAW**: RAW image format support (CR2, NEF, ARW, etc.)
- **Technology**: Rust FFI bindings to native libraries

### 5. DocBrakeGUI (C# Component)
- **Purpose**: Document processing and UI enhancements
- **Technology**: C# WPF/XAML
- **Integration**: Exposed via FFI (reduced_lege_ffi.dll)
- **Location**: `DocBrakeGUI/` folder

## Building the Project

### Prerequisites
- Rust toolchain (latest stable)
- MSVC or MinGW-w64 (for C++ compilation)
- .NET SDK 8.0+ (for DocBrakeGUI)

### Build Commands

```powershell
# Build entire workspace
cargo build --release

# Build specific subcrate
cargo build -p openarc-gui --release
cargo build -p arcmax --release
cargo build -p zstd-archive --release
cargo build -p codecs --release

# Run the GUI application
cargo run -p openarc-gui --release
```

### Build DocBrakeGUI C# Component

```powershell
cd DocBrakeGUI
dotnet build -c Release
```

## Usage

1. **Launch the GUI**:
   ```powershell
   cargo run -p openarc-gui --release
   ```

2. **Add Files/Folders**:
   - Click "Add Files" or "Add Folder" to select content
   - Or drag and drop files into the application

3. **Configure Compression**:
   - **Images**: Set BPG quality (0-51) and lossless mode
   - **Videos**: Choose preset (Phone/Camera/Fast/Quality) and CRF
   - **Archives**: Select compression method (ArcMax or Zstandard)

4. **Start Compression**:
   - Select output archive location (.oarc file)
   - Click "Start Compression"
   - Monitor progress in real-time

## File Type Routing

OpenArc automatically detects file types and routes them to the appropriate codec:

- **Images** (JPG, PNG, TIFF, BMP, WebP, RAW) → **BPG codec**
- **Videos** (MP4, MOV, AVI, MKV) → **FFmpeg codec**
- **Other files** → **ArcMax or Zstandard**

## Compression Presets

### Image Compression (BPG)
- **Quality**: 0-51 (lower = better quality, larger size)
- **Lossless**: Preserve original quality (recommended for RAW files)
- **Typical**: QP 23 for good balance

### Video Compression (FFmpeg)
- **Phone**: H.264, CRF 23, optimized for phone videos
- **Camera**: H.265, CRF 20, high quality for camera footage
- **Fast**: H.264, CRF 28, quick compression
- **Quality**: H.265, CRF 18, best quality (slow)

### Archive Compression
- **ArcMax**: FreeARC codecs (LZMA2, best compression ratio)
- **Zstandard**: Fast compression with good ratio

## Development

### Project Structure

```
openarc-gui/src/
├── main.rs           # Entry point
├── app.rs            # Main application UI
├── config.rs         # Configuration management
├── processor.rs      # File processing logic
└── ffi.rs            # C# FFI integration

arcmax/src/
├── lib.rs            # Library interface
├── main.rs           # CLI tool
└── ...               # FreeARC codec implementations

zstd-archive/src/
└── lib.rs            # Zstd compression wrapper

codecs/
├── mod.rs            # Module exports
├── bpg.rs            # BPG codec FFI
├── ffmpeg.rs         # FFmpeg codec FFI
└── raw.rs            # RAW image support
```

### Adding New Features

1. **New Codec**: Add to `codecs/` subcrate
2. **New Archive Format**: Extend `processor.rs` in openarc-gui
3. **UI Enhancement**: Modify `app.rs` in openarc-gui
4. **C# Integration**: Update FFI bindings in `ffi.rs`

## Testing

```powershell
# Test all subcrates
cargo test --workspace

# Test specific subcrate
cargo test -p zstd-archive
cargo test -p arcmax
```

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Please ensure:
- Code compiles without warnings
- Tests pass
- Follow existing code style
- Update documentation as needed
