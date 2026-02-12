# Build and deploy thumbnail DLLs for OpenArc
# This ensures both the main bpg-viewer DLL and the GPU thumbnail DLL are built and deployed

param(
    [string]$Configuration = "Release",
    [switch]$SkipGpu = $false
)

$ErrorActionPreference = "Stop"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "Building OpenArc Thumbnail Libraries" -ForegroundColor Cyan
Write-Host "Configuration: $Configuration" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""

# Determine build profile
$profile = if ($Configuration -eq "Debug") { "dev" } else { "release" }
$targetDir = if ($Configuration -eq "Debug") { "target\debug" } else { "target\release" }

# Find C# output directories
$csharpOutputDirs = @(
    "DocBrakeGUI\bin\$Configuration\net8.0-windows10.0.26100.0",
    "DocBrakeGUI\bin\$Configuration\net8.0-windows"
)

# Build GPU thumbnail DLL (optional - has special requirements)
if (-not $SkipGpu) {
    Write-Host "Building GPU thumbnail DLL (gpu-thumbnail-dll)..." -ForegroundColor Yellow
    cargo build --profile $profile --package gpu-thumbnail-dll

    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ GPU thumbnail DLL built successfully" -ForegroundColor Green

        $gpuDll = "$targetDir\openarc_gpu_thumbnails.dll"
        if (Test-Path $gpuDll) {
            Write-Host "  Deploying GPU DLL..." -ForegroundColor Gray

            foreach ($outDir in $csharpOutputDirs) {
                if (Test-Path $outDir) {
                    Copy-Item $gpuDll $outDir -Force -ErrorAction SilentlyContinue
                    if ($?) {
                        Write-Host "  → Copied to $outDir" -ForegroundColor Gray
                    }
                }
            }
        } else {
            Write-Host "  WARNING: GPU DLL not found at $gpuDll" -ForegroundColor Yellow
        }
    } else {
        Write-Host "✗ GPU thumbnail DLL build FAILED" -ForegroundColor Red
        Write-Host "  This is OK - system will fall back to CPU thumbnailing" -ForegroundColor Yellow
        Write-Host "  Common reasons:" -ForegroundColor Yellow
        Write-Host "    - Windows 10 1809 or newer required for DirectX 12" -ForegroundColor Yellow
        Write-Host "    - GPU drivers need to support D3D12" -ForegroundColor Yellow
        Write-Host "    - Some build dependencies may be missing" -ForegroundColor Yellow
        Write-Host ""
    }
} else {
    Write-Host "Skipping GPU DLL build (--SkipGpu specified)" -ForegroundColor Gray
}

Write-Host ""

# Build main bpg-viewer DLL (required)
Write-Host "Building main thumbnail DLL (bpg-viewer)..." -ForegroundColor Yellow
cargo build --profile $profile --package bpg-viewer

if ($LASTEXITCODE -ne 0) {
    Write-Host "✗ Main thumbnail DLL build FAILED" -ForegroundColor Red
    exit 1
}

Write-Host "✓ Main thumbnail DLL built successfully" -ForegroundColor Green

$mainDll = "$targetDir\bpg_viewer.dll"
if (Test-Path $mainDll) {
    Write-Host "  Deploying main DLL..." -ForegroundColor Gray

    foreach ($outDir in $csharpOutputDirs) {
        if (Test-Path $outDir) {
            Copy-Item $mainDll $outDir -Force -ErrorAction SilentlyContinue
            if ($?) {
                Write-Host "  → Copied to $outDir" -ForegroundColor Gray
            }
        } else {
            Write-Host "  → Directory not found: $outDir (skipping)" -ForegroundColor DarkGray
        }
    }
} else {
    Write-Host "  ERROR: Main DLL not found at $mainDll" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "Build complete!" -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Rebuild C# solution in Visual Studio"
Write-Host "  2. Run the application"
Write-Host "  3. Check console for GPU init status"
Write-Host ""
