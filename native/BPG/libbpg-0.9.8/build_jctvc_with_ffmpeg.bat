@echo off
REM Build BPG encoder with JCTVC support using new FFmpeg libraries
REM This uses the FFmpeg 8.0.1 libraries we just built

echo ========================================
echo Building BPG JCTVC with FFmpeg 8.0.1
echo ========================================
echo.

REM Set paths to our FFmpeg build
set FFMPEG_DIR=..\..\ffmpeg-build
set FFMPEG_INCLUDE=-I%FFMPEG_DIR%\include
set FFMPEG_LIBS=-L%FFMPEG_DIR%\lib -lavcodec -lavutil -lavformat -lswscale -lswresample

REM Set base compiler flags
set BASE_CFLAGS=-Os -Wall -fno-asynchronous-unwind-tables -fdata-sections -ffunction-sections
set BASE_CFLAGS=%BASE_CFLAGS% -fno-math-errno -fno-signed-zeros -fno-tree-vectorize -fomit-frame-pointer
set BASE_CFLAGS=%BASE_CFLAGS% -D_FILE_OFFSET_BITS=64 -D_LARGEFILE_SOURCE -D_REENTRANT
set BASE_CFLAGS=%BASE_CFLAGS% -I. -DCONFIG_BPG_VERSION=\"0.9.8\"

REM JCTVC specific flags with FFmpeg includes
set JCTVC_CFLAGS=%FFMPEG_INCLUDE% -I./jctvc -I./jctvc/TLibCommon -I./jctvc/TLibEncoder -I./jctvc/TLibVideoIO -I./jctvc/libmd5
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -Wno-sign-compare -Wno-unused-parameter -Wno-missing-field-initializers
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -Wno-misleading-indentation -Wno-class-memaccess
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -DMSYS_PROJECT -D_MSYS2 -D_CRT_SECURE_NO_DEPRECATE -D_CRT_SECURE_NO_WARNINGS
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -D_CRT_NONSTDC_NO_WARNINGS -D_WIN32_WINNT=0x0600 -DUSE_JCTVC
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -D_ISOC99_SOURCE -D_GNU_SOURCE -DHAVE_STRING_H -DHAVE_STDINT_H
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -DHAVE_INTTYPES_H -DHAVE_MALLOC_H -D__STDC_LIMIT_MACROS

set CXXFLAGS=%BASE_CFLAGS% %JCTVC_CFLAGS% -std=c++11

REM Clean previous build
if exist jctvc\*.o (
    echo Cleaning previous JCTVC build...
    del /q jctvc\TLibCommon\*.o 2>nul
    del /q jctvc\TLibEncoder\*.o 2>nul
    del /q jctvc\TLibVideoIO\*.o 2>nul
    del /q jctvc\libmd5\*.o 2>nul
    del /q jctvc\*.o 2>nul
    del /q jctvc\libjctvc.a 2>nul
)

echo.
echo Compiling JCTVC TLibCommon...
for %%f in (jctvc\TLibCommon\*.cpp) do (
    echo   %%~nxf
    g++ %CXXFLAGS% -c %%f -o %%~dpnf.o
    if errorlevel 1 goto error
)

echo.
echo Compiling JCTVC TLibEncoder...
for %%f in (jctvc\TLibEncoder\*.cpp) do (
    echo   %%~nxf
    g++ %CXXFLAGS% -c %%f -o %%~dpnf.o
    if errorlevel 1 goto error
)

echo.
echo Compiling JCTVC TLibVideoIO...
for %%f in (jctvc\TLibVideoIO\*.cpp) do (
    echo   %%~nxf
    g++ %CXXFLAGS% -c %%f -o %%~dpnf.o
    if errorlevel 1 goto error
)

echo.
echo Compiling JCTVC libmd5...
for %%f in (jctvc\libmd5\*.c) do (
    echo   %%~nxf
    gcc %BASE_CFLAGS% %JCTVC_CFLAGS% -c %%f -o %%~dpnf.o
    if errorlevel 1 goto error
)

echo.
echo Compiling JCTVC main files...
g++ %CXXFLAGS% -c jctvc/TAppEncCfg.cpp -o jctvc/TAppEncCfg.o
if errorlevel 1 goto error
g++ %CXXFLAGS% -c jctvc/TAppEncTop.cpp -o jctvc/TAppEncTop.o
if errorlevel 1 goto error
g++ %CXXFLAGS% -c jctvc/program_options_lite.cpp -o jctvc/program_options_lite.o
if errorlevel 1 goto error

echo.
echo Creating JCTVC static library...
REM Use PowerShell to create object file list and build library
powershell -Command "Get-ChildItem -Path jctvc -Recurse -Filter *.o | ForEach-Object { $_.FullName } | Out-File -FilePath objfiles.txt -Encoding ASCII"
ar rcs jctvc/libjctvc.a @objfiles.txt
del objfiles.txt
if errorlevel 1 goto error

echo.
echo Compiling jctvc_glue.cpp...
g++ %CXXFLAGS% -c jctvc_glue.cpp -o jctvc_glue.o
if errorlevel 1 goto error

echo.
echo Compiling bpgenc.c with JCTVC support...
gcc %BASE_CFLAGS% %FFMPEG_INCLUDE% -DUSE_JCTVC -c bpgenc.c -o bpgenc.o
if errorlevel 1 goto error

echo.
echo Linking bpgenc-jctvc.exe with FFmpeg libraries...
g++ -o bpgenc-jctvc.exe bpgenc.o jctvc_glue.o jctvc/libjctvc.a %FFMPEG_LIBS% -lx264 -lx265 -lpng -ljpeg -lz -lm -lstdc++ -lpthread -lbcrypt -lole32 -lstrmiids -luuid -loleaut32 -lshlwapi -lpsapi -ladvapi32 -lshell32 -lws2_32 -luser32 -lwinmm
if errorlevel 1 goto error

echo.
echo ========================================
echo Build Complete!
echo ========================================
echo.
echo bpgenc-jctvc.exe created with:
echo   - JCTVC encoder (better compression)
echo   - FFmpeg 8.0.1 libraries
echo   - H.264/H.265 support
echo.
echo Test with:
echo   bpgenc-jctvc.exe -q 25 -o output.bpg input.jpg
echo.
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
pause
exit /b 1

:end
