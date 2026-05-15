@echo off
REM Build OpenArc BPG DLL with Combined Encoder Support
REM This creates openarc_bpg.dll for direct integration into OpenArc applications

echo ========================================
echo Building OpenArc BPG DLL
echo ========================================
echo.

set GCC=gcc
set GPP=g++

REM Compiler flags for DLL
set CFLAGS=-O3 -Wall -fno-strict-aliasing -std=c99 -DMSYS_UNIX -DMSYS_WIN32 -DBUILDING_DLL
set CXXFLAGS=-O3 -Wall -fno-strict-aliasing -std=c++11 -DMSYS_UNIX -DMSYS_WIN32

REM Include paths
set INCLUDES=-I. -Ijctvc -Ijctvc/TLibCommon -Ijctvc/TLibEncoder -Ijctvc/TLibVideoIO -Ijctvc/libmd5 -Ix265

REM Defines for both encoders
set DEFINES=-DUSE_X265 -DUSE_JCTVC -DCONFIG_BPG_VERSION=\"0.9.8\" -DCONFIG_WIN32=1

REM Link flags for DLL
set LDFLAGS=-shared -Wl,--out-implib,openarc_bpg.lib

REM Libraries to link
set LIBS=libbpg_encoder_combined.a -lx265 -lstdc++ -lpng -ljpeg -lz -lws2_32

echo.
echo ========================================
echo Step 1: Compiling OpenArc DLL wrapper
echo ========================================
echo Compiling openarc_bpg_dll.c...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c openarc_bpg_dll.c -o openarc_bpg_dll.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 2: Compiling BPG API implementation
echo ========================================
echo Compiling bpg_api.c...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c bpg_api.c -o bpg_api.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 3: Linking OpenArc BPG DLL
echo ========================================
echo Creating openarc_bpg.dll...
%GCC% %LDFLAGS% -o openarc_bpg.dll ^
    openarc_bpg_dll.o ^
    bpg_api.o ^
    libbpg_encoder_combined.a ^
    %LIBS%

if errorlevel 1 goto error

echo.
echo ========================================
echo Step 4: Creating import library
echo ========================================
echo Import library already created: openarc_bpg.lib

echo.
echo ========================================
echo OpenArc BPG DLL Build Complete!
echo ========================================
echo.
dir /b openarc_bpg.dll 2>nul && echo Built: openarc_bpg.dll
dir /b openarc_bpg.lib 2>nul && echo Import library: openarc_bpg.lib

for %%F in (openarc_bpg.dll) do echo DLL size: %%~zF bytes
for %%F in (openarc_bpg.lib) do echo LIB size: %%~zF bytes

echo.
echo Files created:
echo   - openarc_bpg.dll (DLL for runtime)
echo   - openarc_bpg.lib (Import library for linking)
echo.
echo Usage in OpenArc:
echo   1. Link with: openarc_bpg.lib
echo   2. Include: openarc_bpg_dll.h (create from function declarations)
echo   3. Load DLL: LoadLibrary("openarc_bpg.dll")
echo   4. Use functions: openarc_bpg_encode_file(), etc.
echo.
echo Example:
echo   // Encode with x265 (fast)
echo   openarc_bpg_encode_file("input.jpg", "output_x265.bpg", 28, 0);
echo.
echo   // Encode with JCTVC (best compression)
echo   openarc_bpg_encode_file("input.jpg", "output_jctvc.bpg", 28, 1);
echo.
echo Required runtime DLLs:
echo   - x265.dll (if using x265 encoder)
echo   - libpng16-16.dll, libjpeg-62.dll, zlib1.dll (for image input)
echo.
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
echo.
echo Common issues:
echo 1. Missing x265 library - install with: pacman -S mingw-w64-x86_64-x265
echo 2. Missing image libraries - install: pacman -S mingw-w64-x86_64-libpng mingw-w64-x86_64-libjpeg
echo 3. Link errors - check library paths and dependencies
exit /b 1

:end
