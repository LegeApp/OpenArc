@echo off
REM Build Combined BPG Encoder Library (Minimal)
REM This creates libbpg_encoder_combined.a with both x265 and JCTVC for direct linking

echo ========================================
echo Building Combined BPG Encoder Library
echo ========================================
echo.

set GCC=gcc
set GPP=g++
set AR=ar

REM Compiler flags
set CFLAGS=-O3 -Wall -fno-strict-aliasing -std=c99 -DMSYS_UNIX -DMSYS_WIN32
set CXXFLAGS=-O3 -Wall -fno-strict-aliasing -std=c++11 -DMSYS_UNIX -DMSYS_WIN32

REM Include paths
set INCLUDES=-I. -Ijctvc -Ijctvc/TLibCommon -Ijctvc/TLibEncoder -Ijctvc/TLibVideoIO -Ijctvc/libmd5 -Ix265

REM Defines for both encoders
set DEFINES=-DUSE_X265 -DUSE_JCTVC -DCONFIG_BPG_VERSION=\"0.9.8\" -DCONFIG_WIN32=1

REM Create output directory
if not exist obj_encoder mkdir obj_encoder

echo.
echo ========================================
echo Step 1: Compiling BPG encoder core
echo ========================================
echo Compiling libbpg.c...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c libbpg.c -o obj_encoder\libbpg.o
if errorlevel 1 goto error

echo Compiling bpgenc.c...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c bpgenc.c -o obj_encoder\bpgenc.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 2: Compiling x265 glue code
echo ========================================
echo Compiling x265_glue.c...
%GCC% %CFLAGS% %INCLUDES% %DEFINES% -c x265_glue.c -o obj_encoder\x265_glue.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 3: Compiling JCTVC components
echo ========================================

REM JCTVC TLibCommon
for %%f in (jctvc\TLibCommon\*.cpp) do (
    echo Compiling TLibCommon\%%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_encoder\%%~nf.o
    if errorlevel 1 goto error
)

REM JCTVC TLibEncoder  
for %%f in (jctvc\TLibEncoder\*.cpp) do (
    echo Compiling TLibEncoder\%%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_encoder\%%~nf.o
    if errorlevel 1 goto error
)

REM JCTVC TLibVideoIO
for %%f in (jctvc\TLibVideoIO\*.cpp) do (
    echo Compiling TLibVideoIO\%%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_encoder\%%~nf.o
    if errorlevel 1 goto error
)

REM JCTVC libmd5
for %%f in (jctvc\libmd5\*.c) do (
    echo Compiling libmd5\%%~nf.c...
    %GCC% %CFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_encoder\%%~nf.o
    if errorlevel 1 goto error
)

REM JCTVC top-level
for %%f in (jctvc\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% %DEFINES% -c %%f -o obj_encoder\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 4: Compiling JCTVC glue code
echo ========================================
echo Compiling jctvc_glue.cpp...
%GPP% %CXXFLAGS% %INCLUDES% %DEFINES% -c jctvc_glue.cpp -o obj_encoder\jctvc_glue.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 5: Creating combined encoder library
echo ========================================
echo Creating libbpg_encoder_combined.a...

REM Build library by adding each object file individually
%AR% rcs libbpg_encoder_combined.a obj_encoder\*.o
if errorlevel 1 (
    echo Failed with wildcard, trying individual files...
    %AR% rcs libbpg_encoder_combined.a obj_encoder\bpgenc.o
    %AR% rcs libbpg_encoder_combined.a obj_encoder\libbpg.o
    %AR% rcs libbpg_encoder_combined.a obj_encoder\x265_glue.o
    %AR% rcs libbpg_encoder_combined.a obj_encoder\jctvc_glue.o
    %AR% rcs libbpg_encoder_combined.a obj_encoder\TLibCommon\*.o 2>nul
    %AR% rcs libbpg_encoder_combined.a obj_encoder\TLibEncoder\*.o 2>nul
    %AR% rcs libbpg_encoder_combined.a obj_encoder\TLibVideoIO\*.o 2>nul
    %AR% rcs libbpg_encoder_combined.a obj_encoder\libmd5\*.o 2>nul
    %AR% rcs libbpg_encoder_combined.a obj_encoder\encmain.o 2>nul
    %AR% rcs libbpg_encoder_combined.a obj_encoder\program_options_lite.o 2>nul
)
if errorlevel 1 goto error

echo.
echo ========================================
echo Combined BPG Encoder Library Complete!
echo ========================================
echo.
dir /b libbpg_encoder_combined.a 2>nul && echo Built: libbpg_encoder_combined.a
for /f %%A in ('dir /b obj_encoder\*.o ^| find /c ".o"') do echo Object files: %%A

REM Show library size
for %%F in (libbpg_encoder_combined.a) do echo Library size: %%~zF bytes

echo.
echo Library contains:
echo   - BPG encoder core
echo   - x265 encoder glue code
echo   - JCTVC encoder (full reference implementation)
echo.
echo Next: Create OpenArc DLL wrapper
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
