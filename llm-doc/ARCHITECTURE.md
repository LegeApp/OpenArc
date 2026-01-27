# OpenArc High-Level Architecture Plan

## Project Overview
**OpenArc** - Specialized archiver for phone/camera media files using:
- **BPG** for images (JPGs, RAWs) - excellent compression even lossless
- **FFmpeg** for videos (MP4s) - with optimized presets for phone/camera footage
- **ARC** for other files - general-purpose compression

## Architecture Design

```
┌─────────────────────────────────────────────────────────────┐
│                     OpenArc CLI                              │
│  File type detection → Codec routing → Archive management    │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
┌──────────────┐      ┌──────────────┐      ┌──────────────┐
│  BPG Codec   │      │  FFmpeg Codec│      │  ARC Codec   │
│  (Images)    │      │  (Videos)    │      │  (Files)     │
│  libbpg.a    │      │  libav*.a    │      │  libfreearc.a│
└──────────────┘      └──────────────┘      └──────────────┘
```

## Phase 1: BPG Integration

### 1.1 Analyze BPG Source Structure
- **Location**: `d:\misc\arc\openarc\BPG\libbpg-0.9.8\`
- **Key files**: `libbpg.c`, `libbpg.h`, `bpgenc.c`, `bpgenc.h`
- **Dependencies**: x265 (HEVC encoder), jctvc (HEVC reference encoder)
- **Existing**: `libbpg.a` already compiled (2.2MB)

### 1.2 BPG Static Library Compilation for Windows
**Challenges**:
- BPG uses Makefiles designed for Linux/Unix
- Needs MinGW/MSYS2 or Visual Studio adaptation
- Dependencies: x265, libpng, libjpeg

**Compilation Steps**:
```bash
# Using MSYS2/MinGW on Windows
cd d:\misc\arc\openarc\BPG\libbpg-0.9.8

# Clean previous builds
make clean

# Compile with Windows-compatible flags
make CC=gcc CFLAGS="-O2 -fPIC -DWIN32" AR=ar

# Output: libbpg.a (static library)
```

**Windows-specific modifications needed**:
- Replace Unix-specific headers (unistd.h, etc.)
- Adjust file I/O for Windows paths
- Handle DLL dependencies (libpng, libjpeg, x265)

### 1.3 Rust FFI Bindings for BPG
Create `openarc/src/codecs/bpg.rs`:
```rust
use std::os::raw::{c_int, c_uint, c_void};

// BPG image format
#[repr(C)]
pub struct BPGDecoderContext {
    _private: [u8; 0],
}

#[repr(C)]
pub struct BPGEncoderParameters {
    pub qp: c_int,              // Quantization parameter (0-51, lower = better quality)
    pub alpha_qp: c_int,        // Alpha channel QP
    pub preferred_chroma_format: c_int,
    pub compress_level: c_int,  // 1-9, higher = slower but better compression
    pub lossless: c_int,        // 0 = lossy, 1 = lossless
}

extern "C" {
    // Decoder functions
    pub fn bpg_decoder_open() -> *mut BPGDecoderContext;
    pub fn bpg_decoder_decode(
        s: *mut BPGDecoderContext,
        buf: *const u8,
        buf_len: c_int,
    ) -> c_int;
    pub fn bpg_decoder_get_info(
        s: *mut BPGDecoderContext,
        p_info: *mut BPGImageInfo,
    ) -> c_int;
    pub fn bpg_decoder_start(
        s: *mut BPGDecoderContext,
        out_fmt: c_int,
    ) -> c_int;
    pub fn bpg_decoder_get_line(
        s: *mut BPGDecoderContext,
        buf: *mut c_void,
    ) -> c_int;
    pub fn bpg_decoder_close(s: *mut BPGDecoderContext);
    
    // Encoder functions (if available)
    pub fn bpg_encoder_new() -> *mut c_void;
    pub fn bpg_encoder_set_params(
        enc: *mut c_void,
        params: *const BPGEncoderParameters,
    ) -> c_int;
}

