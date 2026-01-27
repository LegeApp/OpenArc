@echo off
REM Build BPG Native Library (Simple version)
REM Uses existing libbpg.a and just adds API wrapper

echo ========================================
echo Building BPG Native Library (Simple)
echo ========================================

set GCC=gcc
set AR=ar

REM Compiler flags
set CFLAGS=-O3 -Wall -I.

echo.
echo ========================================
echo Step 1: Compiling BPG Native API wrapper
echo ========================================
echo Compiling bpg_api.c...
%GCC% %CFLAGS% -c bpg_api.c -o bpg_api.o
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 2: Creating libbpg_native.a
echo ========================================
echo Combining with existing libbpg.a...

REM Extract objects from existing libbpg.a
if not exist temp_obj mkdir temp_obj
cd temp_obj
%AR% x ..\libbpg.a
if errorlevel 1 (
    cd ..
    goto error
)
cd ..

REM Create new library with all objects
REM Build object file list
dir /b temp_obj\*.o > temp_obj\objlist.txt
set OBJFILES=bpg_api.o
for /f %%f in (temp_obj\objlist.txt) do call set OBJFILES=%%OBJFILES%% temp_obj\%%f
%AR% rcs libbpg_native.a %OBJFILES%
if errorlevel 1 goto error
del temp_obj\objlist.txt

REM Cleanup
rmdir /s /q temp_obj

echo.
echo ========================================
echo BPG Native Library build complete!
echo ========================================
echo.
dir /b libbpg_native.a 2>nul && echo Built: libbpg_native.a
echo.
echo This library includes:
echo   - BPG decoder (from libbpg.a)
echo   - BPG Native API (bpg_api.c)
echo.
echo Link with: -lbpg_native -lpng -ljpeg -lz
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
if exist temp_obj rmdir /s /q temp_obj
exit /b 1

:end
