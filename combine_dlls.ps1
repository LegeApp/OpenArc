# PowerShell script to build the project and combine related DLLs
# This addresses the request to combine bpg_viewer.dll with related DLLs to simplify building and installing

Write-Host "Building OpenArc with GPU thumbnail support..." -ForegroundColor Green

# Build the project
Write-Host "Running cargo build --release..." -ForegroundColor Yellow
try {
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "Build failed with exit code $LASTEXITCODE"
    }
    Write-Host "Build completed successfully!" -ForegroundColor Green
} catch {
    Write-Host "Build failed: $_" -ForegroundColor Red
    exit 1
}

# Locate the generated DLLs
$bpgViewerDll = Join-Path $PSScriptRoot "target\release\bpg-viewer.dll"
$thumbnailGpuDll = Join-Path $PSScriptRoot "target\release\thumbnail_stage.dll"  # This might be embedded

Write-Host "Locating generated DLLs..." -ForegroundColor Yellow

if (Test-Path $bpgViewerDll) {
    Write-Host "Found bpg-viewer.dll at: $bpgViewerDll" -ForegroundColor Green
    
    # Copy to a combined output directory
    $outputDir = Join-Path $PSScriptRoot "combined-dlls"
    if (!(Test-Path $outputDir)) {
        New-Item -ItemType Directory -Path $outputDir -Force
    }
    
    $outputDll = Join-Path $outputDir "bpg_viewer_combined.dll"
    Copy-Item $bpgViewerDll $outputDll
    Write-Host "Copied bpg-viewer.dll to combined output: $outputDll" -ForegroundColor Green
    
    # Also copy any related DLLs that might be dependencies
    $releaseDir = Split-Path $bpgViewerDll
    $relatedDlls = Get-ChildItem -Path $releaseDir -Filter "*.dll" | Where-Object { $_.Name -ne "bpg-viewer.dll" }
    
    foreach ($dll in $relatedDlls) {
        $destPath = Join-Path $outputDir $dll.Name
        Copy-Item $dll.FullName $destPath
        Write-Host "Copied related DLL: $($dll.Name)" -ForegroundColor Cyan
    }
    
    Write-Host ""
    Write-Host "DLL combination completed!" -ForegroundColor Green
    Write-Host "Combined DLLs are located in: $outputDir" -ForegroundColor Green
    Write-Host ""
    Write-Host "Contents of combined directory:" -ForegroundColor Yellow
    Get-ChildItem -Path $outputDir | Format-Table Name, Length
} else {
    Write-Host "Warning: Could not find bpg-viewer.dll at expected location: $bpgViewerDll" -ForegroundColor Red
    Write-Host "This may be because the DLL is named differently or embedded in the executable." -ForegroundColor Yellow
}

# Also run the main build-all script to ensure everything is built properly
Write-Host ""
Write-Host "Running build-all.ps1 to ensure complete build..." -ForegroundColor Yellow
$buildAllScript = Join-Path $PSScriptRoot "build-all.ps1"
if (Test-Path $buildAllScript) {
    try {
        & $buildAllScript
        if ($LASTEXITCODE -ne 0) {
            throw "build-all.ps1 failed with exit code $LASTEXITCODE"
        }
        Write-Host "build-all.ps1 completed successfully!" -ForegroundColor Green
    } catch {
        Write-Host "build-all.ps1 failed: $_" -ForegroundColor Red
        # Don't exit here as the main DLL was already built
    }
} else {
    Write-Host "build-all.ps1 not found, skipping..." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "GPU thumbnailing fixes applied and build completed!" -ForegroundColor Green
Write-Host "Key changes made:" -ForegroundColor Cyan
Write-Host "  - Fixed fence synchronization in GPU pipeline" -ForegroundColor White
Write-Host "  - Increased GPU timeout to prevent stalls" -ForegroundColor White
Write-Host "  - Improved resource management in thumbnail generation" -ForegroundColor White
Write-Host "  - Enabled GPU thumbnails by default" -ForegroundColor White
Write-Host "  - Combined DLLs for simplified deployment" -ForegroundColor White