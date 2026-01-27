@echo off
REM Windows build script for libbpg
REM Requires MinGW/MSYS2 with gcc, g++, ar

echo Building libbpg for Windows...

REM Set compiler flags
set CFLAGS=-Os -Wall -fno-asynchronous-unwind-tables -fdata-sections -ffunction-sections -fno-math-errno -fno-signed-zeros -fno-tree-vectorize -fomit-frame-pointer
set CFLAGS=%CFLAGS% -D_FILE_OFFSET_BITS=64 -D_LARGEFILE_SOURCE -D_REENTRANT
set CFLAGS=%CFLAGS% -I. -DCONFIG_BPG_VERSION=\"0.9.8\"
set CFLAGS=%CFLAGS% -D_ISOC99_SOURCE -D_POSIX_C_SOURCE=200112 -D_XOPEN_SOURCE=600 -DHAVE_AV_CONFIG_H -std=c99 -D_GNU_SOURCE=1 -DUSE_VAR_BIT_DEPTH -DUSE_PRED

REM Compile libavcodec files
echo Compiling libavcodec...
gcc %CFLAGS% -c libavcodec/hevc_cabac.c -o libavcodec/hevc_cabac.o
gcc %CFLAGS% -c libavcodec/hevc_filter.c -o libavcodec/hevc_filter.o
gcc %CFLAGS% -c libavcodec/hevc.c -o libavcodec/hevc.o
gcc %CFLAGS% -c libavcodec/hevcpred.c -o libavcodec/hevcpred.o
gcc %CFLAGS% -c libavcodec/hevc_refs.c -o libavcodec/hevc_refs.o
gcc %CFLAGS% -c libavcodec/hevcdsp.c -o libavcodec/hevcdsp.o
gcc %CFLAGS% -c libavcodec/hevc_mvs.c -o libavcodec/hevc_mvs.o
gcc %CFLAGS% -c libavcodec/hevc_ps.c -o libavcodec/hevc_ps.o
gcc %CFLAGS% -c libavcodec/hevc_sei.c -o libavcodec/hevc_sei.o
gcc %CFLAGS% -c libavcodec/utils.c -o libavcodec/utils.o
gcc %CFLAGS% -c libavcodec/cabac.c -o libavcodec/cabac.o
gcc %CFLAGS% -c libavcodec/golomb.c -o libavcodec/golomb.o
gcc %CFLAGS% -c libavcodec/videodsp.c -o libavcodec/videodsp.o

REM Compile libavutil files
echo Compiling libavutil...
gcc %CFLAGS% -c libavutil/mem.c -o libavutil/mem.o
gcc %CFLAGS% -c libavutil/buffer.c -o libavutil/buffer.o
gcc %CFLAGS% -c libavutil/log2_tab.c -o libavutil/log2_tab.o
gcc %CFLAGS% -c libavutil/frame.c -o libavutil/frame.o
gcc %CFLAGS% -c libavutil/pixdesc.c -o libavutil/pixdesc.o
gcc %CFLAGS% -c libavutil/md5.c -o libavutil/md5.o

REM Compile libbpg.c
echo Compiling libbpg...
gcc %CFLAGS% -c libbpg.c -o libbpg.o

REM Create static library
echo Creating libbpg.a...
ar rcs libbpg.a ^
    libavcodec/hevc_cabac.o ^
    libavcodec/hevc_filter.o ^
    libavcodec/hevc.o ^
    libavcodec/hevcpred.o ^
    libavcodec/hevc_refs.o ^
    libavcodec/hevcdsp.o ^
    libavcodec/hevc_mvs.o ^
    libavcodec/hevc_ps.o ^
    libavcodec/hevc_sei.o ^
    libavcodec/utils.o ^
    libavcodec/cabac.o ^
    libavcodec/golomb.o ^
    libavcodec/videodsp.o ^
    libavutil/mem.o ^
    libavutil/buffer.o ^
    libavutil/log2_tab.o ^
    libavutil/frame.o ^
    libavutil/pixdesc.o ^
    libavutil/md5.o ^
    libbpg.o

echo Build complete! libbpg.a created.
echo.
echo Note: This is the decoder-only library.
echo For encoder support, you need to build with x265 or JCTVC.
