@echo off
REM Build BPG encoder with JCTVC HEVC encoder
REM JCTVC provides better compression than x265 but slower encoding

echo ========================================
echo Building BPG Encoder with JCTVC
echo ========================================

set GCC=gcc
set GPP=g++
set AR=ar

REM Compiler flags
set CFLAGS=-O3 -Wall -I. -Ilibavutil -Ilibavcodec -Ijctvc -Ijctvc/TLibCommon -Ijctvc/TLibEncoder -Ijctvc/TLibVideoIO -Ijctvc/libmd5
set CXXFLAGS=-O3 -Wall -std=c++11 -I. -Ilibavutil -Ilibavcodec -Ijctvc -Ijctvc/TLibCommon -Ijctvc/TLibEncoder -Ijctvc/TLibVideoIO -Ijctvc/libmd5

REM Define USE_JCTVC to enable JCTVC encoder in BPG
set DEFINES=-DUSE_JCTVC -DCONFIG_BPG_VERSION=\"0.9.8\" -DFF_MEMORY_POISON=0x2a

REM Create output directory
if not exist obj_jctvc mkdir obj_jctvc

echo.
echo ========================================
echo Step 1: Compiling libavutil sources
echo ========================================
for %%f in (libavutil\*.c) do (
    echo Compiling %%~nf.c...
    %GCC% %CFLAGS% %DEFINES% -c %%f -o obj_jctvc\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 2: Compiling libavcodec sources
echo ========================================
for %%f in (libavcodec\*.c) do (
    echo Compiling %%~nf.c...
    %GCC% %CFLAGS% %DEFINES% -c %%f -o obj_jctvc\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 3: Compiling BPG core
echo ========================================
echo Compiling libbpg.c...
%GCC% %CFLAGS% %DEFINES% -c libbpg.c -o obj_jctvc\libbpg.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 4: Compiling JCTVC glue code
echo ========================================
echo Compiling jctvc_glue.cpp...
%GPP% %CXXFLAGS% %DEFINES% -c jctvc_glue.cpp -o obj_jctvc\jctvc_glue.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 5: Compiling BPG encoder
echo ========================================
echo Compiling bpgenc.c...
%GCC% %CFLAGS% %DEFINES% -DCONFIG_JCTVC -c bpgenc.c -o obj_jctvc\bpgenc.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 6: Linking bpgenc_jctvc.exe
echo ========================================
echo Linking executable...
%GPP% -o bpgenc_jctvc.exe ^
    obj_jctvc\bpgenc.o ^
    obj_jctvc\libbpg.o ^
    obj_jctvc\jctvc_glue.o ^
    obj_jctvc\*.o ^
    jctvc\libjctvc.a ^
    -lpng -ljpeg -lz -lstdc++ -lm

if errorlevel 1 goto error

echo.
echo ========================================
echo BPG JCTVC encoder build complete!
echo ========================================
echo.
dir /b bpgenc_jctvc.exe 2>nul && echo Built: bpgenc_jctvc.exe
echo.
echo Test with:
echo   bpgenc_jctvc.exe -q 25 -o output.bpg input.jpg
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
exit /b 1

:end
