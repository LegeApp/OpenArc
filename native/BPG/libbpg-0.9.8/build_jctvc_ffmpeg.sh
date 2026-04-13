#!/bin/bash
# Build BPG JCTVC with FFmpeg 8.0.1 libraries
# Run from MSYS2 MinGW64 environment

set -e

echo "========================================"
echo "Building BPG JCTVC with FFmpeg 8.0.1"
echo "========================================"
echo ""

# Set paths
FFMPEG_DIR="../../ffmpeg-build"
FFMPEG_INCLUDE="-I${FFMPEG_DIR}/include"
FFMPEG_LIBS="-L${FFMPEG_DIR}/lib -lavcodec -lavutil -lavformat -lswscale -lswresample"

# Base compiler flags
BASE_CFLAGS_COMMON="-Os -Wall -fdata-sections -ffunction-sections"
BASE_CFLAGS_COMMON="$BASE_CFLAGS_COMMON -fno-math-errno -fno-signed-zeros -fno-tree-vectorize -fomit-frame-pointer"
BASE_CFLAGS_COMMON="$BASE_CFLAGS_COMMON -D_FILE_OFFSET_BITS=64 -D_LARGEFILE_SOURCE -D_REENTRANT"
BASE_CFLAGS_COMMON="$BASE_CFLAGS_COMMON -I. -DCONFIG_BPG_VERSION=\"0.9.8\""

BASE_CFLAGS_C="$BASE_CFLAGS_COMMON -fno-asynchronous-unwind-tables"
BASE_CFLAGS_CXX="$BASE_CFLAGS_COMMON"

# JCTVC specific flags
JCTVC_CFLAGS="$FFMPEG_INCLUDE -I./jctvc -I./jctvc/TLibCommon -I./jctvc/TLibEncoder -I./jctvc/TLibVideoIO -I./jctvc/libmd5"
JCTVC_CFLAGS="$JCTVC_CFLAGS -Wno-sign-compare -Wno-unused-parameter -Wno-missing-field-initializers"
JCTVC_CFLAGS="$JCTVC_CFLAGS -Wno-misleading-indentation -Wno-class-memaccess"
JCTVC_CFLAGS="$JCTVC_CFLAGS -DMSYS_PROJECT -D_MSYS2 -D_CRT_SECURE_NO_DEPRECATE -D_CRT_SECURE_NO_WARNINGS"
JCTVC_CFLAGS="$JCTVC_CFLAGS -D_CRT_NONSTDC_NO_WARNINGS -D_WIN32_WINNT=0x0600 -DUSE_JCTVC"
JCTVC_CFLAGS="$JCTVC_CFLAGS -D_ISOC99_SOURCE -D_GNU_SOURCE -DHAVE_STRING_H -DHAVE_STDINT_H"
JCTVC_CFLAGS="$JCTVC_CFLAGS -DHAVE_INTTYPES_H -DHAVE_MALLOC_H -D__STDC_LIMIT_MACROS"

CXXFLAGS="$BASE_CFLAGS_CXX $JCTVC_CFLAGS -std=c++11"

# Clean previous build
echo "Cleaning previous build..."
find jctvc -name "*.o" -delete 2>/dev/null || true
rm -f jctvc/libjctvc.a 2>/dev/null || true
rm -f jctvc_glue.o bpgenc.o bpgenc-jctvc.exe 2>/dev/null || true

echo ""
echo "Compiling JCTVC TLibCommon..."
for f in jctvc/TLibCommon/*.cpp; do
    echo "  $(basename $f)"
    g++ $CXXFLAGS -c "$f" -o "${f%.cpp}.o"
done

echo ""
echo "Compiling JCTVC TLibEncoder..."
for f in jctvc/TLibEncoder/*.cpp; do
    echo "  $(basename $f)"
    g++ $CXXFLAGS -c "$f" -o "${f%.cpp}.o"
done

echo ""
echo "Compiling JCTVC TLibVideoIO..."
for f in jctvc/TLibVideoIO/*.cpp; do
    echo "  $(basename $f)"
    g++ $CXXFLAGS -c "$f" -o "${f%.cpp}.o"
done

echo ""
echo "Compiling JCTVC libmd5..."
for f in jctvc/libmd5/*.c; do
    echo "  $(basename $f)"
    gcc $BASE_CFLAGS_C $JCTVC_CFLAGS -c "$f" -o "${f%.c}.o"
done

echo ""
echo "Compiling JCTVC main files..."
g++ $CXXFLAGS -c jctvc/TAppEncCfg.cpp -o jctvc/TAppEncCfg.o
g++ $CXXFLAGS -c jctvc/TAppEncTop.cpp -o jctvc/TAppEncTop.o
g++ $CXXFLAGS -c jctvc/program_options_lite.cpp -o jctvc/program_options_lite.o

echo ""
echo "Creating JCTVC static library..."
find jctvc -name "*.o" > objfiles.txt
ar rcs jctvc/libjctvc.a $(cat objfiles.txt)
rm objfiles.txt

# Verify library was created
if [ ! -s jctvc/libjctvc.a ]; then
    echo "ERROR: JCTVC library is empty or missing!"
    exit 1
fi

echo "JCTVC library size: $(ls -lh jctvc/libjctvc.a | awk '{print $5}')"

echo ""
echo "Compiling jctvc_glue.cpp..."
g++ $CXXFLAGS -c jctvc_glue.cpp -o jctvc_glue.o

echo ""
echo "Compiling bpgenc.c with JCTVC support..."
gcc $BASE_CFLAGS_C $FFMPEG_INCLUDE -DUSE_JCTVC -c bpgenc.c -o bpgenc.o

echo ""
echo "Linking bpgenc-jctvc.exe with FFmpeg libraries..."
g++ -o bpgenc-jctvc.exe \
    bpgenc.o \
    jctvc_glue.o \
    jctvc/libjctvc.a \
    $FFMPEG_LIBS \
    -lx264 -lx265 \
    -lpng -ljpeg -lz \
    -lm -lstdc++ -lpthread \
    -lbcrypt -lole32 -lstrmiids -luuid -loleaut32 \
    -lshlwapi -lpsapi -ladvapi32 -lshell32 \
    -lws2_32 -luser32 -lwinmm

echo ""
echo "========================================"
echo "Build Complete!"
echo "========================================"
echo ""
echo "bpgenc-jctvc.exe created with:"
echo "  - JCTVC encoder (better compression)"
echo "  - FFmpeg 8.0.1 libraries"
echo "  - H.264/H.265 support"
echo ""
echo "Test with:"
echo "  ./bpgenc-jctvc.exe -q 25 -o output.bpg input.jpg"
echo ""
