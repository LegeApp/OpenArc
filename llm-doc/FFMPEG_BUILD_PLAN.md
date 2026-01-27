# FFmpeg Minimal Build Plan

**Objective**: Build FFmpeg from source with only H.264/H.265 codecs for minimal size and dependencies

---

## Why Build From Source?

### Advantages vs Pre-built
1. **Minimal Size**: Only include codecs we need (H.264/H.265)
2. **Static Linking**: No DLL dependencies, easier distribution
3. **Custom Configuration**: Optimize for our specific use case
4. **Control**: No unwanted codecs or features
5. **Compatibility**: Build with exact compiler/toolchain we use

### Target Configuration
- **Codecs**: H.264 (libx264), H.265/HEVC (libx265)
- **Containers**: MP4, MKV (minimal)
- **Libraries**: libavcodec, libavutil, libavformat, libswscale
- **Linking**: Static (no DLL dependencies)
- **Size**: Target < 10MB (vs 50MB+ pre-built)

---

## Build Strategy

### Phase 1: Dependencies
1. **x264** - H.264 encoder (already available via MSYS2)
2. **x265** - H.265 encoder (already built)
3. **NASM/YASM** - Assembly (already installed)

### Phase 2: FFmpeg Configuration
Minimal configure flags:
```bash
./configure \
    --prefix=/d/misc/arc/openarc/ffmpeg \
    --toolchain=gcc-mingw64 \
    --enable-static \
    --disable-shared \
    --disable-all \
    --enable-avcodec \
    --enable-avutil \
    --enable-avformat \
    --enable-swscale \
    --enable-libx264 \
    --enable-libx265 \
    --enable-gpl \
    --enable-nonfree \
    --disable-programs \
    --disable-doc \
    --disable-debug \
    --disable-ffmpeg \
    --disable-ffplay \
    --disable-ffprobe
```

### Phase 3: Build Process
1. Download FFmpeg source (latest stable)
2. Configure with minimal options
3. Build static libraries
4. Test with simple program

---

## Detailed Build Script

### Step 1: Download FFmpeg
```batch
REM download_ffmpeg.bat
echo Downloading FFmpeg source...

cd /d d:\misc\arc\openarc

if not exist ffmpeg_src mkdir ffmpeg_src
cd ffmpeg_src

REM Download latest stable FFmpeg
curl -L -o ffmpeg.tar.bz2 https://ffmpeg.org/releases/ffmpeg-6.1.tar.bz2
tar -xf ffmpeg.tar.bz2
move ffmpeg-6.1 ffmpeg
del ffmpeg.tar.bz2

echo FFmpeg source downloaded to ffmpeg_src/ffmpeg/
```

### Step 2: Build Script
```batch
REM build_ffmpeg_minimal.bat
echo Building minimal FFmpeg with H.264/H.265 only...

setlocal enabledelayedexpansion

REM Paths
set FFMPEG_SRC=d:\misc\arc\openarc\ffmpeg_src\ffmpeg
set FFMPEG_INSTALL=d:\misc\arc\openarc\ffmpeg
set X264_DIR=C:\msys64\mingw64
set X265_DIR=d:\misc\arc\openarc\BPG\libbpg-0.9.8\x265.out\8bit

REM Compiler
set CC=gcc
set CXX=g++
set AR=ar
set STRIP=strip

REM Flags
set CFLAGS=-O3 -static
set LDFLAGS=-static

echo.
echo ========================================
echo Configuring FFmpeg (minimal build)
echo ========================================

cd "%FFMPEG_SRC%"

"%FFMPEG_SRC%\configure" ^
    --prefix="%FFMPEG_INSTALL%" ^
    --toolchain=gcc-mingw64 ^
    --enable-static ^
    --disable-shared ^
    --disable-all ^
    --enable-avcodec ^
    --enable-avutil ^
    --enable-avformat ^
    --enable-swscale ^
    --enable-libx264 ^
    --enable-libx265 ^
    --enable-gpl ^
    --enable-nonfree ^
    --disable-programs ^
    --disable-doc ^
    --disable-debug ^
    --disable-ffmpeg ^
    --disable-ffplay ^
    --disable-ffprobe ^
    --pkgconfig-flags="--static" ^
    --extra-cflags="-I%X264_DIR%/include -I%X265_DIR%/include" ^
    --extra-ldflags="-L%X264_DIR%/lib -L%X265_DIR%" ^
    --arch=x86_64 ^
    --target-os=mingw64

if errorlevel 1 goto error

echo.
echo ========================================
echo Building FFmpeg
echo ========================================

make -j4
if errorlevel 1 goto error

echo.
echo ========================================
echo Installing FFmpeg
echo ========================================

make install
if errorlevel 1 goto error

echo.
echo ========================================
echo FFmpeg build complete!
echo ========================================

echo Built libraries:
dir /b "%FFMPEG_INSTALL%\lib\*.a" 2>nul

echo.
echo Library sizes:
for %%f in ("%FFMPEG_INSTALL%\lib\*.a") do (
    echo %%~nxf: %%~zf bytes
)

echo.
echo Headers:
dir /b "%FFMPEG_INSTALL%\include\*.h" 2>nul

goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
exit /b 1

:end
```

