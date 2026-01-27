# FFmpeg Integration Plan for OpenArc

**Created**: January 19, 2026  
**Objective**: Integrate FFmpeg for video compression and fix BPG JCTVC compatibility

---

## Overview

FFmpeg integration serves two critical purposes:
1. **Fix BPG JCTVC Build** - Provide compatible libavutil/libavcodec for JCTVC encoder
2. **Enable Video Compression** - Add HEVC/H.265 video encoding to OpenArc

---

## Why FFmpeg is Needed

### Problem: BPG JCTVC Build Failures
The BPG source includes minimal FFmpeg components (libavutil/libavcodec) that have version compatibility issues:
- `FF_MEMORY_POISON` undefined
- `frame` type conflicts
- `get_frame_defaults` missing

**Root Cause**: BPG's bundled FFmpeg is from ~2014 and incompatible with modern compilers.

**Solution**: Build a complete, modern FFmpeg and use its libraries for both:
- BPG JCTVC encoder (better compression)
- Video encoding in OpenArc

### Benefit: Video Compression
FFmpeg provides production-ready video encoding:
- **HEVC/H.265** - Modern, efficient codec
- **x265 integration** - Same encoder as BPG
- **Hardware acceleration** - NVENC, QSV support
- **Format support** - MP4, MKV, AVI, etc.

---

## FFmpeg Build Strategy

### Option 1: Use Pre-built FFmpeg (RECOMMENDED)
**Pros**:
- Fast setup (minutes vs hours)
- Known working configuration
- Regular updates available

**Cons**:
- Less control over features
- May include unnecessary codecs

**Sources**:
- https://github.com/BtbN/FFmpeg-Builds (MinGW builds)
- https://www.gyan.dev/ffmpeg/builds/ (Windows builds)

### Option 2: Build FFmpeg from Source
**Pros**:
- Full control over features
- Optimized for our use case
- Can match BPG's FFmpeg version if needed

**Cons**:
- Complex build process
- Time-consuming (1-2 hours)
- Many dependencies

**Required for**:
- Custom codec configurations
- Matching BPG's exact FFmpeg version
- Minimal binary size

---

## Recommended Approach: Hybrid

1. **Phase 1**: Use pre-built FFmpeg shared libraries
   - Quick integration
   - Test video encoding
   - Validate approach

2. **Phase 2**: Build minimal FFmpeg static libraries
   - Only required codecs (HEVC, x265)
   - Static linking for distribution
   - Optimized for size

3. **Phase 3**: Fix BPG JCTVC with FFmpeg libraries
   - Link JCTVC against our FFmpeg build
   - Test compression improvements

---

## Implementation Plan

### Step 1: Download Pre-built FFmpeg
```bash
# Download from BtbN/FFmpeg-Builds
# MinGW shared build with GPL codecs
https://github.com/BtbN/FFmpeg-Builds/releases

# Extract to:
D:/misc/arc/openarc/ffmpeg/
```

**Required Files**:
- `avcodec-*.dll` / `libavcodec.a`
- `avutil-*.dll` / `libavutil.a`
- `avformat-*.dll` / `libavformat.a`
- `swscale-*.dll` / `libswscale.a`
- Header files in `include/`

### Step 2: Create FFmpeg Rust Bindings
```rust
// src/codecs/ffmpeg.rs

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};

// FFmpeg structures
#[repr(C)]
pub struct AVCodec { _private: [u8; 0] }

#[repr(C)]
pub struct AVCodecContext { _private: [u8; 0] }

#[repr(C)]
pub struct AVFrame { _private: [u8; 0] }

#[repr(C)]
pub struct AVPacket { _private: [u8; 0] }

extern "C" {
    // Codec functions
    fn avcodec_find_encoder(id: c_int) -> *mut AVCodec;
    fn avcodec_alloc_context3(codec: *const AVCodec) -> *mut AVCodecContext;
    fn avcodec_open2(ctx: *mut AVCodecContext, codec: *const AVCodec, options: *mut c_void) -> c_int;
    
    // Frame functions
    fn av_frame_alloc() -> *mut AVFrame;
    fn av_frame_free(frame: *mut *mut AVFrame);
    
    // Packet functions
    fn av_packet_alloc() -> *mut AVPacket;
    fn av_packet_free(pkt: *mut *mut AVPacket);
    
    // Encoding
    fn avcodec_send_frame(ctx: *mut AVCodecContext, frame: *const AVFrame) -> c_int;
    fn avcodec_receive_packet(ctx: *mut AVCodecContext, pkt: *mut AVPacket) -> c_int;
}
```

