# FFmpeg Minimal Build Instructions

## Configuration for BPG JCTVC Integration

### Codecs Included:
- **Video**: H.264 (libx264), H.265 (libx265) 
- **Audio**: AAC (native, lo-fi), FLAC (hi-fi)

### Build Steps:

#### 1. Open MSYS2 MinGW64 Shell
Start > MSYS2 MinGW64

#### 2. Navigate to FFmpeg directory
```bash
cd /d/misc/arc/openarc/ffmpeg-8.0.1
```

#### 3. Run configuration script
```bash
bash configure_minimal.sh
```

OR manually run:
```bash
./configure \
    --prefix=/d/misc/arc/openarc/ffmpeg-build \
    --enable-static \
    --disable-shared \
    --disable-programs \
    --disable-doc \
    --disable-debug \
    --enable-gpl \
    --enable-version3 \
    --disable-all \
    --enable-avcodec \
    --enable-avformat \
    --enable-avutil \
    --enable-swscale \
    --enable-swresample \
    --enable-libx264 \
    --enable-libx265 \
    --enable-encoder=libx264,libx265,aac,flac \
    --enable-decoder=h264,hevc,aac,flac \
    --enable-parser=h264,hevc,aac,flac \
    --enable-muxer=mp4,matroska,flac \
    --enable-demuxer=h264,hevc,mov,matroska,flac \
    --enable-protocol=file \
    --enable-bsf=h264_mp4toannexb,hevc_mp4toannexb \
    --enable-filter=scale,format,null \
    --arch=x86_64 \
    --target-os=mingw32
```

#### 4. Build FFmpeg (15-30 minutes)
```bash
make -j4
```

#### 5. Install libraries
```bash
make install
```

### Output Location
Libraries will be installed to: `/d/misc/arc/openarc/ffmpeg-build/`
- `lib/` - Static libraries (.a files)
- `include/` - Header files

### Next Steps
After building FFmpeg, rebuild BPG JCTVC encoder with these libraries.
