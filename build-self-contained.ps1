# OpenArc - Quick Self-Contained Build
# Builds just the final executables ready for distribution
# Output: DocBrakeGUI.exe (~150MB) + openarc.exe (~30MB)

param(
    [switch]$SkipRust,
    [switch]$SkipGui
)

$ErrorActionPreference = "Stop"

Write-Host "=== OpenArc Self-Contained Build ===" -ForegroundColor Cyan
Write-Host ""

# Build Rust components (unless skipped)
if (-not $SkipRust) {
    Write-Host "Building Rust workspace (Release)..." -ForegroundColor Green
    Push-Location "$PSScriptRoot"
    try {
        cargo build --release --workspace --exclude codecs
        if ($LASTEXITCODE -ne 0) {
            throw "Rust build failed"
        }
    } finally {
        Pop-Location
    }
    Write-Host "  ✓ Rust components built" -ForegroundColor Green
} else {
    Write-Host "Skipping Rust build (using existing binaries)" -ForegroundColor Yellow
}

Write-Host ""

# Build GUI (unless skipped)
if (-not $SkipGui) {
    Write-Host "Building DocBrakeGUI (Self-Contained Single File)..." -ForegroundColor Green
    Write-Host "  This may take 2-3 minutes..." -ForegroundColor Yellow
    
    Push-Location "$PSScriptRoot\DocBrakeGUI"
    try {
        dotnet publish DocBrakeGUI.csproj -c Release -r win-x64 `
            -p:PublishSingleFile=true `
            -p:SelfContained=true `
            -p:IncludeNativeLibrariesForSelfExtract=true `
            -p:IncludeAllContentForSelfExtract=true `
            -p:EnableCompressionInSingleFile=true `
            -p:PublishTrimmed=false `
            -o "$PSScriptRoot\Release"
        if ($LASTEXITCODE -ne 0) {
            throw "DocBrakeGUI publish failed"
        }
    } finally {
        Pop-Location
    }
    Write-Host "  ✓ DocBrakeGUI.exe built" -ForegroundColor Green
} else {
    Write-Host "Skipping GUI build (using existing binary)" -ForegroundColor Yellow
}

Write-Host ""

# Copy CLI to Release folder
Write-Host "Finalizing Release folder..." -ForegroundColor Green
$cliSource = "$PSScriptRoot\target\release\openarc.exe"
if (Test-Path $cliSource) {
    Copy-Item -Path $cliSource -Destination "$PSScriptRoot\Release\openarc.exe" -Force
    Write-Host "  ✓ openarc.exe copied" -ForegroundColor Green
} else {
    Write-Host "  ⚠ Warning: openarc.exe not found - you may need to build Rust first" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== Build Complete ===" -ForegroundColor Green
Write-Host ""

# Show results
$releaseDir = "$PSScriptRoot\Release"
Write-Host "Final Distribution Files:" -ForegroundColor Cyan
Write-Host "Location: $releaseDir" -ForegroundColor Gray
Write-Host ""

if (Test-Path "$releaseDir\DocBrakeGUI.exe") {
    $guiSize = (Get-Item "$releaseDir\DocBrakeGUI.exe").Length / 1MB
    Write-Host "  📦 DocBrakeGUI.exe" -ForegroundColor Yellow
    Write-Host "     Size: $([math]::Round($guiSize, 1)) MB" -ForegroundColor Gray
    Write-Host "     Type: Self-contained (includes .NET 8 runtime + all native DLLs)" -ForegroundColor Gray
    Write-Host ""
}

if (Test-Path "$releaseDir\openarc.exe") {
    $cliSize = (Get-Item "$releaseDir\openarc.exe").Length / 1MB
    Write-Host "  📦 openarc.exe" -ForegroundColor Yellow
    Write-Host "     Size: $([math]::Round($cliSize, 1)) MB" -ForegroundColor Gray
    Write-Host "     Type: CLI tool for batch processing" -ForegroundColor Gray
    Write-Host ""
}

# Count total files in Release
$allFiles = Get-ChildItem -Path $releaseDir -File
$totalSize = ($allFiles | Measure-Object -Property Length -Sum).Sum / 1MB

Write-Host "Total Distribution:" -ForegroundColor Green
Write-Host "  Files: $($allFiles.Count)" -ForegroundColor Gray
Write-Host "  Size: $([math]::Round($totalSize, 1)) MB" -ForegroundColor Gray
Write-Host ""

# Check for unnecessary files
$unnecessaryFiles = $allFiles | Where-Object { 
    $_.Extension -in @('.pdb', '.xml') -or 
    ($_.Name -like '*.dll' -and $_.Name -ne 'openarc_ffi.dll' -and $_.Name -ne 'bpg_viewer.dll')
}

if ($unnecessaryFiles.Count -gt 0) {
    Write-Host "⚠ Found $($unnecessaryFiles.Count) extra files that may not be needed:" -ForegroundColor Yellow
    $unnecessaryFiles | ForEach-Object { Write-Host "    - $($_.Name)" -ForegroundColor Gray }
    Write-Host ""
    
    $cleanup = Read-Host "Remove these files? (y/n)"
    if ($cleanup -eq 'y') {
        $unnecessaryFiles | Remove-Item -Force
        Write-Host "  ✓ Cleaned up extra files" -ForegroundColor Green
        Write-Host ""
    }
}

Write-Host "Ready to distribute!" -ForegroundColor Green
Write-Host "Just copy these 2 executables to deploy:" -ForegroundColor Cyan
Write-Host "  • DocBrakeGUI.exe (complete GUI application)" -ForegroundColor White
Write-Host "  • openarc.exe (command-line tool)" -ForegroundColor White
Write-Host ""