#[repr(C)]
pub struct BPGImageInfo {
    pub width: c_int,
    pub height: c_int,
    pub format: c_int,
    pub has_alpha: c_int,
    pub color_space: c_int,
    pub bit_depth: c_int,
    pub premultiplied_alpha: c_int,
    pub has_w_plane: c_int,
    pub limited_range: c_int,
}
```

### 1.4 BPG Codec Implementation
- **Input formats**: JPG, PNG, TIFF, RAW (CR2, NEF, ARW, etc.)
- **Output format**: BPG (.bpg files)
- **Features**: 
  - Lossless mode for RAW files
  - Quality-based compression for JPGs (QP 0-51)
  - HEVC-based compression (better than JPEG)
  - Alpha channel support

## Phase 2: FFmpeg Integration

### 2.1 FFmpeg Static Library Compilation for Windows
**Options**:
1. Use pre-built static libraries from https://www.gyan.dev/ffmpeg/builds/
2. Compile from source using MSYS2/MinGW

**Using Pre-built Libraries** (Recommended):
```bash
# Download FFmpeg static build
# Extract to openarc/ffmpeg_lib/

# Link in build.rs:
println!("cargo:rustc-link-search=native=ffmpeg_lib/lib");
println!("cargo:rustc-link-lib=static=avcodec");
println!("cargo:rustc-link-lib=static=avformat");
println!("cargo:rustc-link-lib=static=avutil");
println!("cargo:rustc-link-lib=static=swscale");
println!("cargo:rustc-link-lib=static=swresample");
```

### 2.2 FFmpeg Video Presets for Phone/Camera
Create optimized presets:
- **Phone Video Preset**: H.264, CRF 23, 1080p, moderate bitrate
- **Camera Video Preset**: H.265, CRF 20, 4K/1080p, high quality
- **Fast Preset**: Quick compression for large batches (CRF 28)
- **Quality Preset**: Best quality for important footage (CRF 18)

### 2.3 Rust FFI Bindings for FFmpeg
Use existing Rust crates:
- `ffmpeg-sys-next` - Low-level FFmpeg bindings
- Or create custom bindings for specific needs

Create `openarc/src/codecs/ffmpeg.rs`:
```rust
use ffmpeg_sys_next as ffmpeg;

pub struct VideoEncoder {
    codec_ctx: *mut ffmpeg::AVCodecContext,
    format_ctx: *mut ffmpeg::AVFormatContext,
}

pub enum VideoPreset {
    Phone,      // H.264, CRF 23, 1080p
    Camera,     // H.265, CRF 20, 4K
    Fast,       // H.264, CRF 28, fast preset
    Quality,    // H.265, CRF 18, slow preset
}
```

### 2.4 FFmpeg Codec Implementation
- **Input formats**: MP4, MOV, AVI, MKV
- **Output format**: MP4 (H.264/H.265)
- **Features**:
  - Hardware acceleration (NVENC, QSV if available)
  - Preset-based encoding
  - CRF (Constant Rate Factor) for quality control
  - Resolution scaling (optional)
  - Audio passthrough or re-encode

## Phase 3: ARC Integration

### 3.1 Port ARC from arcmax
Copy relevant files:
```
arcmax/freearc_cpp_lib/Compression/LZMA2/ → openarc/freearc_cpp_lib/Compression/LZMA2/
arcmax/freearc_cpp_lib/Compression/LZP/   → openarc/freearc_cpp_lib/Compression/LZP/
arcmax/freearc_cpp_lib/Compression/Tornado/ → openarc/freearc_cpp_lib/Compression/Tornado/
arcmax/freearc_cpp_lib/Compression/Common.* → openarc/freearc_cpp_lib/Compression/
arcmax/freearc_cpp_lib/freearc_wrapper.cpp → openarc/freearc_cpp_lib/
```

### 3.2 Keep Only Essential Codecs
- **LZMA2** (best general compression) - Keep
- **LZP** (fast for text/metadata) - Keep
- **Tornado** (good for mixed content) - Keep
- **Remove**: PPMD (replaced with ppmd-rust), GRZip, 4x4, etc.

### 3.3 Simplify Build System
- Remove unused codec object files
- Streamline Makefile/build.rs
- Only compile needed codecs

## Phase 4: File Type Detection & Codec Routing

### 4.1 File Type Detection
Create `openarc/src/core/filetype.rs`:
```rust
pub enum FileType {
    // Images
    ImageJpg,
    ImagePng,
    ImageTiff,
    ImageBmp,
    ImageWebP,
    ImageRaw(RawFormat),
    
    // Videos
    VideoMp4,
    VideoMov,
    VideoAvi,
    VideoMkv,
    VideoWebM,
    
    // Other
    Other,
}

