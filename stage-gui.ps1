# OpenArc GUI Staging Script
# Builds DocBrakeGUI and stages it with required DLLs to d:\misc\docbrake-stage

param(
    [switch]$Release
)

$ErrorActionPreference = "Stop"

Write-Host "=== OpenArc GUI Build & Stage ===" -ForegroundColor Cyan
Write-Host ""

$buildConfig = if ($Release) { "Release" } else { "Debug" }
$cargoProfile = if ($Release) { "release" } else { "debug" }

Write-Host "Build Configuration: $buildConfig" -ForegroundColor Yellow
Write-Host ""

# Step 1: Build Rust FFI and BPG Viewer DLLs
Write-Host "Step 1: Building Rust DLLs (openarc_ffi.dll, bpg_viewer.dll)..." -ForegroundColor Green

Push-Location "$PSScriptRoot"
try {
    if ($Release) {
        cargo build --release -p openarc-ffi -p bpg-viewer
    } else {
        cargo build -p openarc-ffi -p bpg-viewer
    }
    if ($LASTEXITCODE -ne 0) {
        throw "Rust DLL build failed"
    }
} finally {
    Pop-Location
}

# Step 2: Build DocBrakeGUI
Write-Host ""
Write-Host "Step 2: Building DocBrakeGUI..." -ForegroundColor Green

Push-Location "$PSScriptRoot\DocBrakeGUI"
try {
    dotnet build DocBrakeGUI.csproj -c $buildConfig
    if ($LASTEXITCODE -ne 0) {
        throw "DocBrakeGUI build failed"
    }
} finally {
    Pop-Location
}

# Step 3: Stage to d:\misc\docbrake-stage
Write-Host ""
Write-Host "Step 3: Staging to d:\misc\docbrake-stage..." -ForegroundColor Green

$stagingDir = "d:\misc\docbrake-stage"
$guiBinDir = "$PSScriptRoot\DocBrakeGUI\bin\$buildConfig\net8.0-windows"
$rustTargetDir = "$PSScriptRoot\target\$cargoProfile"

# Create staging directory
if (Test-Path $stagingDir) {
    Write-Host "  Cleaning existing staging directory..."
    Remove-Item -Path $stagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null

# Copy GUI binaries
Write-Host "  Copying GUI binaries..."
Copy-Item -Path "$guiBinDir\*" -Destination $stagingDir -Recurse -Force

# Copy Rust DLLs (overwrite if GUI build copied old ones)
Write-Host "  Copying Rust DLLs..."
Copy-Item -Path "$rustTargetDir\openarc_ffi.dll" -Destination $stagingDir -Force
Copy-Item -Path "$rustTargetDir\bpg_viewer.dll" -Destination $stagingDir -Force

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
