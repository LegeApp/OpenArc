@echo off
REM BPG Viewer Build and Deploy Script (Batch version)
REM Builds the Rust library and C# GUI, then copies to test folder

setlocal enabledelayedexpansion

echo =====================================
echo BPG Viewer Build and Deploy
echo =====================================
echo.

REM Step 1: Build Rust library
echo [1/3] Building Rust library...
cargo build --release --lib

if errorlevel 1 (
    echo ERROR: Rust build failed!
    pause
    exit /b 1
)

echo   * Rust library built successfully
echo.

REM Step 2: Build C# GUI
echo [2/3] Building C# GUI...
dotnet build BpgViewerGUI\BpgViewerGUI.csproj -c Release

if errorlevel 1 (
    echo ERROR: C# build failed!
    pause
    exit /b 1
)

echo   * C# GUI built successfully
echo.

REM Step 3: Copy to test folder
echo [3/3] Deploying to test folder...

set "TEST_FOLDER=D:\misc\bpg-viewer-test"
set "OUTPUT_DIR=BpgViewerGUI\bin\Release\net8.0-windows"

if not exist "%TEST_FOLDER%" (
    echo   Creating test folder: %TEST_FOLDER%
    mkdir "%TEST_FOLDER%"
)

echo   Copying files from: %OUTPUT_DIR%
echo   Copying files to: %TEST_FOLDER%

xcopy /Y /E /I "%OUTPUT_DIR%\*" "%TEST_FOLDER%\" >nul 2>&1

REM Copy documentation
if exist "LESSONS_LEARNED_IMAGE_RENDERING.md" (
    copy /Y "LESSONS_LEARNED_IMAGE_RENDERING.md" "%TEST_FOLDER%\" >nul 2>&1
    echo   * Copied documentation
)

echo   * Files deployed successfully
echo.

echo =====================================
echo Build Complete!
echo =====================================
echo.
echo Test folder: %TEST_FOLDER%
echo.
echo To run the application:
echo   cd "%TEST_FOLDER%"
echo   BpgViewerGUI.exe
echo.

pause
