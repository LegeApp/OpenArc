@echo off
echo Building combined BPG encoder (x265 + JCTVC)

set GCC=gcc
set GPP=g++
set AR=ar

REM Clean
if exist bpgenc-combined.exe del bpgenc-combined.exe
if exist obj_combined rmdir /s /q obj_combined
mkdir obj_combined

echo Step 1: Compiling JCTVC (using pre-built libjctvc.a)...
if not exist jctvc\libjctvc.a (
    echo ERROR: jctvc\libjctvc.a not found! Run build_native_lib_with_jctvc.bat first
    exit /b 1
)

echo Step 2: Compiling x265 glue...
%GCC% -O3 -Wall -I. -DUSE_X265 -c x265_glue.c -o obj_combined\x265_glue.o
if errorlevel 1 goto error

echo Step 3: Compiling JCTVC glue...
%GPP% -O3 -Wall -I. -Ijctvc -DUSE_JCTVC -c jctvc_glue.cpp -o obj_combined\jctvc_glue.o
if errorlevel 1 goto error

echo Step 4: Compiling libbpg decoder...
%GCC% -O3 -Wall -I. -Ilibavutil -Ilibavcodec -c libbpg.c -o obj_combined\libbpg.o
if errorlevel 1 goto error

echo Step 5: Compiling bpgenc (with both encoders)...
%GCC% -O3 -Wall -I. -DUSE_X265 -DUSE_JCTVC -DCONFIG_BPG_VERSION=\"0.9.8\" -c bpgenc.c -o obj_combined\bpgenc.o
if errorlevel 1 goto error

echo Step 6: Linking final executable...
%GPP% -o bpgenc-combined.exe obj_combined\bpgenc.o obj_combined\libbpg.o obj_combined\x265_glue.o obj_combined\jctvc_glue.o jctvc\libjctvc.a -lx265 -lpng -ljpeg -lz -lstdc++ -lm
if errorlevel 1 goto error

echo.
echo SUCCESS! Built: bpgenc-combined.exe
dir /b bpgenc-combined.exe
echo.
echo Test with:
echo   bpgenc-combined.exe -e x265 -q 28 input.jpg -o output_x265.bpg
echo   bpgenc-combined.exe -e jctvc -q 28 input.jpg -o output_jctvc.bpg
goto end

:error
echo.
echo BUILD FAILED!
exit /b 1

:end
