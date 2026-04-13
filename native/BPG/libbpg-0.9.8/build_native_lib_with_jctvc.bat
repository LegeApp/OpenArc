@echo off
REM Build BPG Native Library with JCTVC Support
REM Provides both x265 and JCTVC encoders in a single library for FFI integration

echo ========================================
echo Building BPG Native Library with JCTVC
echo ========================================

set GCC=gcc
set GPP=g++
set AR=ar

REM Compiler flags with JCTVC support (Debug functionality disabled for GCC 15.2 compatibility)
set BASE_CFLAGS=-O3 -Wall -I. -Ilibavutil -Ilibavcodec -DFF_MEMORY_POISON=0x2a -DUSE_JCTVC
set CFLAGS=%BASE_CFLAGS% -DCONFIG_BPG_VERSION=\"0.9.8\"
set CXXFLAGS=%BASE_CFLAGS% -std=gnu++11 -Ijctvc -Ijctvc/TLibCommon -Ijctvc/TLibEncoder -Ijctvc/TLibVideoIO -Ijctvc/libmd5
set CXXFLAGS=%CXXFLAGS% -Wno-sign-compare -Wno-unused-parameter -Wno-missing-field-initializers -Wno-class-memaccess

REM Create output directories
if not exist obj_native_jctvc mkdir obj_native_jctvc
if not exist jctvc\obj mkdir jctvc\obj

echo.
echo ========================================
echo Step 1: Compiling JCTVC TLibCommon
echo ========================================
for %%f in (jctvc\TLibCommon\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %GPP% %CXXFLAGS% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 2: Compiling JCTVC TLibEncoder
echo ========================================
for %%f in (jctvc\TLibEncoder\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %GPP% %CXXFLAGS% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 3: Compiling JCTVC TLibVideoIO
echo ========================================
for %%f in (jctvc\TLibVideoIO\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %GPP% %CXXFLAGS% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 4: Compiling JCTVC support files
echo ========================================
echo Compiling program_options_lite.cpp...
%GPP% %CXXFLAGS% -c jctvc\program_options_lite.cpp -o jctvc\obj\program_options_lite.o
if errorlevel 1 goto error

echo Compiling TAppEncCfg.cpp...
%GPP% %CXXFLAGS% -c jctvc\TAppEncCfg.cpp -o jctvc\obj\TAppEncCfg.o
if errorlevel 1 goto error

echo Compiling TAppEncTop.cpp...
%GPP% %CXXFLAGS% -c jctvc\TAppEncTop.cpp -o jctvc\obj\TAppEncTop.o
if errorlevel 1 goto error

echo Compiling libmd5...
%GCC% %CFLAGS% -Ijctvc\libmd5 -c jctvc\libmd5\libmd5.c -o jctvc\obj\libmd5.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 5: Creating libjctvc.a
echo ========================================
echo Creating JCTVC static library...
REM Create object file list for JCTVC
dir /b jctvc\obj\*.o > jctvc\obj\jctvc_objlist.txt
set JCTVC_OBJFILES=
for /f %%f in (jctvc\obj\jctvc_objlist.txt) do call set JCTVC_OBJFILES=%%JCTVC_OBJFILES%% jctvc\obj\%%f
%AR% rcs jctvc\libjctvc.a %JCTVC_OBJFILES%
if errorlevel 1 goto error
del jctvc\obj\jctvc_objlist.txt
dir /b jctvc\libjctvc.a 2>nul && echo Built: jctvc\libjctvc.a

echo.
echo ========================================
echo Step 6: Compiling JCTVC glue code
echo ========================================
echo Compiling jctvc_glue.cpp...
%GPP% %CXXFLAGS% -c jctvc_glue.cpp -o obj_native_jctvc\jctvc_glue.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 7: Compiling BPG decoder (libbpg)
echo ========================================
echo Compiling libbpg.c...
%GCC% %CFLAGS% -c libbpg.c -o obj_native_jctvc\libbpg.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 8: Compiling libavutil sources
echo ========================================
for %%f in (libavutil\*.c) do (
    echo Compiling %%~nf.c...
    %GCC% %CFLAGS% -c %%f -o obj_native_jctvc\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 9: Compiling libavcodec sources
echo ========================================
for %%f in (libavcodec\*.c) do (
    echo Compiling %%~nf.c...
    %GCC% %CFLAGS% -c %%f -o obj_native_jctvc\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 10: Compiling BPG Native API
echo ========================================
echo Compiling bpg_api.c...
%GCC% %CFLAGS% -c bpg_api.c -o obj_native_jctvc\bpg_api.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 11: Creating libbpg_native.a with JCTVC
echo ========================================
echo Merging BPG objects + JCTVC into single library...

REM Create object file list for BPG core
dir /b obj_native_jctvc\*.o > obj_native_jctvc\objlist.txt
set OBJFILES=
for /f %%f in (obj_native_jctvc\objlist.txt) do call set OBJFILES=%%OBJFILES%% obj_native_jctvc\%%f

REM Create combined library: BPG objects + extract JCTVC objects
echo Extracting JCTVC objects...
cd jctvc
%AR% x libjctvc.a
cd ..

REM Combine all objects into final library
%AR% rcs libbpg_native.a %OBJFILES% jctvc\*.o
if errorlevel 1 goto error

REM Clean up extracted objects
del jctvc\*.o 2>nul
del obj_native_jctvc\objlist.txt 2>nul

echo.
echo ========================================
echo BPG Native Library with JCTVC build complete!
echo ========================================
echo.
dir /b libbpg_native.a 2>nul && echo Built: libbpg_native.a (with x265 + JCTVC)
for /f %%A in ('dir /b obj_native_jctvc\*.o ^| find /c ".o"') do echo BPG object files: %%A
dir /b jctvc\libjctvc.a 2>nul && echo JCTVC library: jctvc\libjctvc.a
echo.
echo Library now supports:
echo   encoder_type=0: x265 (fast)
echo   encoder_type=1: JCTVC (slow, best quality)
echo.
echo Next: Rebuild Rust codecs to link against updated library
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
echo.
echo Common issues:
echo   - Missing MinGW/MSYS2 g++ compiler
echo   - Missing C++ headers
echo   - Syntax errors in JCTVC source
exit /b 1

:end
