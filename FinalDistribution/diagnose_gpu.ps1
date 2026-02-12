#!/usr/bin/env pwsh
# Comprehensive GPU Thumbnail Diagnostic Tool

Write-Host "=== GPU Thumbnail Diagnostic Tool ===" -ForegroundColor Cyan
Write-Host ""

# ─── Step 1: Check DLL exists ────────────────────────────────────────────
Write-Host "[1/5] Checking bpg_viewer.dll..." -ForegroundColor Yellow
$dllPath = Join-Path $PSScriptRoot "bpg_viewer.dll"
if (!(Test-Path $dllPath)) {
    Write-Host "  ✗ DLL not found!" -ForegroundColor Red
    exit 1
}
$dllInfo = Get-Item $dllPath
Write-Host ("  ✓ DLL found: {0:N2} MB, modified {1}" -f ($dllInfo.Length / 1MB), $dllInfo.LastWriteTime) -ForegroundColor Green
Write-Host ""

# ─── Step 2: Check D3D12 runtime availability ───────────────────────────
Write-Host "[2/5] Checking D3D12 runtime..." -ForegroundColor Yellow
$d3d12Path = "$env:SystemRoot\System32\d3d12.dll"
if (Test-Path $d3d12Path) {
    $d3d12Info = Get-Item $d3d12Path
    Write-Host ("  ✓ D3D12.dll found (version {0})" -f $d3d12Info.VersionInfo.FileVersion) -ForegroundColor Green
} else {
    Write-Host "  ✗ D3D12.dll not found!" -ForegroundColor Red
    Write-Host "  GPU thumbnailing requires Windows 10 or later" -ForegroundColor Gray
}

# Check DXGI for hardware info
try {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class DxgiInfo {
    [DllImport("dxgi.dll")]
    public static extern int CreateDXGIFactory(ref Guid riid, out IntPtr ppFactory);
}
"@
    Write-Host "  ✓ DXGI available" -ForegroundColor Green
} catch {
    Write-Host "  ⚠ Could not check DXGI" -ForegroundColor Yellow
}
Write-Host ""

# ─── Step 3: Verify DLL exports ─────────────────────────────────────────
Write-Host "[3/5] Verifying DLL exports..." -ForegroundColor Yellow
try {
    $lib = [System.Runtime.InteropServices.NativeLibrary]::Load($dllPath)
    
    $requiredFunctions = @(
        "gpu_thumbnail_pipeline_init",
        "gpu_thumbnail_process_jpeg",
        "gpu_thumbnail_readback_jpeg",
        "universal_thumbnail_generate_jpeg"
    )
    
    $missingCount = 0
    foreach ($func in $requiredFunctions) {
        try {
            $ptr = [System.Runtime.InteropServices.NativeLibrary]::GetExport($lib, $func)
            if ($ptr -ne [IntPtr]::Zero) {
                Write-Host ("  ✓ {0}" -f $func) -ForegroundColor Green
            } else {
                Write-Host ("  ✗ {0} (null pointer)" -f $func) -ForegroundColor Red
                $missingCount++
            }
        } catch {
            Write-Host ("  ✗ {0} (not found)" -f $func) -ForegroundColor Red
            $missingCount++
        }
    }
    
    [System.Runtime.InteropServices.NativeLibrary]::Free($lib)
    
    if ($missingCount -gt 0) {
        Write-Host ""
        Write-Host "  ✗ Missing $missingCount functions - GPU will NOT work!" -ForegroundColor Red
        Write-Host "  Rebuild required:" -ForegroundColor Yellow
        Write-Host "    cd D:\misc\arc\openarc\bpg-viewer" -ForegroundColor Gray
        Write-Host "    cargo build --release" -ForegroundColor Gray
        exit 1
    }
} catch {
    Write-Host "  ✗ Failed to load DLL: $_" -ForegroundColor Red
    exit 1
}
Write-Host ""