pub enum RawFormat {
    CR2,    // Canon
    NEF,    // Nikon
    ARW,    // Sony
    DNG,    // Adobe
    RAF,    // Fujifilm
    ORF,    // Olympus
    RW2,    // Panasonic
}

pub fn detect_file_type(data: &[u8], extension: &str) -> FileType {
    // Magic number detection
    if data.len() >= 4 {
        match &data[0..4] {
            [0xFF, 0xD8, 0xFF, _] => return FileType::ImageJpg,
            [0x89, 0x50, 0x4E, 0x47] => return FileType::ImagePng,
            [0x00, 0x00, 0x00, 0x18] | [0x00, 0x00, 0x00, 0x1C] => return FileType::VideoMp4,
            _ => {}
        }
    }
    
    // Extension fallback
    match extension.to_lowercase().as_str() {
        "jpg" | "jpeg" => FileType::ImageJpg,
        "png" => FileType::ImagePng,
        "cr2" => FileType::ImageRaw(RawFormat::CR2),
        "nef" => FileType::ImageRaw(RawFormat::NEF),
        "mp4" => FileType::VideoMp4,
        "mov" => FileType::VideoMov,
        _ => FileType::Other,
    }
}
```

### 4.2 Codec Routing Logic
```rust
pub enum CompressionCodec {
    BPG,
    FFmpeg,
    ARC,
}

pub fn route_to_codec(file_type: FileType) -> CompressionCodec {
    match file_type {
        FileType::ImageJpg | FileType::ImagePng | FileType::ImageTiff 
        | FileType::ImageBmp | FileType::ImageWebP | FileType::ImageRaw(_) => {
            CompressionCodec::BPG
        }
        FileType::VideoMp4 | FileType::VideoMov | FileType::VideoAvi 
        | FileType::VideoMkv | FileType::VideoWebM => {
            CompressionCodec::FFmpeg
        }
        FileType::Other => CompressionCodec::ARC,
    }
}
```

## Phase 5: Unified Archive Format

### 5.1 Archive Structure Design
```
OpenArc Archive (.oarc)
├── Header (64 bytes)
│   ├── Magic: "OARC" (4 bytes)
│   ├── Version: 1 (2 bytes)
│   ├── File count: N (4 bytes)
│   ├── Flags: (2 bytes)
│   └── Reserved: (52 bytes)
├── File Table
│   ├── File 1 metadata (variable size)
│   │   ├── Filename length (2 bytes)
│   │   ├── Filename (UTF-8, variable)
│   │   ├── Original size (8 bytes)
│   │   ├── Compressed size (8 bytes)
│   │   ├── Codec type (1 byte): 0=BPG, 1=FFmpeg, 2=ARC
│   │   ├── Compression params (8 bytes)
│   │   ├── CRC32 checksum (4 bytes)
│   │   ├── Timestamp (8 bytes)
│   │   └── Data offset (8 bytes)
│   ├── File 2 metadata
│   └── ...
└── Data Streams
    ├── File 1 data (compressed with appropriate codec)
    ├── File 2 data
    └── ...
```

### 5.2 Archive Creation
```rust
pub struct ArchiveOptions {
    pub bpg_quality: u32,       // 0-51 for BPG
    pub bpg_lossless: bool,
    pub ffmpeg_preset: VideoPreset,
    pub ffmpeg_crf: u32,        // 0-51 for video
    pub arc_method: String,     // "lzma2", "lzp", "tornado"
}

