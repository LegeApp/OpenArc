@echo off
REM Build BPG Native Library
REM Provides C API for direct FFI integration (no subprocess)

echo ========================================
echo Building BPG Native Library
echo ========================================

set GCC=gcc
set AR=ar

REM Compiler flags
set CFLAGS=-O3 -Wall -I. -Ilibavutil -Ilibavcodec -DFF_MEMORY_POISON=0x2a

REM Create output directory
if not exist obj_native mkdir obj_native

echo.
echo ========================================
echo Step 1: Compiling BPG decoder (libbpg)
echo ========================================
echo Compiling libbpg.c...
%GCC% %CFLAGS% -c libbpg.c -o obj_native\libbpg.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 2: Compiling libavutil sources
echo ========================================
for %%f in (libavutil\*.c) do (
    echo Compiling %%~nf.c...
    %GCC% %CFLAGS% -c %%f -o obj_native\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 3: Compiling libavcodec sources
echo ========================================
for %%f in (libavcodec\*.c) do (
    echo Compiling %%~nf.c...
    %GCC% %CFLAGS% -c %%f -o obj_native\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 4: Compiling BPG Native API
echo ========================================
echo Compiling bpg_api.c...
%GCC% %CFLAGS% -c bpg_api.c -o obj_native\bpg_api.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 5: Creating libbpg_native.a
echo ========================================
echo Creating static library...
REM Create object file list
dir /b obj_native\*.o > obj_native\objlist.txt
set OBJFILES=
for /f %%f in (obj_native\objlist.txt) do call set OBJFILES=%%OBJFILES%% obj_native\%%f
%AR% rcs libbpg_native.a %OBJFILES%
if errorlevel 1 goto error
del obj_native\objlist.txt

echo.
echo ========================================
echo BPG Native Library build complete!
echo ========================================
echo.
dir /b libbpg_native.a 2>nul && echo Built: libbpg_native.a
for /f %%A in ('dir /b obj_native\*.o ^| find /c ".o"') do echo Object files: %%A
echo.
echo Next step: Build test executable or integrate with Rust
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
exit /b 1

:end
