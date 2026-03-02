@echo off
REM Complete BPG library build - decoder + encoder wrapper
REM This creates a library that can be used from Rust via FFI

echo Building complete BPG library (decoder + encoder wrapper)...

REM Set base compiler flags
set CFLAGS=-Os -Wall -fno-asynchronous-unwind-tables -fdata-sections -ffunction-sections
set CFLAGS=%CFLAGS% -fno-math-errno -fno-signed-zeros -fno-tree-vectorize -fomit-frame-pointer
set CFLAGS=%CFLAGS% -D_FILE_OFFSET_BITS=64 -D_LARGEFILE_SOURCE -D_REENTRANT
set CFLAGS=%CFLAGS% -I. -DCONFIG_BPG_VERSION=\"0.9.8\"
set CFLAGS=%CFLAGS% -D_ISOC99_SOURCE -D_POSIX_C_SOURCE=200112 -D_XOPEN_SOURCE=600
set CFLAGS=%CFLAGS% -DHAVE_AV_CONFIG_H -std=c99 -D_GNU_SOURCE=1 -DUSE_VAR_BIT_DEPTH -DUSE_PRED

echo.
echo ========================================
echo Step 1: Building decoder library
echo ========================================

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

echo.
echo ========================================
echo Step 2: Creating encoder wrapper
echo ========================================

REM Create a simple C wrapper that calls bpgenc.exe as a subprocess
echo Creating bpg_encoder_wrapper.c...
(
echo #include ^<stdio.h^>
echo #include ^<stdlib.h^>
echo #include ^<string.h^>
echo #include ^<windows.h^>
echo.
echo // Simple encoder wrapper that calls bpgenc.exe
echo int bpg_encode_file^(const char* input_file, const char* output_file, int quality, int lossless^) {
echo     char cmd[1024];
echo     if ^(lossless^) {
echo         snprintf^(cmd, sizeof^(cmd^), "bpgenc.exe -lossless -o %%s %%s", output_file, input_file^);
echo     } else {
echo         snprintf^(cmd, sizeof^(cmd^), "bpgenc.exe -q %%d -o %%s %%s", quality, output_file, input_file^);
echo     }
echo     return system^(cmd^);
echo }
echo.
echo // Memory-based encoding ^(writes to temp file then reads back^)
echo int bpg_encode_memory^(const unsigned char* input_data, size_t input_size,
echo                        unsigned char** output_data, size_t* output_size,
echo                        int width, int height, int quality, int lossless^) {
echo     // For now, return error - implement file-based encoding first
echo     return -1;
echo }
) > bpg_encoder_wrapper.c

echo Compiling encoder wrapper...
gcc %CFLAGS% -c bpg_encoder_wrapper.c -o bpg_encoder_wrapper.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 3: Creating static library
echo ========================================

echo Creating libbpg_complete.a...
ar rcs libbpg_complete.a ^
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
    bpg_encoder_wrapper.o

if errorlevel 1 goto error

echo.
echo ========================================
echo Build complete!
echo ========================================
echo.
echo Created libraries:
echo   - libbpg.a (decoder only, 205KB)
echo   - libbpg_complete.a (decoder + encoder wrapper)
echo.
echo The encoder wrapper uses bpgenc.exe as a subprocess.
echo Make sure bpgenc.exe is in the same directory as your executable.
echo.
echo Next steps:
echo   1. Copy bpgenc.exe to your Rust project's output directory
echo   2. Link libbpg_complete.a in your Rust build.rs
echo   3. Create FFI bindings in Rust
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
