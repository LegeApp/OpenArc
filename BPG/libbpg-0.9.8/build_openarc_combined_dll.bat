@echo off
REM Build OpenArc BPG DLL with Combined x265 AND JCTVC Encoder Support
REM This creates openarc_bpg.dll for direct integration into OpenArc applications
REM Both x265 (fast/standard) and JCTVC (slow/best compression) encoders are included

REM Run from this script's directory to make relative paths reliable
pushd "%~dp0"

REM Prefer parallel compilation when pwsh is available.
REM Set OPENARC_NO_PARALLEL=1 to force the legacy sequential .bat build.
where pwsh >nul 2>&1
if %errorlevel%==0 (
    if not "%OPENARC_NO_PARALLEL%"=="1" (
        set "OPENARC_JOBS_ARG=%OPENARC_JOBS%"
        if "%OPENARC_JOBS_ARG%"=="" set "OPENARC_JOBS_ARG=0"
        pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0build_openarc_combined_dll.ps1" -Jobs %OPENARC_JOBS_ARG% -Clean
        set "PS_EXIT=%errorlevel%"
        popd
        exit /b %PS_EXIT%
    )
)

echo ========================================
echo Building OpenArc BPG DLL (Combined)
echo x265 + JCTVC Encoders
echo ========================================
echo.

REM Prefer MSYS2 MinGW64 toolchain explicitly (avoids mixing MSYS/UCRT/MinGW32 runtimes)
set "MSYS64=C:\msys64"
set "TOOLBIN="

if exist "%MSYS64%\mingw64\bin\g++.exe" (
    set "TOOLBIN=%MSYS64%\mingw64\bin"
) else if exist "%MSYS64%\ucrt64\bin\g++.exe" (
    set "TOOLBIN=%MSYS64%\ucrt64\bin"
) else if exist "%MSYS64%\clang64\bin\clang++.exe" (
    set "TOOLBIN=%MSYS64%\clang64\bin"
)

if not "%TOOLBIN%"=="" (
    set "PATH=%TOOLBIN%;%PATH%"
)

set "GCC=gcc"
set "GPP=g++"
set "AR=ar"

if not "%TOOLBIN%"=="" (
    if exist "%TOOLBIN%\gcc.exe" set "GCC=%TOOLBIN%\gcc.exe"
    if exist "%TOOLBIN%\g++.exe" set "GPP=%TOOLBIN%\g++.exe"
    if exist "%TOOLBIN%\ar.exe" set "AR=%TOOLBIN%\ar.exe"
)

REM Create output directory
if not exist obj_dll mkdir obj_dll

REM Base compiler flags
REM Note: do NOT disable unwind tables for C++ (breaks SEH unwinding and can lead to SjLj references)
set BASE_CFLAGS=-O3 -Wall -fno-strict-aliasing -fno-asynchronous-unwind-tables -fdata-sections -ffunction-sections
set BASE_CFLAGS=%BASE_CFLAGS% -fno-math-errno -fno-signed-zeros -fno-tree-vectorize -fomit-frame-pointer
set BASE_CFLAGS=%BASE_CFLAGS% -D_FILE_OFFSET_BITS=64 -D_LARGEFILE_SOURCE -D_REENTRANT

REM C-specific flags
set CFLAGS=%BASE_CFLAGS% -std=c99

REM C++-specific flags
REM For C++ on MinGW-w64 x64 we need proper unwind tables for SEH exceptions.
REM Start from BASE_CFLAGS but override the unwind setting.
set CXXFLAGS=%BASE_CFLAGS% -std=c++11 -fexceptions -funwind-tables -fasynchronous-unwind-tables

REM Include paths for all components
set INCLUDES=-I. -Ilibavutil -Ilibavcodec -Ijctvc -Ijctvc/TLibCommon -Ijctvc/TLibEncoder -Ijctvc/TLibVideoIO -Ijctvc/libmd5 -Ix265

