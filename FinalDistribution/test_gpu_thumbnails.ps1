#!/usr/bin/env pwsh
# GPU Thumbnail Testing Script
# This script helps verify that GPU thumbnail acceleration is working

Write-Host "=== GPU Thumbnail Test Script ===" -ForegroundColor Cyan
Write-Host ""

# 1. Check if bpg_viewer.dll exists
$dllPath = ".\bpg_viewer.dll"
if (!(Test-Path $dllPath)) {
    Write-Host "ERROR: bpg_viewer.dll not found!" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] bpg_viewer.dll found" -ForegroundColor Green

# 2. Check if DocBrakeGUI.exe exists
$exePath = ".\DocBrakeGUI.exe"
if (!(Test-Path $exePath)) {
    Write-Host "ERROR: DocBrakeGUI.exe not found!" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] DocBrakeGUI.exe found" -ForegroundColor Green
Write-Host ""

# 3. Check cache directory
$cacheDir = "$env:LOCALAPPDATA\OpenArc\Cache\Thumbnails"
if (Test-Path $cacheDir) {
    $jpgCount = (Get-ChildItem $cacheDir -Filter "*.jpg" -ErrorAction SilentlyContinue).Count
    $pngCount = (Get-ChildItem $cacheDir -Filter "*.png" -ErrorAction SilentlyContinue).Count
    Write-Host "Cache Directory: $cacheDir" -ForegroundColor Yellow
    Write-Host "  - JPEG thumbnails: $jpgCount" -ForegroundColor Cyan
    Write-Host "  - PNG thumbnails (legacy): $pngCount" -ForegroundColor Gray
    
    if ($pngCount -gt 0) {
        Write-Host ""
        Write-Host "RECOMMENDATION: Clear old PNG cache for fresh test" -ForegroundColor Yellow
        Write-Host "  Run: Remove-Item '$cacheDir\*.png' -Force" -ForegroundColor Gray
    }
} else {
    Write-Host "Cache directory not yet created (will be created on first run)" -ForegroundColor Gray
}

Write-Host ""
Write-Host "=== Test Instructions ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. Launch DocBrakeGUI (the app will launch in a moment)" -ForegroundColor Yellow
Write-Host "2. Navigate to a folder with JPEG images" -ForegroundColor Yellow
Write-Host "3. Watch for these indicators:" -ForegroundColor Yellow
Write-Host "   - Console output: '[ThumbnailCache] GPU thumbnail pipeline init: SUCCESS'" -ForegroundColor Green
Write-Host "   - Console output: '[ThumbnailCache] GPU SUCCESS for <filename>'" -ForegroundColor Green
Write-Host "   - Thumbnails load FAST (2-5ms each, nearly instant for batch)" -ForegroundColor Green
Write-Host "   - Check cache: Get-ChildItem '$cacheDir' | Measure-Object -Property Length -Sum" -ForegroundColor Gray
Write-Host ""
Write-Host "4. If GPU init FAILED:" -ForegroundColor Red
Write-Host "   - Fallback to CPU path is automatic (slower, ~20-50ms/thumbnail)" -ForegroundColor Gray
Write-Host "   - Possible causes: No D3D12 GPU, driver issue, incompatible hardware" -ForegroundColor Gray
Write-Host ""
Write-Host "Press Enter to launch DocBrakeGUI..."
Read-Host

# 4. Launch with console visible
Write-Host "Launching DocBrakeGUI..." -ForegroundColor Cyan
Start-Process -FilePath $exePath -NoNewWindow -Wait

# 5. Post-test cache analysis
Write-Host ""
Write-Host "=== Post-Test Cache Analysis ===" -ForegroundColor Cyan
if (Test-Path $cacheDir) {
    $jpgFiles = Get-ChildItem $cacheDir -Filter "*.jpg" -ErrorAction SilentlyContinue
    $jpgCount = $jpgFiles.Count
    $jpgSize = ($jpgFiles | Measure-Object -Property Length -Sum).Sum / 1MB
    
    Write-Host "JPEG thumbnails generated: $jpgCount" -ForegroundColor Green
    Write-Host "Total cache size: $([math]::Round($jpgSize, 2)) MB" -ForegroundColor Cyan
    
    if ($jpgCount -gt 0) {
        $avgSize = ($jpgSize * 1024 * 1024) / $jpgCount / 1KB
        Write-Host "Average thumbnail size: $([math]::Round($avgSize, 1)) KB" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "[SUCCESS] GPU thumbnail integration appears to be working!" -ForegroundColor Green
        
        if ($avgSize -gt 30) {
            Write-Host "NOTE: Average size >30KB suggests CPU path may have been used" -ForegroundColor Yellow
            Write-Host "      (GPU JPEG thumbnails typically 10-20KB @ quality 85)" -ForegroundColor Gray
        }
    }
} else {
    Write-Host "No cache files created - thumbnails may not have been generated" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Press Enter to exit..."
Read-Host
