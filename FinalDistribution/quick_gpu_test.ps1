#!/usr/bin/env pwsh
# Quick GPU Test - Just check if GPU pipeline can initialize

Write-Host "=== Quick GPU Test ===" -ForegroundColor Cyan
Write-Host ""

$dllPath = Join-Path $PSScriptRoot "bpg_viewer.dll"
if (!(Test-Path $dllPath)) {
    Write-Host "ERROR: bpg_viewer.dll not found!" -ForegroundColor Red
    exit 1
}

# Try to call GPU init function directly
try {
    Add-Type @"
using System;
using System.Runtime.InteropServices;
public class QuickGpuTest {
    [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern int gpu_thumbnail_pipeline_init();
}
"@

    [Environment]::CurrentDirectory = $PSScriptRoot
    $result = [QuickGpuTest]::gpu_thumbnail_pipeline_init()
    
    Write-Host "GPU pipeline init result: $result" -ForegroundColor Yellow
    
    if ($result -eq 0) {
        Write-Host "✓ SUCCESS: GPU thumbnailing is available!" -ForegroundColor Green
        Write-Host ""
        Write-Host "Expected performance:" -ForegroundColor Cyan
        Write-Host "  - JPEG thumbnails: 2-5ms each (GPU accelerated)" -ForegroundColor Green
        Write-Host "  - Other formats: 20-50ms each (CPU fallback)" -ForegroundColor Yellow
    } else {
        Write-Host "✗ FAILED: GPU thumbnailing not available" -ForegroundColor Red
        Write-Host ""
        Write-Host "All thumbnails will use CPU path (20-50ms each)" -ForegroundColor Yellow
        Write-Host ""
        Write-Host "Possible reasons:" -ForegroundColor Gray
        Write-Host "  - No D3D12-capable GPU" -ForegroundColor Gray
        Write-Host "  - Outdated GPU drivers" -ForegroundColor Gray
        Write-Host "  - Running in VM without GPU passthrough" -ForegroundColor Gray
    }
    
} catch {
    Write-Host "ERROR: Failed to test GPU: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "This suggests the DLL is corrupted or missing dependencies." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Press Enter to exit..."
Read-Host
