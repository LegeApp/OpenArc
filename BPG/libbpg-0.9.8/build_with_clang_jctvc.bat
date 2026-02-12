@echo off
REM Build BPG Native Library with JCTVC using Clang (workaround for GCC 15.2 time.h bug)

echo ========================================
echo Building BPG with JCTVC using Clang
echo ========================================

set CC=D:\tools\clang\bin\clang.exe
set CXX=D:\tools\clang\bin\clang++.exe
set AR=ar

REM Clang flags - MSVC-compatible mode with MinGW libraries
set BASE_CFLAGS=-O3 -Wall -I. -Ilibavutil -Ilibavcodec -DFF_MEMORY_POISON=0x2a -DUSE_JCTVC
set BASE_CFLAGS=%BASE_CFLAGS% -target x86_64-w64-mingw32 -fms-compatibility-version=19
set CFLAGS=%BASE_CFLAGS% -DCONFIG_BPG_VERSION=\"0.9.8\"
set CXXFLAGS=%BASE_CFLAGS% -std=c++11 -Ijctvc -Ijctvc/TLibCommon -Ijctvc/TLibEncoder -Ijctvc/TLibVideoIO -Ijctvc/libmd5
set CXXFLAGS=%CXXFLAGS% -Wno-sign-compare -Wno-unused-parameter -Wno-missing-field-initializers

REM Create output directories
if not exist obj_clang_jctvc mkdir obj_clang_jctvc
if not exist jctvc\obj mkdir jctvc\obj

echo Using: %CXX%
%CXX% --version

echo.
echo ========================================
echo Step 1: Compiling JCTVC TLibCommon
echo ========================================
for %%f in (jctvc\TLibCommon\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %CXX% %CXXFLAGS% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 2: Compiling JCTVC TLibEncoder
echo ========================================
for %%f in (jctvc\TLibEncoder\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %CXX% %CXXFLAGS% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 3: Compiling JCTVC TLibVideoIO
echo ========================================
for %%f in (jctvc\TLibVideoIO\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %CXX% %CXXFLAGS% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 4: Compiling JCTVC support files
echo ========================================
echo Compiling program_options_lite.cpp...
%CXX% %CXXFLAGS% -c jctvc\program_options_lite.cpp -o jctvc\obj\program_options_lite.o
if errorlevel 1 goto error

echo Compiling TAppEncCfg.cpp...
%CXX% %CXXFLAGS% -c jctvc\TAppEncCfg.cpp -o jctvc\obj\TAppEncCfg.o
if errorlevel 1 goto error

echo Compiling TAppEncTop.cpp...
%CXX% %CXXFLAGS% -c jctvc\TAppEncTop.cpp -o jctvc\obj\TAppEncTop.o
if errorlevel 1 goto error

echo Compiling libmd5...
%CC% %CFLAGS% -Ijctvc\libmd5 -c jctvc\libmd5\libmd5.c -o jctvc\obj\libmd5.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 5: Creating libjctvc.a
echo ========================================
echo Creating JCTVC static library...
%AR% rcs jctvc\libjctvc.a jctvc\obj\*.o
if errorlevel 1 goto error
dir /b jctvc\libjctvc.a 2>nul && echo Built: jctvc\libjctvc.a

echo.
echo ========================================
echo Step 6: Compiling JCTVC glue code
echo ========================================
echo Compiling jctvc_glue.cpp...
%CXX% %CXXFLAGS% -c jctvc_glue.cpp -o obj_clang_jctvc\jctvc_glue.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 7: Compiling BPG decoder (libbpg)
echo ========================================
echo Compiling libbpg.c...
%CC% %CFLAGS% -c libbpg.c -o obj_clang_jctvc\libbpg.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 8: Compiling libavutil sources
echo ========================================
for %%f in (libavutil\*.c) do (
    echo Compiling %%~nf.c...
    %CC% %CFLAGS% -c %%f -o obj_clang_jctvc\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 9: Compiling libavcodec sources
echo ========================================
for %%f in (libavcodec\*.c) do (
    echo Compiling %%~nf.c...
    %CC% %CFLAGS% -c %%f -o obj_clang_jctvc\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 10: Compiling BPG Native API
echo ========================================
echo Compiling bpg_api.c...
%CC% %CFLAGS% -c bpg_api.c -o obj_clang_jctvc\bpg_api.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 11: Creating libbpg_native.a with JCTVC
echo ========================================
echo Merging BPG objects + JCTVC into single library...

dir /b obj_clang_jctvc\*.o > obj_clang_jctvc\objlist.txt
set OBJFILES=
for /f %%f in (obj_clang_jctvc\objlist.txt) do call set OBJFILES=%%OBJFILES%% obj_clang_jctvc\%%f

echo Extracting JCTVC objects...
cd jctvc
%AR% x libjctvc.a
cd ..

%AR% rcs libbpg_native.a %OBJFILES% jctvc\*.o
if errorlevel 1 goto error

del jctvc\*.o 2>nul
del obj_clang_jctvc\objlist.txt 2>nul

echo.
echo ========================================
echo SUCCESS! BPG with JCTVC built using Clang
echo ========================================
echo.
dir /b libbpg_native.a 2>nul && echo Built: libbpg_native.a (x265 + JCTVC)
echo.
echo Library supports:
echo   encoder_type=0: x265
echo   encoder_type=1: JCTVC
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
exit /b 1

:end