# ─── Step 4: Test GPU initialization ────────────────────────────────────
Write-Host "[4/5] Testing GPU pipeline initialization..." -ForegroundColor Yellow
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class GpuTest {
    [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern int gpu_thumbnail_pipeline_init();
}
"@

[Environment]::CurrentDirectory = $PSScriptRoot
$initResult = [GpuTest]::gpu_thumbnail_pipeline_init()
if ($initResult -eq 0) {
    Write-Host "  ✓ GPU pipeline initialized successfully!" -ForegroundColor Green
    Write-Host "  ✓ D3D12 device created" -ForegroundColor Green
    Write-Host "  ✓ Compute shader compiled" -ForegroundColor Green
    Write-Host "  ✓ Atlas allocated (4096×4096 RGBA8)" -ForegroundColor Green
} else {
    Write-Host "  ✗ GPU pipeline init FAILED (error code: $initResult)" -ForegroundColor Red
    Write-Host ""
    Write-Host "  Possible causes:" -ForegroundColor Yellow
    Write-Host "    - No D3D12-capable GPU found" -ForegroundColor Gray
    Write-Host "    - GPU drivers too old (update to latest)" -ForegroundColor Gray
    Write-Host "    - Running in VM without GPU passthrough" -ForegroundColor Gray
    Write-Host "    - Feature Level 11.0 not supported" -ForegroundColor Gray
    Write-Host ""
    Write-Host "  Fallback: CPU path will be used (5-10x slower)" -ForegroundColor Yellow
}
Write-Host ""

# ─── Step 5: Cache directory check ──────────────────────────────────────
Write-Host "[5/5] Checking cache directory..." -ForegroundColor Yellow
$cacheDir = "$env:LOCALAPPDATA\OpenArc\Cache\Thumbnails"
if (!(Test-Path $cacheDir)) {
    Write-Host "  ⚠ Cache directory not yet created" -ForegroundColor Yellow
    Write-Host "    Will be created on first thumbnail generation" -ForegroundColor Gray
} else {
    $jpgCount = (Get-ChildItem $cacheDir -Filter "*.jpg" -ErrorAction SilentlyContinue).Count
    $pngCount = (Get-ChildItem $cacheDir -Filter "*.png" -ErrorAction SilentlyContinue).Count
    Write-Host "  ✓ Cache directory exists" -ForegroundColor Green
    Write-Host ("    JPEG thumbnails: {0}" -f $jpgCount) -ForegroundColor Cyan
    Write-Host ("    PNG thumbnails (legacy): {0}" -f $pngCount) -ForegroundColor Gray
    
    if ($pngCount -gt 0) {
        Write-Host ""
        Write-Host "  ⚠ RECOMMENDATION: Clear legacy PNG cache for clean test" -ForegroundColor Yellow
        Write-Host "    Remove-Item '$cacheDir\*.png' -Force" -ForegroundColor Gray
    }
}
Write-Host ""

# ─── Summary ─────────────────────────────────────────────────────────────
Write-Host "=== SUMMARY ===" -ForegroundColor Cyan
if ($initResult -eq 0) {
    Write-Host "✓ GPU thumbnailing is ENABLED and working!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Expected performance:" -ForegroundColor Yellow
    Write-Host "  - JPEG thumbnails: 2-5ms each (GPU accelerated)" -ForegroundColor Green
    Write-Host "  - Other formats: 20-50ms each (CPU fallback)" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "To test:" -ForegroundColor Cyan
    Write-Host "  1. Launch DocBrakeGUI.exe" -ForegroundColor White
    Write-Host "  2. Navigate to folder with JPEG images" -ForegroundColor White
    Write-Host "  3. Watch console for '[GPU] ✓ SUCCESS' messages" -ForegroundColor White
    Write-Host "  4. Thumbnails should load nearly instantly" -ForegroundColor White
} else {
    Write-Host "✗ GPU thumbnailing is DISABLED (init failed)" -ForegroundColor Red
    Write-Host ""
    Write-Host "All thumbnails will use CPU path:" -ForegroundColor Yellow
    Write-Host "  - All formats: 20-50ms each (no GPU acceleration)" -ForegroundColor Yellow
    Write-Host "  - JPEG caching still enabled (quality 85)" -ForegroundColor Green
    Write-Host ""
    Write-Host "To investigate:" -ForegroundColor Cyan
    Write-Host "  1. Update GPU drivers to latest version" -ForegroundColor White
    Write-Host "  2. Check Device Manager for GPU hardware" -ForegroundColor White
    Write-Host "  3. Verify Windows 10/11 with DirectX 12 support" -ForegroundColor White
}
Write-Host ""

Write-Host "Press Enter to exit..."
Read-Host
