#!/usr/bin/env pwsh
# Simple GPU Thumbnail Diagnostic Tool (No .NET reflection)

Write-Host "=== GPU Thumbnail Diagnostic Tool ===" -ForegroundColor Cyan
Write-Host ""

# ─── Step 1: Check DLL exists ────────────────────────────────────────────
Write-Host "[1/4] Checking bpg_viewer.dll..." -ForegroundColor Yellow
$dllPath = Join-Path $PSScriptRoot "bpg_viewer.dll"
if (!(Test-Path $dllPath)) {
    Write-Host "  ✗ DLL not found!" -ForegroundColor Red
    exit 1
}
$dllInfo = Get-Item $dllPath
Write-Host ("  ✓ DLL found: {0:N2} MB, modified {1}" -f ($dllInfo.Length / 1MB), $dllInfo.LastWriteTime) -ForegroundColor Green
Write-Host ""

# ─── Step 2: Check D3D12 runtime availability ───────────────────────────
Write-Host "[2/4] Checking D3D12 runtime..." -ForegroundColor Yellow
$d3d12Path = "$env:SystemRoot\System32\d3d12.dll"
if (Test-Path $d3d12Path) {
    $d3d12Info = Get-Item $d3d12Path
    Write-Host ("  ✓ D3D12.dll found (version {0})" -f $d3d12Info.VersionInfo.FileVersion) -ForegroundColor Green
} else {
    Write-Host "  ✗ D3D12.dll not found!" -ForegroundColor Red
    Write-Host "  GPU thumbnailing requires Windows 10 or later" -ForegroundColor Gray
}
Write-Host ""

# ─── Step 3: Check GPU hardware ─────────────────────────────────────────
Write-Host "[3/4] Checking GPU hardware..." -ForegroundColor Yellow
try {
    $gpuInfo = Get-WmiObject Win32_VideoController | Where-Object { $_.AdapterRAM -gt 0 } | Select-Object -First 1
    if ($gpuInfo) {
        $vramMB = [math]::Round($gpuInfo.AdapterRAM / 1MB, 0)
        Write-Host ("  ✓ GPU found: {0} ({1} MB VRAM)" -f $gpuInfo.Name, $vramMB) -ForegroundColor Green
        if ($vramMB -lt 512) {
            Write-Host "  ⚠ Low VRAM detected - GPU thumbnailing may be slow" -ForegroundColor Yellow
        }
    } else {
        Write-Host "  ✗ No GPU hardware detected" -ForegroundColor Red
    }
} catch {
    Write-Host "  ⚠ Could not check GPU hardware" -ForegroundColor Yellow
}
Write-Host ""

# ─── Step 4: Cache directory check ──────────────────────────────────────
Write-Host "[4/4] Checking cache directory..." -ForegroundColor Yellow
$cacheDir = "$env:LOCALAPPDATA\OpenArc\Cache\Thumbnails"
if (!(Test-Path $cacheDir)) {
    Write-Host "  ⚠ Cache directory not yet created" -ForegroundColor Yellow
    Write-Host "    Will be created on first thumbnail generation" -ForegroundColor Gray
} else {
    $jpgCount = (Get-ChildItem $cacheDir -Filter "*.jpg" -ErrorAction SilentlyContinue).Count
    $pngCount = (Get-ChildItem $cacheDir -Filter "*.png" -ErrorAction SilentlyContinue).Count
    Write-Host "  ✓ Cache directory exists" -ForegroundColor Green
    Write-Host ("    JPEG thumbnails: {0}" -f $jpgCount) -ForegroundColor Cyan
    Write-Host ("    PNG thumbnails: {0}" -f $pngCount) -ForegroundColor Gray

    if ($pngCount -gt 0) {
        Write-Host ""
        Write-Host "  RECOMMENDATION: Clear legacy PNG cache" -ForegroundColor Yellow
        Write-Host "    Remove-Item '$cacheDir\*.png' -Force" -ForegroundColor Gray
    }
}
Write-Host ""

# ─── Instructions ───────────────────────────────────────────────────────
Write-Host "=== NEXT STEPS ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. Launch DocBrakeGUI.exe" -ForegroundColor Yellow
Write-Host "2. Watch the console output carefully:" -ForegroundColor Yellow
Write-Host ""
Write-Host "   If you see:" -ForegroundColor White
Write-Host "   ============================================" -ForegroundColor Green
Write-Host "   [ThumbnailCache] GPU INIT: ✓ SUCCESS" -ForegroundColor Green
Write-Host "   ============================================" -ForegroundColor Green
Write-Host "   → GPU thumbnailing is working!" -ForegroundColor Green
Write-Host ""
Write-Host "   If you see:" -ForegroundColor White
Write-Host "   ============================================" -ForegroundColor Red
Write-Host "   [ThumbnailCache] GPU INIT: ✗ FAILED (return code: -1)" -ForegroundColor Red
Write-Host "   ============================================" -ForegroundColor Red
Write-Host "   → GPU not available, CPU fallback active" -ForegroundColor Red
Write-Host ""
Write-Host "3. Navigate to a folder with JPEG images" -ForegroundColor Yellow
Write-Host "4. Watch for per-file messages:" -ForegroundColor Yellow
Write-Host "   [GPU] Processing: photo.jpg → [GPU] ✓ SUCCESS: photo.jpg" -ForegroundColor Green
Write-Host "   [CPU] Processing non-JPEG: photo.png (.png)" -ForegroundColor Yellow
Write-Host ""

Write-Host "Press Enter to launch DocBrakeGUI..."
Read-Host

# Launch with console visible
Write-Host "Launching DocBrakeGUI..." -ForegroundColor Cyan
Start-Process -FilePath (Join-Path $PSScriptRoot "DocBrakeGUI.exe") -NoNewWindow -Wait

Write-Host ""
Write-Host "DocBrakeGUI closed. Check the output above for GPU status." -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Enter to exit..."
Read-Host