### Step 3: Update build.rs
```rust
// Link FFmpeg libraries
println!("cargo:rustc-link-search=native=../ffmpeg/lib");
println!("cargo:rustc-link-lib=dylib=avcodec");
println!("cargo:rustc-link-lib=dylib=avutil");
println!("cargo:rustc-link-lib=dylib=avformat");
println!("cargo:rustc-link-lib=dylib=swscale");

// Copy FFmpeg DLLs to target directory
let ffmpeg_dlls = [
    "avcodec-60.dll",
    "avutil-58.dll",
    "avformat-60.dll",
    "swscale-7.dll",
];
```

### Step 4: Implement Video Encoder
```rust
pub struct FFmpegVideoEncoder {
    codec: *mut AVCodec,
    context: *mut AVCodecContext,
    frame: *mut AVFrame,
    packet: *mut AVPacket,
}

impl FFmpegVideoEncoder {
    pub fn new(width: u32, height: u32, bitrate: u64) -> Result<Self> {
        // Find HEVC encoder
        let codec = unsafe { avcodec_find_encoder(AV_CODEC_ID_HEVC) };
        
        // Allocate context
        let context = unsafe { avcodec_alloc_context3(codec) };
        
        // Configure encoder
        // ...
        
        Ok(Self { codec, context, frame, packet })
    }
    
    pub fn encode_frame(&mut self, rgb_data: &[u8]) -> Result<Vec<u8>> {
        // Convert RGB to YUV
        // Send frame to encoder
        // Receive encoded packet
        // Return compressed data
    }
}
```

### Step 5: Test Video Encoding
```rust
#[test]
fn test_video_encoding() {
    let encoder = FFmpegVideoEncoder::new(1920, 1080, 5_000_000).unwrap();
    
    // Create test frame (solid color)
    let frame_data = vec![128u8; 1920 * 1080 * 3];
    
    let encoded = encoder.encode_frame(&frame_data).unwrap();
    assert!(!encoded.is_empty());
}
```

---

## BPG JCTVC Integration with FFmpeg

### Step 6: Rebuild JCTVC with FFmpeg
```batch
REM build_jctvc_with_ffmpeg.bat

set FFMPEG_INCLUDE=-I..\ffmpeg\include
set FFMPEG_LIBS=-L..\ffmpeg\lib -lavcodec -lavutil

REM Compile JCTVC with FFmpeg headers
g++ %FFMPEG_INCLUDE% -c jctvc/TLibCommon/*.cpp
g++ %FFMPEG_INCLUDE% -c jctvc/TLibEncoder/*.cpp

REM Link with FFmpeg libraries
ar rcs libjctvc.a obj/*.o
```

### Step 7: Build BPG with JCTVC + FFmpeg
```batch
REM build_bpg_jctvc_ffmpeg.bat

REM Compile BPG encoder
gcc -DUSE_JCTVC %FFMPEG_INCLUDE% -c bpgenc.c

REM Link everything together
g++ -o bpgenc_jctvc.exe ^
    bpgenc.o ^
    libjctvc.a ^
    %FFMPEG_LIBS% ^
    -lpng -ljpeg -lz -lstdc++
```

---

## File Structure

