@echo off
REM Build JCTVC HEVC encoder library for BPG
REM JCTVC provides better compression than x265 but slower encoding

echo ========================================
echo Building JCTVC HEVC Encoder Library
echo ========================================

set GCC=gcc
set GPP=g++
set AR=ar

REM Compiler flags for JCTVC
set CXXFLAGS=-O3 -Wall -fno-strict-aliasing -std=c++11 -DMSYS_UNIX -DMSYS_WIN32

REM Include paths
set INCLUDES=-I. -Ijctvc -Ijctvc/TLibCommon -Ijctvc/TLibEncoder -Ijctvc/TLibVideoIO -Ijctvc/libmd5

REM Create output directory
if not exist jctvc\obj mkdir jctvc\obj

echo.
echo ========================================
echo Step 1: Compiling TLibCommon
echo ========================================
for %%f in (jctvc\TLibCommon\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 2: Compiling TLibEncoder
echo ========================================
for %%f in (jctvc\TLibEncoder\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 3: Compiling TLibVideoIO
echo ========================================
for %%f in (jctvc\TLibVideoIO\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 4: Compiling libmd5
echo ========================================
for %%f in (jctvc\libmd5\*.c) do (
    echo Compiling %%~nf.c...
    %GCC% -O3 -Wall %INCLUDES% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 5: Compiling JCTVC top-level
echo ========================================
for %%f in (jctvc\*.cpp) do (
    echo Compiling %%~nf.cpp...
    %GPP% %CXXFLAGS% %INCLUDES% -c %%f -o jctvc\obj\%%~nf.o
    if errorlevel 1 goto error
)

echo.
echo ========================================
echo Step 6: Creating libjctvc.a
echo ========================================
echo Creating static library...
REM Create object file list for ar (Windows batch doesn't expand *.o)
dir /b jctvc\obj\*.o > jctvc\obj\objlist.txt
set OBJFILES=
for /f %%f in (jctvc\obj\objlist.txt) do call set OBJFILES=%%OBJFILES%% jctvc\obj\%%f
%AR% rcs jctvc\libjctvc.a %OBJFILES%
if errorlevel 1 goto error
del jctvc\obj\objlist.txt

echo.
echo ========================================
echo JCTVC build complete!
echo ========================================
echo.
dir /b jctvc\libjctvc.a 2>nul && echo Built: jctvc\libjctvc.a
for /f %%A in ('dir /b jctvc\obj\*.o ^| find /c ".o"') do echo Object files: %%A
echo.
echo Next step: Build BPG encoder with JCTVC support
echo Run: build_bpg_with_jctvc.bat
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
exit /b 1

:end