REM Defines for BOTH encoders - this is the key!
REM HAVE_AV_CONFIG_H is required for the bundled FFmpeg subset to consistently
REM include config.h/intmath.h via libavutil/common.h (fixes missing get_bits*/golomb and av_log2 symbols).
set DEFINES=-DUSE_X265 -DUSE_JCTVC -DHAVE_AV_CONFIG_H=1 -DCONFIG_BPG_VERSION=\"0.9.8\" -DCONFIG_WIN32=1 -DFF_MEMORY_POISON=0x2a
set DEFINES=%DEFINES% -DMSYS_PROJECT -D_MSYS2 -D_CRT_SECURE_NO_DEPRECATE -D_CRT_SECURE_NO_WARNINGS
set DEFINES=%DEFINES% -D_CRT_NONSTDC_NO_WARNINGS -D_WIN32_WINNT=0x0600 -DBUILDING_DLL
set DEFINES=%DEFINES% -D_ISOC99_SOURCE -D_GNU_SOURCE -DHAVE_STRING_H -DHAVE_STDINT_H
set DEFINES=%DEFINES% -DHAVE_INTTYPES_H -DHAVE_MALLOC_H -D__STDC_LIMIT_MACROS

REM JCTVC-specific warning suppressions for C++
set JCTVC_WARNINGS=-Wno-sign-compare -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation -Wno-class-memaccess
set JCTVC_CXXFLAGS=%CXXFLAGS% %JCTVC_WARNINGS%

