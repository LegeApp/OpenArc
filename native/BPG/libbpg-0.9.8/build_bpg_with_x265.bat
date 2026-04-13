@echo off
REM Build BPG encoder with x265 support
REM Requires: x265 built in x265.out/, libpng, libjpeg

echo ========================================
echo Building BPG encoder with x265 support
echo ========================================

set CFLAGS=-Os -Wall -fno-asynchronous-unwind-tables -fdata-sections -ffunction-sections
set CFLAGS=%CFLAGS% -fno-math-errno -fno-signed-zeros -fno-tree-vectorize -fomit-frame-pointer
set CFLAGS=%CFLAGS% -D_FILE_OFFSET_BITS=64 -D_LARGEFILE_SOURCE -D_REENTRANT
set CFLAGS=%CFLAGS% -I. -DCONFIG_BPG_VERSION=\"0.9.8\"
set CFLAGS=%CFLAGS% -DUSE_X265

set CXXFLAGS=%CFLAGS% -std=c++11

REM Include paths for x265
set X265_INCLUDE=-I./x265/source -I./x265.out/8bit

REM Check if x265 libraries exist
if not exist x265.out\8bit\libx265.a (
    echo ERROR: x265 libraries not found!
    echo Run build_x265.bat first.
    exit /b 1
)

echo.
echo ========================================
echo Step 1: Building decoder library
echo ========================================

REM Compile libavcodec files
echo Compiling libavcodec...
set DECODER_CFLAGS=%CFLAGS% -D_ISOC99_SOURCE -D_POSIX_C_SOURCE=200112 -D_XOPEN_SOURCE=600 -DHAVE_AV_CONFIG_H -std=c99 -D_GNU_SOURCE=1 -DUSE_VAR_BIT_DEPTH -DUSE_PRED

gcc %DECODER_CFLAGS% -c libavcodec/hevc_cabac.c -o libavcodec/hevc_cabac.o
gcc %DECODER_CFLAGS% -c libavcodec/hevc_filter.c -o libavcodec/hevc_filter.o
gcc %DECODER_CFLAGS% -c libavcodec/hevc.c -o libavcodec/hevc.o
gcc %DECODER_CFLAGS% -c libavcodec/hevcpred.c -o libavcodec/hevcpred.o
gcc %DECODER_CFLAGS% -c libavcodec/hevc_refs.c -o libavcodec/hevc_refs.o
gcc %DECODER_CFLAGS% -c libavcodec/hevcdsp.c -o libavcodec/hevcdsp.o
gcc %DECODER_CFLAGS% -c libavcodec/hevc_mvs.c -o libavcodec/hevc_mvs.o
gcc %DECODER_CFLAGS% -c libavcodec/hevc_ps.c -o libavcodec/hevc_ps.o
gcc %DECODER_CFLAGS% -c libavcodec/hevc_sei.c -o libavcodec/hevc_sei.o
gcc %DECODER_CFLAGS% -c libavcodec/utils.c -o libavcodec/utils.o
gcc %DECODER_CFLAGS% -c libavcodec/cabac.c -o libavcodec/cabac.o
gcc %DECODER_CFLAGS% -c libavcodec/golomb.c -o libavcodec/golomb.o
gcc %DECODER_CFLAGS% -c libavcodec/videodsp.c -o libavcodec/videodsp.o

echo Compiling libavutil...
gcc %DECODER_CFLAGS% -c libavutil/mem.c -o libavutil/mem.o
gcc %DECODER_CFLAGS% -c libavutil/buffer.c -o libavutil/buffer.o
gcc %DECODER_CFLAGS% -c libavutil/log2_tab.c -o libavutil/log2_tab.o
gcc %DECODER_CFLAGS% -c libavutil/frame.c -o libavutil/frame.o
gcc %DECODER_CFLAGS% -c libavutil/pixdesc.c -o libavutil/pixdesc.o
gcc %DECODER_CFLAGS% -c libavutil/md5.c -o libavutil/md5.o

echo Compiling libbpg...
gcc %DECODER_CFLAGS% -c libbpg.c -o libbpg.o

echo.
echo ========================================
echo Step 2: Building encoder components
echo ========================================

echo Compiling x265_glue.c...
gcc %CFLAGS% %X265_INCLUDE% -c x265_glue.c -o x265_glue.o
if errorlevel 1 goto error

echo Compiling bpgenc.c...
gcc %CFLAGS% %X265_INCLUDE% -c bpgenc.c -o bpgenc.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 3: Linking bpgenc.exe
echo ========================================

echo Linking...
g++ -o bpgenc_native.exe bpgenc.o x265_glue.o ^
    x265.out/8bit/libx265.a ^
    x265.out/10bit/libx265.a ^
    x265.out/12bit/libx265.a ^
    -lpng -ljpeg -lm -lstdc++ -lpthread
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 4: Creating static library
echo ========================================

echo Creating libbpg_full.a...
ar rcs libbpg_full.a ^
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
    libbpg.o ^
    x265_glue.o ^
    bpgenc.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Build complete!
echo ========================================
echo.
echo Created:
echo   - bpgenc_native.exe (BPG encoder with x265)
echo   - libbpg_full.a (static library with encoder+decoder)
echo.
echo Test with:
echo   bpgenc_native.exe -o test.bpg test_input.jpg
echo.
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
exit /b 1

:end
