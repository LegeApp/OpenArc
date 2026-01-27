# BPG Build Status for Windows

## ✅ Successfully Compiled

**Date**: January 19, 2026  
**Platform**: Windows with MinGW/MSYS2  
**Library**: libbpg.a (205KB - decoder-only)

## Build Details

### Compiler
- **GCC Version**: 15.2.0 (MSYS2)
- **Make**: mingw32-make 4.4.1

### Build Script
Created custom Windows build script: `build_windows.bat`

### Compilation Flags
```
-Os -Wall -fno-asynchronous-unwind-tables -fdata-sections 
-ffunction-sections -fno-math-errno -fno-signed-zeros 
-fno-tree-vectorize -fomit-frame-pointer
-D_FILE_OFFSET_BITS=64 -D_LARGEFILE_SOURCE -D_REENTRANT
-DCONFIG_BPG_VERSION="0.9.8"
-D_ISOC99_SOURCE -D_POSIX_C_SOURCE=200112 -D_XOPEN_SOURCE=600
-DHAVE_AV_CONFIG_H -std=c99 -D_GNU_SOURCE=1
-DUSE_VAR_BIT_DEPTH -DUSE_PRED
```

### Components Compiled
**libavcodec** (HEVC decoder):
- hevc_cabac.o, hevc_filter.o, hevc.o, hevcpred.o
- hevc_refs.o, hevcdsp.o, hevc_mvs.o, hevc_ps.o
- hevc_sei.o, utils.o, cabac.o, golomb.o, videodsp.o

**libavutil**:
- mem.o, buffer.o, log2_tab.o, frame.o, pixdesc.o, md5.o

**libbpg**:
- libbpg.o (main BPG decoder implementation)

### Warnings (Non-critical)
- `alloc_size` attribute warnings (can be ignored)
- Unused function warnings (can be ignored)
- Parentheses suggestions (can be ignored)

## Current Limitations

### ⚠️ Decoder Only
The current build is **decoder-only**. It can:
- ✅ Decode BPG images to PNG/JPG
- ❌ Encode images to BPG (requires x265 or JCTVC)

### Encoder Support (Not Yet Built)
To enable encoding, we need to:
1. **Option A**: Build with x265 (HEVC encoder)
   - Requires CMake
   - Builds x265 in 8-bit, 10-bit, and 12-bit modes
   - More complex but better performance

2. **Option B**: Build with JCTVC (reference encoder)
   - Simpler to build
   - Slower but no external dependencies
   - Good for testing

## Next Steps

### For OpenArc Development

1. **Test Decoder**
   - Create Rust FFI bindings for `libbpg.a`
   - Test decoding existing BPG files
   - Verify output quality

2. **Build Encoder** (Choose one):
   - **Recommended**: Build x265 encoder for production use
   - **Alternative**: Build JCTVC encoder for testing

3. **Integration**
   - Link `libbpg.a` in Rust build.rs
   - Create codec wrapper in `src/codecs/bpg.rs`
   - Implement image format detection

### Building x265 Encoder (Future)

```bash
# Requires CMake
cd d:\misc\arc\openarc\BPG\libbpg-0.9.8

# Create build directories
mkdir -p x265.out/8bit x265.out/10bit x265.out/12bit

# Build 12-bit
cd x265.out/12bit
cmake ../../x265/source -DHIGH_BIT_DEPTH=ON -DEXPORT_C_API=OFF -DENABLE_SHARED=OFF -DENABLE_CLI=OFF -DMAIN12=ON
mingw32-make

# Build 10-bit
cd ../10bit
cmake ../../x265/source -DHIGH_BIT_DEPTH=ON -DEXPORT_C_API=OFF -DENABLE_SHARED=OFF -DENABLE_CLI=OFF -DMAIN10=ON
mingw32-make

# Build 8-bit (links 10-bit and 12-bit)
cd ../8bit
cmake ../../x265/source -DLINKED_10BIT=ON -DLINKED_12BIT=ON -DENABLE_SHARED=OFF -DENABLE_CLI=OFF
mingw32-make

# Rebuild libbpg with x265 support
cd ../..
# Add x265 object files to build_windows.bat
```

## File Locations

- **Source**: `d:\misc\arc\openarc\BPG\libbpg-0.9.8\`
- **Library**: `d:\misc\arc\openarc\BPG\libbpg-0.9.8\libbpg.a`
- **Header**: `d:\misc\arc\openarc\BPG\libbpg-0.9.8\libbpg.h`
- **Build Script**: `d:\misc\arc\openarc\BPG\libbpg-0.9.8\build_windows.bat`

## Usage in Rust

### build.rs
```rust
fn main() {
    println!("cargo:rustc-link-search=native=BPG/libbpg-0.9.8");
    println!("cargo:rustc-link-lib=static=bpg");
}
```

### FFI Bindings
```rust
extern "C" {
    pub fn bpg_decoder_open() -> *mut BPGDecoderContext;
    pub fn bpg_decoder_decode(s: *mut BPGDecoderContext, buf: *const u8, buf_len: c_int) -> c_int;
    pub fn bpg_decoder_close(s: *mut BPGDecoderContext);
}
```

## Testing

### Test Files
- Use existing BPG files from the web
- Or use `bpgenc.exe` (if available) to create test files

### Verification
```bash
# Test decoder (if bpgdec.exe is available)
bpgdec.exe test.bpg -o test.png
```

## Notes

- BPG uses HEVC (H.265) compression internally
- Provides 20-50% better compression than JPEG
- Supports lossless compression
- Supports alpha channel
- Bit depths: 8, 10, 12, 14 bits
- Color spaces: YCbCr, RGB, YCgCo, CMYK

## References

- BPG Website: https://bellard.org/bpg/
- Source: https://github.com/mirrorer/libbpg
- x265: https://bitbucket.org/multicoreware/x265/
