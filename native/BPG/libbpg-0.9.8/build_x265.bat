@echo off
REM Build x265 in 8-bit, 10-bit, and 12-bit configurations
REM Required for BPG encoder

echo ========================================
echo Building x265 HEVC encoder
echo ========================================

set "CMAKE=C:\Program Files\CMake3\bin\cmake.exe"
set MAKE=mingw32-make

REM Create build directories
echo Creating build directories...
if not exist x265.out mkdir x265.out
if not exist x265.out\12bit mkdir x265.out\12bit
if not exist x265.out\10bit mkdir x265.out\10bit
if not exist x265.out\8bit mkdir x265.out\8bit

echo.
echo ========================================
echo Step 1: Building x265 12-bit
echo ========================================
cd x265.out\12bit
"%CMAKE%" ..\..\x265\source -G "MinGW Makefiles" ^
    -DCMAKE_C_COMPILER=gcc ^
    -DCMAKE_CXX_COMPILER=g++ ^
    -DENABLE_ASSEMBLY=ON ^
    -DHIGH_BIT_DEPTH=ON ^
    -DEXPORT_C_API=OFF ^
    -DENABLE_SHARED=OFF ^
    -DENABLE_CLI=OFF ^
    -DMAIN12=ON
if errorlevel 1 goto error

%MAKE%
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 2: Building x265 10-bit
echo ========================================
cd ..\10bit
"%CMAKE%" ..\..\x265\source -G "MinGW Makefiles" ^
    -DCMAKE_C_COMPILER=gcc ^
    -DCMAKE_CXX_COMPILER=g++ ^
    -DENABLE_ASSEMBLY=ON ^
    -DHIGH_BIT_DEPTH=ON ^
    -DEXPORT_C_API=OFF ^
    -DENABLE_SHARED=OFF ^
    -DENABLE_CLI=OFF ^
    -DMAIN10=ON
if errorlevel 1 goto error

%MAKE%
if errorlevel 1 goto error

echo.
echo ========================================
echo Step 3: Building x265 8-bit (main)
echo ========================================
cd ..\8bit
"%CMAKE%" ..\..\x265\source -G "MinGW Makefiles" ^
    -DCMAKE_C_COMPILER=gcc ^
    -DCMAKE_CXX_COMPILER=g++ ^
    -DENABLE_ASSEMBLY=ON ^
    -DLINKED_10BIT=ON ^
    -DLINKED_12BIT=ON ^
    -DENABLE_SHARED=OFF ^
    -DENABLE_CLI=OFF
if errorlevel 1 goto error

%MAKE%
if errorlevel 1 goto error

cd ..\..

echo.
echo ========================================
echo x265 build complete!
echo ========================================
echo.
echo Built libraries:
dir /b x265.out\12bit\libx265.a 2>nul && echo   - x265.out\12bit\libx265.a (12-bit)
dir /b x265.out\10bit\libx265.a 2>nul && echo   - x265.out\10bit\libx265.a (10-bit)
dir /b x265.out\8bit\libx265.a 2>nul && echo   - x265.out\8bit\libx265.a (8-bit main)
echo.
echo Next step: Build BPG encoder with x265 support
echo Run: build_bpg_with_x265.bat
goto end

:error
echo.
echo ========================================
echo Build FAILED!
echo ========================================
echo Check the error messages above.
cd ..\..
exit /b 1

:end
