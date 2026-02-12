@echo off
echo === GPU Thumbnail Diagnostic Tool ===
echo.

REM ─── Step 1: Check DLL exists ────────────────────────────────────────────
echo [1/4] Checking bpg_viewer.dll...
if not exist "bpg_viewer.dll" (
    echo   ✗ DLL not found!
    pause
    exit /b 1
)
for %%A in ("bpg_viewer.dll") do echo   ✓ DLL found: %%~zA bytes
echo.

REM ─── Step 2: Check D3D12 runtime availability ───────────────────────────
echo [2/4] Checking D3D12 runtime...
if exist "%SystemRoot%\System32\d3d12.dll" (
    echo   ✓ D3D12.dll found
) else (
    echo   ✗ D3D12.dll not found!
    echo   GPU thumbnailing requires Windows 10 or later
)
echo.

REM ─── Step 3: Check GPU hardware ─────────────────────────────────────────
echo [3/4] Checking GPU hardware...
wmic path win32_VideoController get name /value 2>nul | findstr "Name=" >nul
if %errorlevel% equ 0 (
    for /f "tokens=2 delims==" %%i in ('wmic path win32_VideoController get name /value ^| findstr "Name="') do (
        echo   ✓ GPU found: %%i
        goto :gpu_found
    )
) else (
    echo   ⚠ Could not check GPU hardware
)
:gpu_found
echo.

REM ─── Step 4: Cache directory check ──────────────────────────────────────
echo [4/4] Checking cache directory...
if exist "%LOCALAPPDATA%\OpenArc\Cache\Thumbnails" (
    echo   ✓ Cache directory exists
    dir /b "%LOCALAPPDATA%\OpenArc\Cache\Thumbnails\*.jpg" 2>nul | find /c ".jpg" > temp_count.txt
    set /p jpg_count=<temp_count.txt
    dir /b "%LOCALAPPDATA%\OpenArc\Cache\Thumbnails\*.png" 2>nul | find /c ".png" > temp_count.txt
    set /p png_count=<temp_count.txt
    del temp_count.txt 2>nul
    echo     JPEG thumbnails: %jpg_count%
    echo     PNG thumbnails: %png_count%
    if %png_count% gtr 0 (
        echo.
        echo   RECOMMENDATION: Clear legacy PNG cache
        echo     Remove-Item '$env:LOCALAPPDATA\OpenArc\Cache\Thumbnails\*.png' -Force
    )
) else (
    echo   ⚠ Cache directory not yet created
    echo     Will be created on first thumbnail generation
)
echo.

REM ─── Instructions ───────────────────────────────────────────────────────
echo === NEXT STEPS ===
echo.
echo 1. Launch DocBrakeGUI.exe
echo 2. Watch the console output carefully:
echo.
echo    If you see:
echo    ============================================
echo    [ThumbnailCache] GPU INIT: ✓ SUCCESS
echo    ============================================
echo    → GPU thumbnailing is working!
echo.
echo    If you see:
echo    ============================================
echo    [ThumbnailCache] GPU INIT: ✗ FAILED (return code: -1)
echo    ============================================
echo    → GPU not available, CPU fallback active
echo.
echo 3. Navigate to a folder with JPEG images
echo 4. Watch for per-file messages:
echo    [GPU] Processing: photo.jpg → [GPU] ✓ SUCCESS: photo.jpg
echo    [CPU] Processing non-JPEG: photo.png (.png)
echo.
echo Press any key to launch DocBrakeGUI...
pause >nul

REM Launch with console visible
echo.
echo Launching DocBrakeGUI...
DocBrakeGUI.exe

echo.
echo DocBrakeGUI closed. Check the output above for GPU status.
echo.
pause