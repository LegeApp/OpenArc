@echo off
REM Build Combined BPG Static Library with Both x265 and JCTVC Encoders
REM This creates libbpg_combined.a for direct linking into OpenArc

echo ========================================
echo Building Combined BPG Static Library
echo ========================================
echo.

set GCC=gcc
set GPP=g++
set AR=ar

REM Compiler flags for combined library
set CFLAGS=-O3 -Wall -fno-strict-aliasing -std=c99 -DMSYS_UNIX -DMSYS_WIN32
set CXXFLAGS=-O3 -Wall -fno-strict-aliasing -std=c++11 -DMSYS_UNIX -DMSYS_WIN32

REM Include paths for all components
set INCLUDES=-I. -Ilibavutil -Ilibavcodec -Ijctvc -Ijctvc/TLibCommon -Ijctvc/TLibEncoder -Ijctvc/TLibVideoIO -Ijctvc/libmd5 -Ix265

REM Defines for both encoders
set DEFINES=-DUSE_X265 -DUSE_JCTVC -DCONFIG_BPG_VERSION=\"0.9.8\" -DCONFIG_WIN32=1 -DFF_MEMORY_POISON=0x2a

REM Create output directory
if not exist obj_combined mkdir obj_combined

echo.
echo ========================================
echo Step 1: Compiling libavutil sources
echo ========================================
for %%f in (libavutil\*.c) do (
    echo Compiling %%~nf.c...
    %GCC% %CFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_combined\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 2: Compiling libavcodec sources  
echo ========================================
for %%f in (libavcodec\*.c) do (
    echo Compiling %%~nf.c...
    %GCC% %CFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_combined\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 3: Compiling BPG core
echo ========================================
echo Compiling libbpg.c...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c libbpg.c -o obj_combined\libbpg.o
if errorlevel 1 goto error

echo Compiling bpgenc.c...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c bpgenc.c -o obj_combined\bpgenc.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 4: Compiling x265 glue code
echo ========================================
echo Compiling x265_glue.c...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c x265_glue.c -o obj_combined\x265_glue.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 5: Compiling JCTVC components
echo ========================================

REM JCTVC TLibCommon
for %%f in (jctvc\TLibCommon\*.cpp) do (
    echo Compiling TLibCommon\%%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_combined\%%~nf.o
    if errorlevel 1 goto error
)

REM JCTVC TLibEncoder  
for %%f in (jctvc\TLibEncoder\*.cpp) do (
    echo Compiling TLibEncoder\%%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_combined\%%~nf.o
    if errorlevel 1 goto error
)

REM JCTVC TLibVideoIO
for %%f in (jctvc\TLibVideoIO\*.cpp) do (
    echo Compiling TLibVideoIO\%%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_combined\%%~nf.o
    if errorlevel 1 goto error
)

REM JCTVC libmd5
for %%f in (jctvc\libmd5\*.c) do (
    echo Compiling libmd5\%%~nf.c...
    %GCC% %CFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_combined\%%~nf.o
    if errorlevel 1 goto error
)

REM JCTVC top-level
for %%f in (jctvc\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_combined\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 6: Compiling JCTVC glue code
echo ========================================
echo Compiling jctvc_glue.cpp...
%GPP% %CXXFLAGS% %INCLUDES% %DEFINES% -c jctvc_glue.cpp -o obj_combined\jctvc_glue.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 7: Creating combined static library
echo ========================================
echo Creating libbpg_combined.a...

REM Create object file list for ar
dir /b obj_combined\*.o > obj_combined\objlist.txt
set OBJFILES=
for /f %%f in (obj_combined\objlist.txt) do set OBJFILES=%OBJFILES% obj_combined\%%f

%AR% rcs libbpg_combined.a %OBJFILES%
if errorlevel 1 goto error
del obj_combined\objlist.txt

echo.
echo ========================================
echo Combined BPG Library Build Complete!
echo ========================================
echo.
dir /b libbpg_combined.a 2>nul && echo Built: libbpg_combined.a
for /f %%A in ('dir /b obj_combined\*.o ^| find /c ".o"') do echo Object files: %%A

REM Show library size
for %%F in (libbpg_combined.a) do echo Library size: %%~zF bytes

echo.
echo Library contains:
echo   - BPG decoder (libavutil + libavcodec)
echo   - BPG encoder core  
echo   - x265 encoder glue code
echo   - JCTVC encoder (full reference implementation)
echo.
echo Usage in OpenArc:
echo   1. Link with: -L. -lbpg_combined -lx265 -lstdc++ -lpng -ljpeg -lz
echo   2. Include: bpg_api.h
echo   3. Call: bpg_encode_with_encoder(input, output, quality, encoder_type)
echo.
echo Encoder types: BPG_ENCODER_X265, BPG_ENCODER_JCTVC
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
exit /b 1

:end