pub fn create_archive(
    files: Vec<PathBuf>, 
    output: PathBuf, 
    options: ArchiveOptions
) -> Result<()> {
    // 1. Detect file types
    // 2. Route to appropriate codec
    // 3. Compress each file
    // 4. Build archive structure
    // 5. Write to disk with progress tracking
}
```

### 5.3 Archive Extraction
```rust
pub fn extract_archive(archive: PathBuf, output_dir: PathBuf) -> Result<()> {
    // 1. Read header and validate
    // 2. Read file table
    // 3. For each file:
    //    - Read compressed data
    //    - Decompress with appropriate codec
    //    - Restore original filename and timestamp
    //    - Write to disk
}
```

## Phase 6: Project Structure

```
openarc/
├── Cargo.toml
├── build.rs                    # Build script for BPG/FFmpeg/ARC
├── ARCHITECTURE.md             # This file
├── README.md
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── lib.rs                  # Library interface
│   ├── cli.rs                  # Command-line parsing (clap)
│   ├── core/
│   │   ├── mod.rs
│   │   ├── filetype.rs         # File type detection
│   │   ├── archive.rs          # Archive format implementation
│   │   └── routing.rs          # Codec routing logic
│   └── codecs/
│       ├── mod.rs
│       ├── bpg.rs              # BPG codec (FFI)
│       ├── ffmpeg.rs           # FFmpeg codec (FFI)
│       ├── lzma2.rs            # LZMA2 codec
│       ├── lzp.rs              # LZP codec
│       └── tornado.rs          # Tornado codec
├── freearc_cpp_lib/            # ARC codec C++ code
│   ├── Compression/
│   │   ├── LZMA2/
│   │   ├── LZP/
│   │   ├── Tornado/
│   │   └── Common.*
│   └── freearc_wrapper.cpp
├── bpg_lib/                    # BPG static library
│   └── libbpg-0.9.8/
│       ├── libbpg.a
│       ├── libbpg.h
│       └── ...
└── ffmpeg_lib/                 # FFmpeg static libraries
    ├── include/
    │   └── libav*/
    └── lib/
        ├── libavcodec.a
        ├── libavformat.a
        ├── libavutil.a
        ├── libswscale.a
        └── libswresample.a
```

## Phase 7: Build System

### 7.1 build.rs Script
```rust
use std::process::Command;
use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // Build BPG static library
    println!("cargo:rerun-if-changed=bpg_lib/libbpg-0.9.8");
    
    let bpg_dir = "bpg_lib/libbpg-0.9.8";
    Command::new("make")
        .args(&["-C", bpg_dir, "clean"])
        .status()
        .ok(); // Ignore errors on clean
    
    Command::new("make")
        .args(&["-C", bpg_dir])
        .env("CC", "gcc")
        .env("CFLAGS", "-O2 -fPIC -DWIN32")
        .status()
        .expect("Failed to build BPG");
    
    // Link BPG library
    println!("cargo:rustc-link-search=native={}", bpg_dir);
    println!("cargo:rustc-link-lib=static=bpg");
    
    // Link FFmpeg libraries
    println!("cargo:rustc-link-search=native=ffmpeg_lib/lib");
    println!("cargo:rustc-link-lib=static=avcodec");
    println!("cargo:rustc-link-lib=static=avformat");
    println!("cargo:rustc-link-lib=static=avutil");
    println!("cargo:rustc-link-lib=static=swscale");
    println!("cargo:rustc-link-lib=static=swresample");
    
    // Link system libraries (Windows)
    println!("cargo:rustc-link-lib=dylib=ws2_32");
    println!("cargo:rustc-link-lib=dylib=secur32");
    println!("cargo:rustc-link-lib=dylib=bcrypt");
    
    // Build ARC codecs
    cc::Build::new()
        .files(&[
            "freearc_cpp_lib/freearc_wrapper.cpp",
            "freearc_cpp_lib/Compression/Common.cpp",
        ])
        .include("freearc_cpp_lib")
        .flag("-O2")
        .flag("-DWIN32")
        .compile("freearc");
}
```

### 7.2 Cargo.toml Dependencies
```toml
[package]
name = "openarc"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4.5", features = ["derive"] }
anyhow = "1.0"
thiserror = "1.0"
crc32fast = "1.4"
rayon = "1.10"          # Parallel processing
walkdir = "2.5"         # Directory traversal
indicatif = "0.17"      # Progress bars