```
openarc/
├── ffmpeg/
│   ├── bin/
│   │   ├── ffmpeg.exe
│   │   └── ffprobe.exe
│   ├── lib/
│   │   ├── avcodec.lib / libavcodec.a
│   │   ├── avutil.lib / libavutil.a
│   │   ├── avformat.lib / libavformat.a
│   │   └── swscale.lib / libswscale.a
│   ├── include/
│   │   ├── libavcodec/
│   │   ├── libavutil/
│   │   ├── libavformat/
│   │   └── libswscale/
│   └── bin/ (DLLs)
│       ├── avcodec-60.dll
│       ├── avutil-58.dll
│       ├── avformat-60.dll
│       └── swscale-7.dll
├── BPG/
│   └── libbpg-0.9.8/
│       ├── jctvc/ (rebuilt with FFmpeg)
│       └── bpgenc_jctvc.exe (JCTVC encoder)
└── openarc/
    ├── src/
    │   └── codecs/
    │       ├── ffmpeg.rs (new)
    │       └── bpg_native.rs
    └── build.rs (updated)
```

---

## Testing Strategy

### Test 1: FFmpeg Library Linking
```bash
cd openarc/openarc
cargo build
# Should link FFmpeg libraries without errors
```

### Test 2: Video Encoding
```bash
cargo test test_video_encoding
# Should encode a test frame successfully
```

### Test 3: BPG JCTVC with FFmpeg
```bash
cd ../BPG/libbpg-0.9.8
.\build_jctvc_with_ffmpeg.bat
.\bpgenc_jctvc.exe -q 25 -o test.bpg test.jpg
# Should produce smaller file than x265 version
```

### Test 4: Integration Test
```bash
cd ../../openarc/openarc
cargo run -- encode-video input.mp4 -o output.hevc
# Should compress video using FFmpeg
```

---

## Expected Results

### BPG JCTVC Improvements
- **Compression**: 10-20% smaller files vs x265
- **Quality**: Better at same bitrate
- **Speed**: 2-3x slower (acceptable for archival)

### Video Encoding Capabilities
- **Formats**: MP4, MKV, AVI input/output
- **Codecs**: HEVC/H.265, H.264, VP9
- **Performance**: Hardware acceleration available
- **Quality**: Configurable bitrate/CRF

---

## Dependencies

### Required Packages (MSYS2)
```bash
pacman -S mingw-w64-x86_64-ffmpeg
pacman -S mingw-w64-x86_64-x265
```

### Or Download Pre-built
- FFmpeg: https://github.com/BtbN/FFmpeg-Builds/releases
- Extract to `openarc/ffmpeg/`

---

## Timeline

- **Step 1-2**: Download FFmpeg, create bindings (30 min)
- **Step 3-4**: Update build.rs, implement encoder (1 hour)
- **Step 5**: Test video encoding (30 min)
- **Step 6-7**: Rebuild JCTVC with FFmpeg (1 hour)
- **Testing**: Integration tests (30 min)

**Total**: ~3.5 hours

---

## Success Criteria

✅ FFmpeg libraries link successfully  
✅ Video encoding works in Rust  
✅ BPG JCTVC builds without errors  
✅ JCTVC produces smaller files than x265  
✅ All tests pass  

---

## Next Steps

1. Download pre-built FFmpeg (BtbN/FFmpeg-Builds)
2. Create `src/codecs/ffmpeg.rs` with basic bindings
3. Update `build.rs` to link FFmpeg
4. Test Rust build
5. Implement video encoder wrapper
6. Rebuild JCTVC with FFmpeg libraries
7. Compare compression results

---

## Alternative: Use Existing FFmpeg Rust Crates

Instead of raw FFI, consider using:
- `ffmpeg-next` - High-level FFmpeg bindings
- `ffmpeg-sys-next` - Low-level FFmpeg bindings

**Pros**: Less code, maintained, safer
**Cons**: Less control, may not match our needs

**Recommendation**: Start with raw FFI for learning, migrate to crate if beneficial.
