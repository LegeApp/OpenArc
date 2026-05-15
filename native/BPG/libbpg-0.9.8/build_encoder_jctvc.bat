@echo off
REM Build BPG encoder with JCTVC support
REM This builds the encoder without requiring x265 or CMake

echo Building BPG encoder with JCTVC...

REM Set base compiler flags
set BASE_CFLAGS=-Os -Wall -fno-asynchronous-unwind-tables -fdata-sections -ffunction-sections
set BASE_CFLAGS=%BASE_CFLAGS% -fno-math-errno -fno-signed-zeros -fno-tree-vectorize -fomit-frame-pointer
set BASE_CFLAGS=%BASE_CFLAGS% -D_FILE_OFFSET_BITS=64 -D_LARGEFILE_SOURCE -D_REENTRANT
set BASE_CFLAGS=%BASE_CFLAGS% -I. -DCONFIG_BPG_VERSION=\"0.9.8\"

REM JCTVC specific flags
set JCTVC_CFLAGS=-I./jctvc -I./jctvc/TLibCommon -I./jctvc/TLibEncoder -I./jctvc/TLibVideoIO -I./jctvc/libmd5
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -Wno-sign-compare -Wno-unused-parameter -Wno-missing-field-initializers
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -Wno-misleading-indentation -Wno-class-memaccess
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -DMSYS_PROJECT -D_MSYS2 -D_CRT_SECURE_NO_DEPRECATE -D_CRT_SECURE_NO_WARNINGS
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -D_CRT_NONSTDC_NO_WARNINGS -D_WIN32_WINNT=0x0600 -DUSE_JCTVC
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -D_ISOC99_SOURCE -D_GNU_SOURCE -DHAVE_STRING_H -DHAVE_STDINT_H
set JCTVC_CFLAGS=%JCTVC_CFLAGS% -DHAVE_INTTYPES_H -DHAVE_MALLOC_H -D__STDC_LIMIT_MACROS

set CXXFLAGS=%BASE_CFLAGS% %JCTVC_CFLAGS% -std=c++11

echo Compiling JCTVC TLibCommon...
for %%f in (jctvc\TLibCommon\*.cpp) do (
    echo   Compiling %%f
    g++ %CXXFLAGS% -c %%f -o %%~dpnf.o
    if errorlevel 1 goto error
)

echo Compiling JCTVC TLibEncoder...
for %%f in (jctvc\TLibEncoder\*.cpp) do (
    echo   Compiling %%f
    g++ %CXXFLAGS% -c %%f -o %%~dpnf.o
    if errorlevel 1 goto error
)

echo Compiling JCTVC TLibVideoIO...
for %%f in (jctvc\TLibVideoIO\*.cpp) do (
    echo   Compiling %%f
    g++ %CXXFLAGS% -c %%f -o %%~dpnf.o
    if errorlevel 1 goto error
)

echo Compiling JCTVC libmd5...
for %%f in (jctvc\libmd5\*.c) do (
    echo   Compiling %%f
    gcc %BASE_CFLAGS% %JCTVC_CFLAGS% -c %%f -o %%~dpnf.o
    if errorlevel 1 goto error
)

echo Compiling JCTVC main files...
g++ %CXXFLAGS% -c jctvc/TAppEncCfg.cpp -o jctvc/TAppEncCfg.o
if errorlevel 1 goto error
g++ %CXXFLAGS% -c jctvc/TAppEncTop.cpp -o jctvc/TAppEncTop.o
if errorlevel 1 goto error
g++ %CXXFLAGS% -c jctvc/program_options_lite.cpp -o jctvc/program_options_lite.o
if errorlevel 1 goto error

echo Creating JCTVC static library...
REM Create list of all object files
dir /b /s jctvc\*.o > objfiles.txt
ar rcs jctvc/libjctvc.a @objfiles.txt
del objfiles.txt
if errorlevel 1 goto error

echo Compiling jctvc_glue.cpp...
g++ %CXXFLAGS% -c jctvc_glue.cpp -o jctvc_glue.o
if errorlevel 1 goto error

echo Compiling bpgenc.c with JCTVC support...
gcc %BASE_CFLAGS% -DUSE_JCTVC -c bpgenc.c -o bpgenc.o
if errorlevel 1 goto error

echo Linking bpgenc.exe...
g++ -o bpgenc.exe bpgenc.o jctvc_glue.o jctvc/libjctvc.a -lpng -ljpeg -lm -lstdc++ -lpthread
if errorlevel 1 goto error

echo.
echo ========================================
echo Build complete!
echo ========================================
echo bpgenc.exe created with JCTVC encoder support
echo.
echo Test with:
echo   bpgenc.exe -o output.bpg input.jpg
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
