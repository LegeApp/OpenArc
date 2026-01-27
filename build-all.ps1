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

# Step 3: Build DocBrakeGUI
Write-Host ""
Write-Host "Step 3: Building DocBrakeGUI..." -ForegroundColor Green

Push-Location "$PSScriptRoot\DocBrakeGUI"
try {
    if ($Release) {
        dotnet publish DocBrakeGUI.csproj -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -o "$PSScriptRoot\Release"
    } else {
        dotnet publish DocBrakeGUI.csproj -c Debug -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -o "$PSScriptRoot\Release"
    }
    if ($LASTEXITCODE -ne 0) {
        throw "DocBrakeGUI build failed"
    }
} finally {
    Pop-Location
}

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
Write-Host "  - DocBrakeGUI (WPF GUI with MediaBrowser)"
Write-Host ""

if ($Release) {
    Write-Host "Executables (Release):" -ForegroundColor Yellow
    Write-Host "  - CLI: target\release\openarc.exe"
    Write-Host "  - FFI: target\release\openarc_ffi.dll"
    Write-Host "  - BPG Viewer: target\release\bpg_viewer.dll"
    Write-Host "  - GUI: DocBrakeGUI\bin\Release\net8.0-windows\DocBrakeGUI.exe"
} else {
    Write-Host "Executables (Debug):" -ForegroundColor Yellow
    Write-Host "  - CLI: target\debug\openarc.exe"
    Write-Host "  - FFI: target\debug\openarc_ffi.dll"
    Write-Host "  - BPG Viewer: target\debug\bpg_viewer.dll"
    Write-Host "  - GUI: DocBrakeGUI\bin\Debug\net8.0-windows\DocBrakeGUI.exe"
}
Write-Host ""

# Step 4: Stage to d:\misc\docbrake-stage
Write-Host "Step 4: Staging to d:\misc\docbrake-stage..." -ForegroundColor Green

$stagingDir = "d:\misc\docbrake-stage"
$rustTargetDir = if ($Release) { "$PSScriptRoot\target\x86_64-pc-windows-gnu\release" } else { "$PSScriptRoot\target\x86_64-pc-windows-gnu\debug" }

# Create staging directory
if (Test-Path $stagingDir) {
    Write-Host "  Cleaning existing staging directory..."
    Remove-Item -Path $stagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null

# Copy GUI binaries
Write-Host "  Copying GUI binaries..."
Copy-Item -Path "$PSScriptRoot\Release\*" -Destination $stagingDir -Recurse -Force

# Copy Rust DLLs (overwrite if GUI build copied old ones)
Write-Host "  Copying Rust DLLs..."
Copy-Item -Path "$rustTargetDir\bpg_viewer.dll" -Destination $stagingDir -Force
Copy-Item -Path "$rustTargetDir\openarc_ffi.dll" -Destination $stagingDir -Force

# Copy CLI executable
Write-Host "  Copying CLI executable..."
Copy-Item -Path "$PSScriptRoot\target\x86_64-pc-windows-gnu\debug\openarc.exe" -Destination $stagingDir -Force

# Optional: Copy PDB files for debugging
if (-not $Release) {
    if (Test-Path "$rustTargetDir\openarc_ffi.pdb") {
        Copy-Item -Path "$rustTargetDir\openarc_ffi.pdb" -Destination $stagingDir -Force
    }
    if (Test-Path "$rustTargetDir\bpg_viewer.pdb") {
        Copy-Item -Path "$rustTargetDir\bpg_viewer.pdb" -Destination $stagingDir -Force
    }
}

Write-Host ""
Write-Host "=== Staging Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "Staged to: $stagingDir" -ForegroundColor Cyan
Write-Host ""
Write-Host "To run the application:" -ForegroundColor Yellow
Write-Host "  cd $stagingDir"
Write-Host "  .\DocBrakeGUI.exe"
Write-Host ""

# Show file count
$fileCount = (Get-ChildItem -Path $stagingDir -File).Count
Write-Host "Total files staged: $fileCount" -ForegroundColor Cyan
Write-Host ""
