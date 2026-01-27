#!/bin/bash
# Complete FFmpeg build script
# Run from MSYS2 MinGW64 environment

set -e

echo "========================================"
echo "FFmpeg Minimal Build"
echo "========================================"
echo ""
echo "Video: H.264, H.265"
echo "Audio: AAC, FLAC"
echo ""

# Clean previous build
if [ -f "config.h" ]; then
    echo "Cleaning previous configuration..."
    make distclean 2>/dev/null || true
fi

echo "Configuring FFmpeg..."
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

if [ $? -ne 0 ]; then
    echo "Configuration failed!"
    exit 1
fi

echo ""
echo "Building FFmpeg (this will take 15-30 minutes)..."
make -j4

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

echo ""
echo "Installing FFmpeg..."
make install

if [ $? -ne 0 ]; then
    echo "Installation failed!"
    exit 1
fi

echo ""
echo "========================================"
echo "Build Complete!"
echo "========================================"
echo ""
echo "Installation: /d/misc/arc/openarc/ffmpeg-build"
echo ""

if [ -d "/d/misc/arc/openarc/ffmpeg-build/lib" ]; then
    echo "Libraries built:"
    ls -lh /d/misc/arc/openarc/ffmpeg-build/lib/*.a | awk '{print "  " $9 " (" $5 ")"}'
fi

echo ""
echo "Next: Rebuild BPG JCTVC with these libraries"