### Step 3: Test Build
```c
// test_ffmpeg.c
#include <stdio.h>
#include <libavcodec/avcodec.h>
#include <libavutil/avutil.h>

int main() {
    printf("FFmpeg version: %s\n", av_version_info());
    
    // Test H.264 codec
    const AVCodec *h264 = avcodec_find_encoder(AV_CODEC_ID_H264);
    printf("H.264 codec: %s\n", h264 ? "found" : "not found");
    
    // Test H.265 codec
    const AVCodec *h265 = avcodec_find_encoder(AV_CODEC_ID_HEVC);
    printf("H.265 codec: %s\n", h265 ? "found" : "not found");
    
    return 0;
}
```

```batch
REM test_ffmpeg.bat
gcc -I../ffmpeg/include -L../ffmpeg/lib test_ffmpeg.c -lavcodec -lavutil -o test_ffmpeg.exe
test_ffmpeg.exe
```

---

## Expected Results

### Library Sizes (Target)
```
libavcodec.a    ~3MB  (H.264/H.265 only)
libavutil.a     ~500KB
libavformat.a   ~1MB  (MP4/MKV only)
libswscale.a    ~500KB
Total: ~5MB (vs 50MB+ pre-built)
```

### Features Included
✅ H.264 encoding (libx264)  
✅ H.265 encoding (libx265)  
✅ MP4/MKV container support  
✅ Static linking (no DLLs)  
✅ Minimal dependencies  

### Features Excluded
❌ All other codecs (VP9, AV1, etc.)  
❌ Audio codecs  
❌ Hardware acceleration (can add later)  
❌ FFmpeg CLI tools  
❌ Documentation  

---

## Build Time Estimates

- **Download**: 5-10 minutes (depends on internet)
- **Configure**: 2-3 minutes
- **Build**: 10-20 minutes (4-core)
- **Install**: 1 minute
- **Test**: 2 minutes

**Total**: ~30 minutes

---

## Alternative: Even More Minimal

If we want even smaller, we can disable more:

```bash
--disable-avformat  # No containers, just raw encoding
--disable-swscale   # No scaling
--enable-encoder=libx264,libx265  # Only encoders
--enable-decoder=h264,hevc       # Only decoders
```

**Result**: ~2MB total, but more limited functionality.

---

## Integration with BPG JCTVC

Once FFmpeg is built, update JCTVC build:

```batch
REM build_jctvc_minimal.bat
set FFMPEG_INCLUDE=-I../../ffmpeg/include
set FFMPEG_LIBS=-L../../ffmpeg/lib -lavcodec -lavutil

REM Compile JCTVC with our FFmpeg
g++ %FFMPEG_INCLUDE% -c jctvc/TLibCommon/*.cpp
g++ %FFMPEG_INCLUDE% -c jctvc/TLibEncoder/*.cpp

REM Link with our minimal FFmpeg
ar rcs libjctvc.a obj/*.o
```

Benefits:
- Compatible FFmpeg versions
- Minimal dependencies
- No version conflicts
- Smaller final binary

---

## Troubleshooting

### Common Issues

1. **x264 not found**
   ```bash
   pacman -S mingw-w64-x86_64-x264
   ```

2. **x265 not found**
   - We already built it, just need proper path

3. **Linker errors**
   - Check library paths in --extra-ldflags
   - Ensure static linking flags

4. **Missing headers**
   - Check include paths in --extra-cflags
   - Verify x264/x265 development packages

### Debug Build
Add to configure:
```bash
--enable-debug
--disable-optimizations
```

### Verbose Build
```bash
make V=1
```

---

## Success Criteria

✅ FFmpeg builds without errors  
✅ Only H.264/H.265 codecs enabled  
✅ Static libraries created  
✅ Test program finds both codecs  
✅ Total size < 10MB  
✅ JCTVC builds with our FFmpeg  

---

## Next Steps After Build

1. **Create Rust FFI bindings** for our minimal FFmpeg
2. **Update build.rs** to link our FFmpeg libraries
3. **Test video encoding** in Rust
4. **Rebuild JCTVC** with our FFmpeg
5. **Compare results** vs current BPG encoder

---

## Files to Create

1. `download_ffmpeg.bat` - Download source
2. `build_ffmpeg_minimal.bat` - Build configuration
3. `test_ffmpeg.c` - Test program
4. `test_ffmpeg.bat` - Compile and run test

---

## Why This Approach is Better

### Pre-built Issues
- **Size**: 50MB+ with unused codecs
- **Dependencies**: Multiple DLLs required
- **Features**: Many codecs we don't need
- **Version**: May not match our toolchain

### Source Build Benefits
- **Size**: < 10MB with only what we need
- **Static**: No DLL dependencies
- **Control**: Exact codec selection
- **Compatibility**: Built with our tools
- **Learning**: Understand FFmpeg internals

### For OpenArc
- **Perfect Fit**: Only video codecs we need
- **Easy Distribution**: Static linking
- **Future**: Can add features as needed
- **BPG Fix**: Compatible FFmpeg for JCTVC

---

## Timeline

- **Download**: 10 minutes
- **Build**: 30 minutes
- **Test**: 10 minutes
- **Integration**: 30 minutes

**Total**: ~1.5 hours for complete FFmpeg integration

This approach gives us exactly what we need: minimal FFmpeg with H.264/H.265, static linking, and perfect compatibility with our BPG JCTVC build.
