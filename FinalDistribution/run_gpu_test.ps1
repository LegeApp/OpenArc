#!/usr/bin/env pwsh
# Test GPU Thumbnailing with Enhanced Logging
# This script shows GPU vs CPU path selection in real-time

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "GPU Thumbnailing Test" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$distFolder = "D:\misc\arc\openarc\FinalDistribution"
$srcDll = "D:\misc\arc\openarc\DocBrakeGUI\obj\Release\DocBrakeGUI.dll"

# Copy latest build
Write-Host "[1/2] Copying latest build..." -ForegroundColor Yellow
if (Test-Path $srcDll) {
    Copy-Item $srcDll "$distFolder\DocBrakeGUI.dll" -Force
    Write-Host "      [OK] DocBrakeGUI.dll copied" -ForegroundColor Green
} else {
    Write-Host "      [ERROR] Source DLL not found" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "[2/2] Launching DocBrakeGUI with enhanced logging..." -ForegroundColor Yellow
Write-Host ""
Write-Host "WHAT TO LOOK FOR:" -ForegroundColor Cyan
Write-Host "  [GPU] Processing: ..." -ForegroundColor Magenta
Write-Host "    > GPU path is being used for JPEGs" -ForegroundColor White
Write-Host ""
Write-Host "  [GPU] [OK] SUCCESS: ..." -ForegroundColor Green
Write-Host "    > JPEG was successfully processed on GPU (2-5ms expected)" -ForegroundColor White
Write-Host ""
Write-Host "  [GPU] [FAIL] FAILED: ... [CPU] Falling back" -ForegroundColor Red
Write-Host "    > GPU path failed, CPU fallback was used (50ms expected)" -ForegroundColor White
Write-Host ""
Write-Host "  [CPU] Processing non-JPEG: ..." -ForegroundColor Yellow
Write-Host "    > This is a PNG/other format, always uses CPU" -ForegroundColor White
Write-Host ""
Write-Host "PERFORMANCE:" -ForegroundColor Cyan
Write-Host "  GPU should be 5-10x faster than CPU for JPEG thumbnails" -ForegroundColor White
Write-Host "  Expected times:" -ForegroundColor White
Write-Host "    GPU: 2-5ms per JPEG (200-500ms total for 100 files)" -ForegroundColor Green
Write-Host "    CPU: 50-100ms per file (5-10 seconds total for 100 files)" -ForegroundColor Red
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

& "$distFolder\DocBrakeGUI.exe"

