#!/usr/bin/env pwsh
# DLL Function Export Checker - Verifies GPU functions are present

Write-Host "=== BPG Viewer DLL Export Checker ===" -ForegroundColor Cyan
Write-Host ""

$dllPath = ".\bpg_viewer.dll"

if (!(Test-Path $dllPath)) {
    Write-Host "ERROR: bpg_viewer.dll not found!" -ForegroundColor Red
    exit 1
}

Write-Host "[Checking] $dllPath" -ForegroundColor Yellow
Write-Host ""

# Use dumpbin if available, otherwise use a .NET reflection approach
try {
    # Try using dotnet/PEReader to list exports
    Add-Type -AssemblyName System.Reflection.Metadata
    
    $dllFullPath = (Resolve-Path $dllPath).Path
    $bytes = [System.IO.File]::ReadAllBytes($dllFullPath)
    
    Write-Host "GPU Functions to verify:" -ForegroundColor Cyan
    Write-Host "  - gpu_thumbnail_pipeline_init" -ForegroundColor Yellow
    Write-Host "  - gpu_thumbnail_process_jpeg" -ForegroundColor Yellow
    Write-Host "  - gpu_thumbnail_readback_jpeg" -ForegroundColor Yellow
    Write-Host ""
    
    # Try to load the DLL and get function addresses
    $lib = [System.Runtime.InteropServices.NativeLibrary]::Load($dllFullPath)
    
    $functions = @(
        "gpu_thumbnail_pipeline_init",
        "gpu_thumbnail_process_jpeg",
        "gpu_thumbnail_readback_jpeg",
        "universal_thumbnail_generate_jpeg"
    )
    
    $exportedCount = 0
    foreach ($func in $functions) {
        try {
            $ptr = [System.Runtime.InteropServices.NativeLibrary]::GetExport($lib, $func)
            if ($ptr -ne [IntPtr]::Zero) {
                Write-Host "[OK] $func" -ForegroundColor Green
                $exportedCount++
            } else {
                Write-Host "[MISSING] $func" -ForegroundColor Red
            }
        } catch {
            Write-Host "[MISSING] $func" -ForegroundColor Red
        }
    }
    
    [System.Runtime.InteropServices.NativeLibrary]::Free($lib)
    
    Write-Host ""
    if ($exportedCount -eq $functions.Count) {
        Write-Host "[SUCCESS] All GPU functions exported correctly!" -ForegroundColor Green
    } else {
        Write-Host "[ERROR] Missing $($functions.Count - $exportedCount) functions" -ForegroundColor Red
        Write-Host "GPU thumbnailing will NOT work!" -ForegroundColor Red
        Write-Host ""
        Write-Host "Solution: Rebuild bpg_viewer.dll" -ForegroundColor Yellow
        Write-Host "  cd D:\misc\arc\openarc\bpg-viewer" -ForegroundColor Gray
        Write-Host "  cargo build --release" -ForegroundColor Gray
        Write-Host "  Copy-Item ..\target\x86_64-pc-windows-gnu\release\bpg_viewer.dll ..\FinalDistribution\ -Force" -ForegroundColor Gray
    }
    
} catch {
    Write-Host "Error checking DLL exports: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "Alternative: Use 'dumpbin /EXPORTS bpg_viewer.dll' (requires Visual Studio)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Press Enter to exit..."
Read-Host
