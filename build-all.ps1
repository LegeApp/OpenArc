# OpenArc Complete Build Script
# Builds all components: codecs, Rust workspace, and DocBrakeGUI

param(
    [switch]$Release
)

$ErrorActionPreference = "Stop"

Write-Host "=== OpenArc Complete Build ===" -ForegroundColor Cyan
Write-Host ""

$buildConfig = if ($Release) { "Release" } else { "Debug" }

Write-Host "Build Configuration: $buildConfig" -ForegroundColor Yellow
Write-Host ""

# Step 1: Build codecs
Write-Host "Step 1: Building codec dependencies..." -ForegroundColor Green
Push-Location "$PSScriptRoot\BPG\libbpg-0.9.8"
try {
    & "$PSScriptRoot\BPG\libbpg-0.9.8\build_complete.bat"
    if ($LASTEXITCODE -ne 0) {
        throw "BPG codec build failed"
    }
} finally {
    Pop-Location
}

# Build FreeArc codecs if needed
$freeArcCodecDir = "$PSScriptRoot\arcmax"
$codecMakefile = Join-Path $freeArcCodecDir "codec_staging\Makefile"
if (Test-Path $codecMakefile) {
    Push-Location (Split-Path $codecMakefile -Parent)
    try {
        Write-Host "Building FreeArc codecs via Makefile..." -ForegroundColor Green
        make -j4
        if ($LASTEXITCODE -ne 0) {
            throw "FreeArc codec Makefile build failed"
        }
    } finally {
        Pop-Location
    }
} else {
    $buildCodecsScript = Join-Path $freeArcCodecDir "build_codecs.ps1"
    if (Test-Path $buildCodecsScript) {
        Write-Host "Running build_codecs.ps1 to build FreeArc codecs..." -ForegroundColor Green
        & powershell -NoProfile -ExecutionPolicy Bypass -File $buildCodecsScript
        if ($LASTEXITCODE -ne 0) {
            throw "FreeArc codec build script failed"
        }
    } else {
        throw "FreeArc codec build tooling not found (Makefile or build_codecs.ps1)."
    }
}

# Step 2: Build Rust workspace (CLI, Core, ArcMax, ZSTD, FFI, BPG-Viewer)
Write-Host ""
Write-Host "Step 2: Building Rust workspace components..." -ForegroundColor Green
Write-Host "  - openarc-core (Core library)"
Write-Host "  - openarc-ffi (FFI library)"
Write-Host "  - bpg-viewer (BPG processing library)"
Write-Host "  - arcmax (Compression library)"
Write-Host "  - zstd-archive (ZSTD wrapper)"
Write-Host "  - openarc (CLI)"

Push-Location "$PSScriptRoot"
try {
    if ($Release) {
        cargo build --release --workspace --exclude codecs
    } else {
        cargo build --workspace --exclude codecs
    }
    if ($LASTEXITCODE -ne 0) {
        throw "Rust workspace build failed"
    }
} finally {
    Pop-Location
}

# Step 3: Build DocBrakeGUI (Self-Contained Single File)
Write-Host ""
Write-Host "Step 3: Building DocBrakeGUI (Self-Contained)..." -ForegroundColor Green
Write-Host "  This will create a single ~150MB executable with all dependencies embedded" -ForegroundColor Yellow

Push-Location "$PSScriptRoot\DocBrakeGUI"
try {
    # Use dotnet publish for self-contained single-file output
    dotnet publish DocBrakeGUI.csproj -c Release -o "$PSScriptRoot\Release"
    if ($LASTEXITCODE -ne 0) {
        throw "DocBrakeGUI publish failed"
    }
} finally {
    Pop-Location
}

# Copy the CLI executable to Release folder
Write-Host ""
Write-Host "Step 4: Copying CLI executable..." -ForegroundColor Green
$cliSource = if ($Release) { "$PSScriptRoot\target\release\openarc.exe" } else { "$PSScriptRoot\target\debug\openarc.exe" }
if (Test-Path $cliSource) {
    Copy-Item -Path $cliSource -Destination "$PSScriptRoot\Release\openarc.exe" -Force
    Write-Host "  Copied: openarc.exe (~30MB)" -ForegroundColor Cyan
} else {
    Write-Host "  Warning: openarc.exe not found at $cliSource" -ForegroundColor Yellow
}

Write-Host ""
Write-Host ""
Write-Host "=== Build Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "Built components:" -ForegroundColor Cyan
Write-Host "  - openarc (CLI)"
Write-Host "  - openarc-core (Core library)"
Write-Host "  - arcmax (Compression library)"
Write-Host "  - zstd-archive (ZSTD wrapper)"
Write-Host "  - openarc-ffi (FFI library)"
Write-Host "  - bpg-viewer (BPG processing library)"
Write-Host "  - DocBrakeGUI (Self-Contained WPF GUI)"
Write-Host ""

Write-Host "Final executables in Release folder:" -ForegroundColor Yellow
$releaseDir = "$PSScriptRoot\Release"
if (Test-Path "$releaseDir\DocBrakeGUI.exe") {
    $guiSize = (Get-Item "$releaseDir\DocBrakeGUI.exe").Length / 1MB
    Write-Host "  - DocBrakeGUI.exe: $([math]::Round($guiSize, 1)) MB (self-contained, includes all .NET runtime + native DLLs)"
}
if (Test-Path "$releaseDir\openarc.exe") {
    $cliSize = (Get-Item "$releaseDir\openarc.exe").Length / 1MB
    Write-Host "  - openarc.exe: $([math]::Round($cliSize, 1)) MB (CLI tool)"
}
Write-Host ""
Write-Host "Total deployment: Just these 2 executables!" -ForegroundColor Green
Write-Host "  Location: $releaseDir" -ForegroundColor Cyan
Write-Host ""

# Remove staging section since we now have self-contained executables