[build-dependencies]
cc = "1.0"
```

## Phase 8: CLI Interface

### 8.1 Command Structure
```bash
# Create archive
openarc create -o photos.oarc vacation/*.jpg videos/*.mp4

# Create with options
openarc create -o archive.oarc \
    --bpg-quality 90 \
    --bpg-lossless \
    --video-preset camera \
    input_folder/

# Extract archive
openarc extract -i archive.oarc -o output_dir/

# List contents
openarc list archive.oarc

# Add files to existing archive
openarc add -i archive.oarc new_photos/*.jpg

# Convert single image to BPG
openarc convert-bpg -q 90 input.jpg output.bpg

# Compress single video
openarc compress-video --preset phone input.mp4 output.mp4

# Batch convert images
openarc batch-bpg -q 85 --lossless input_folder/ output_folder/
```

## Phase 9: Testing Strategy

### 9.1 Test Files
- **Images**: Phone JPGs (various sizes), RAW files (CR2, NEF, ARW), PNG, TIFF
- **Videos**: Phone MP4s (1080p, 4K), camera MP4s, various codecs
- **Files**: Documents (PDF, DOCX), archives (ZIP), text files

### 9.2 Test Scenarios
1. **BPG compression**: Lossy (various quality levels) and lossless modes
2. **FFmpeg compression**: Different presets, resolutions, and bitrates
3. **ARC compression**: Various file types and sizes
4. **Mixed archive**: Images + videos + files in one archive
5. **Round-trip**: Compress → Decompress → Verify integrity
6. **Large files**: Test with 4K videos and large RAW files
7. **Batch processing**: Multiple files in parallel

### 9.3 Performance Metrics
- Compression ratio (original size / compressed size)
- Compression speed (MB/s)
- Decompression speed (MB/s)
- Memory usage
- Quality loss (for lossy compression)

## Implementation Order

1. **Week 1**: BPG integration
   - Compile libbpg for Windows
   - Create FFI bindings
   - Implement basic BPG codec
   - Test with JPG and PNG files

2. **Week 2**: FFmpeg integration
   - Set up FFmpeg static libraries
   - Create FFI bindings
   - Implement video codec with presets
   - Test with MP4 files

3. **Week 3**: ARC integration
   - Port LZMA2, LZP, Tornado from arcmax
   - Simplify and clean up
   - Test with various file types

4. **Week 4**: File type detection and codec routing
   - Implement magic number detection
   - Implement codec routing logic
   - Test automatic codec selection

5. **Week 5**: Archive format
   - Design and implement .oarc format
   - Implement archive creation
   - Implement archive extraction
   - Test with mixed content

6. **Week 6**: CLI interface
   - Implement command-line parsing
   - Add progress bars and status output
   - Implement batch operations
   - Add error handling

7. **Week 7**: Testing and optimization
   - Comprehensive testing with real files
   - Performance optimization
   - Memory usage optimization
   - Documentation

## Key Challenges & Solutions

### Challenge 1: BPG Compilation on Windows
**Problem**: BPG uses Unix-specific Makefiles and headers
**Solution**: 
- Use MSYS2/MinGW environment
- Modify Makefile for Windows paths
- Replace Unix headers with Windows equivalents
- Use pre-compiled libbpg.a if available

### Challenge 2: FFmpeg Size
**Problem**: FFmpeg static libraries are very large (100+ MB)
**Solution**:
- Only link needed codecs (H.264, H.265)
- Use `--disable-*` flags during compilation
- Strip debug symbols
- Consider dynamic linking for development

### Challenge 3: Memory Usage with Large Files
**Problem**: 4K videos and large RAW files can consume lots of memory
**Solution**:
- Implement streaming/chunked processing
- Use memory-mapped files for large inputs
- Process files in parallel with rayon
- Set memory limits per operation

### Challenge 4: Cross-Platform Support
**Problem**: Windows, macOS, and Linux have different build systems
**Solution**:
- Focus on Windows first (user's platform)
- Use conditional compilation for platform-specific code
- Document build process for each platform
- Consider using Docker for consistent builds

### Challenge 5: Quality vs Speed Trade-off
**Problem**: High-quality compression is slow
**Solution**:
- Provide multiple preset options
- Default to balanced settings
- Allow users to choose speed vs quality
- Use hardware acceleration when available

## Future Enhancements

1. **GUI Interface**: Desktop application with drag-and-drop
2. **Cloud Integration**: Upload/download from cloud storage
3. **Metadata Preservation**: EXIF, XMP, GPS data
4. **Deduplication**: Detect and eliminate duplicate files
5. **Encryption**: Optional AES encryption for sensitive files
6. **Incremental Backups**: Only compress changed files
7. **Multi-threading**: Parallel compression of multiple files
8. **Hardware Acceleration**: GPU encoding for videos (NVENC, QSV)
9. **Mobile App**: Android/iOS companion app
10. **Web Interface**: Browser-based archive viewer

## Notes

- BPG provides better compression than JPEG (20-50% smaller)
- BPG supports lossless compression for RAW files
- FFmpeg with H.265 provides excellent video compression
- ARC codecs (LZMA2) provide best compression for general files
- Archive format is designed to be extensible for future codecs
- Windows build requires MSYS2/MinGW or Visual Studio