echo.
echo ========================================
echo Step 1: Compiling JCTVC TLibCommon
echo ========================================
for %%f in (jctvc\TLibCommon\*.cpp) do (
    echo   Compiling TLibCommon\%%~nf.cpp...
    %GPP% %JCTVC_CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_dll\TLib_%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 2: Compiling JCTVC TLibEncoder
echo ========================================
for %%f in (jctvc\TLibEncoder\*.cpp) do (
    echo   Compiling TLibEncoder\%%~nf.cpp...
    %GPP% %JCTVC_CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_dll\TEnc_%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 3: Compiling JCTVC TLibVideoIO
echo ========================================
for %%f in (jctvc\TLibVideoIO\*.cpp) do (
    echo   Compiling TLibVideoIO\%%~nf.cpp...
    %GPP% %JCTVC_CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_dll\TVid_%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 4: Compiling JCTVC libmd5
echo ========================================
for %%f in (jctvc\libmd5\*.c) do (
    echo   Compiling libmd5\%%~nf.c...
    %GCC% %CFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_dll\md5_%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 5: Compiling JCTVC top-level files
echo ========================================
echo   Compiling TAppEncCfg.cpp...
%GPP% %JCTVC_CXXFLAGS% %INCLUDES% %DEFINES% -c jctvc\TAppEncCfg.cpp -o obj_dll\TAppEncCfg.o
if errorlevel 1 goto error

echo   Compiling TAppEncTop.cpp...
%GPP% %JCTVC_CXXFLAGS% %INCLUDES% %DEFINES% -c jctvc\TAppEncTop.cpp -o obj_dll\TAppEncTop.o
if errorlevel 1 goto error

echo   Compiling program_options_lite.cpp...
%GPP% %JCTVC_CXXFLAGS% %INCLUDES% %DEFINES% -c jctvc\program_options_lite.cpp -o obj_dll\program_options_lite.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 6: Compiling encoder glue code
echo ========================================
echo   Compiling jctvc_glue.cpp (JCTVC encoder bridge)...
%GPP% %JCTVC_CXXFLAGS% %INCLUDES% %DEFINES% -c jctvc_glue.cpp -o obj_dll\jctvc_glue.o
if errorlevel 1 goto error

echo   Compiling x265_glue.c (x265 encoder bridge)...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c x265_glue.c -o obj_dll\x265_glue.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 7: Compiling BPG core
echo ========================================
echo   Compiling bpgenc.c (encoder with both USE_X265 and USE_JCTVC)...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c bpgenc.c -o obj_dll\bpgenc.o
if errorlevel 1 goto error

echo   Compiling libbpg.c (decoder)...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c libbpg.c -o obj_dll\libbpg.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 7b: Compiling libavutil (decoder deps)
echo ========================================
for %%f in (libavutil\*.c) do (
    echo   Compiling libavutil\%%~nf.c...
    %GCC% %CFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_dll\avutil_%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 7c: Compiling libavcodec (decoder deps)
echo ========================================
REM NOTE: libavcodec contains several "*_template.c" files that are meant to
REM be #included by other compilation units, not compiled standalone.
REM Compiling them directly fails (missing symbols like "transform").
set AVCODEC_SRCS=cabac.c golomb.c hevc.c hevc_cabac.c hevc_filter.c hevc_mvs.c hevc_ps.c hevc_refs.c hevc_sei.c hevcdsp.c hevcpred.c utils.c videodsp.c
for %%f in (%AVCODEC_SRCS%) do (
    echo   Compiling libavcodec\%%f...
    %GCC% %CFLAGS% %INCLUDES% %DEFINES% -c libavcodec\%%f -o obj_dll\avcodec_%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 8: Compiling BPG API layer
echo ========================================
echo   Compiling bpg_api.c...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c bpg_api.c -o obj_dll\bpg_api.o
if errorlevel 1 goto error

echo   Compiling openarc_bpg_dll.c...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c openarc_bpg_dll.c -o obj_dll\openarc_bpg_dll.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 9: Creating combined object archive
echo ========================================
echo   Creating libbpg_combined.a...

rem Use response file to avoid command line length limits and variable expansion issues
dir /b obj_dll\*.o > obj_dll\objlist.txt
setlocal enabledelayedexpansion
set OBJFILES=
for /f %%f in (obj_dll\objlist.txt) do (
    set OBJFILES=!OBJFILES! obj_dll\%%f
)
endlocal & set OBJFILES=%OBJFILES%

if "!OBJFILES!"=="" (
    echo Error: No object files found in obj_dll
    goto error
)

%AR% rcs obj_dll\libbpg_combined.a %OBJFILES%
if errorlevel 1 goto error
del obj_dll\objlist.txt

echo.
echo ========================================
echo Step 10: Linking openarc_bpg.dll
echo ========================================
echo   Linking DLL with x265 + JCTVC support...
%GPP% -shared -o openarc_bpg.dll ^
    -Wl,--out-implib,openarc_bpg.lib ^
    -Wl,--export-all-symbols ^
    -Wl,--whole-archive obj_dll\libbpg_combined.a -Wl,--no-whole-archive ^
    -lx265 -lpng -ljpeg -lz -lpthread -lm -lws2_32

if errorlevel 1 goto error

echo.
echo ========================================
echo OpenArc BPG DLL Build Complete!
echo ========================================
echo.

REM Display file info
for %%F in (openarc_bpg.dll) do echo DLL: openarc_bpg.dll (%%~zF bytes)
for %%F in (openarc_bpg.lib) do echo Import Lib: openarc_bpg.lib (%%~zF bytes)

echo.
echo Encoders included:
echo   [x265]  Fast encoder - for standard/fast mode
echo   [JCTVC] Reference encoder - for slow/best compression mode
echo.
echo API Functions:
echo   openarc_bpg_encode_file(input, output, quality, encoder_type)
echo   openarc_bpg_encode_memory(data, w, h, stride, fmt, q, enc, out, size)
echo   openarc_bpg_get_supported_encoders()
popd
echo   openarc_bpg_is_encoder_supported(encoder_type)
echo.
echo Encoder Types:
echo   0 = x265 (fast, good compression)
echo   1 = JCTVC (slow, best compression)
echo.
echo Required Runtime DLLs:
echo   - libx265.dll (or x265.dll)
echo   - libpng16-16.dll
echo   - libjpeg-62.dll
echo   - zlib1.dll
echo   - libgcc_s_seh-1.dll
echo   - libstdc++-6.dll
echo   - libwinpthread-1.dll
echo.
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
echo.
echo Common fixes:
echo   1. Install x265: pacman -S mingw-w64-x86_64-x265
echo   2. Install image libs: pacman -S mingw-w64-x86_64-libpng mingw-w64-x86_64-libjpeg-turbo
echo   3. Ensure MSYS2/MinGW64 bin is in PATH
echo.
exit /b 1

:end
exit /b 0
