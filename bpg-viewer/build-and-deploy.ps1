# BPG Viewer Build and Deploy Script
# Builds the Rust library and C# GUI, then copies to test folder

param(
    [switch]$Release = $true,
    [string]$TestFolder = "D:\misc\bpg-viewer-test"
)

$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "BPG Viewer Build and Deploy" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Build Rust library
Write-Host "[1/3] Building Rust library..." -ForegroundColor Yellow
Set-Location $RootDir

if ($Release) {
    Write-Host "  Building in RELEASE mode..." -ForegroundColor Gray
    cargo build --release --lib
    $BuildMode = "release"
} else {
    Write-Host "  Building in DEBUG mode..." -ForegroundColor Gray
    cargo build --lib
    $BuildMode = "debug"
}

if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Rust build failed!" -ForegroundColor Red
    exit 1
}

$RustDllPath = Join-Path $RootDir "target\$BuildMode\bpg_viewer.dll"
if (-not (Test-Path $RustDllPath)) {
    Write-Host "ERROR: bpg_viewer.dll not found at $RustDllPath" -ForegroundColor Red
    exit 1
}

Write-Host "  ✓ Rust library built successfully" -ForegroundColor Green
Write-Host ""

# Step 2: Build C# GUI
Write-Host "[2/3] Building C# GUI..." -ForegroundColor Yellow
$CsprojPath = Join-Path $RootDir "BpgViewerGUI\BpgViewerGUI.csproj"

if ($Release) {
    Write-Host "  Building in RELEASE mode..." -ForegroundColor Gray
    dotnet build $CsprojPath -c Release
    $CsharpBuildMode = "Release"
} else {
    Write-Host "  Building in DEBUG mode..." -ForegroundColor Gray
    dotnet build $CsprojPath -c Debug
    $CsharpBuildMode = "Debug"
}

if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: C# build failed!" -ForegroundColor Red
    exit 1
}

Write-Host "  ✓ C# GUI built successfully" -ForegroundColor Green
Write-Host ""

# Step 3: Copy to test folder
Write-Host "[3/3] Deploying to test folder..." -ForegroundColor Yellow

# Create test folder if it doesn't exist
if (-not (Test-Path $TestFolder)) {
    Write-Host "  Creating test folder: $TestFolder" -ForegroundColor Gray
    New-Item -ItemType Directory -Path $TestFolder -Force | Out-Null
}

# Find the output directory
$OutputDir = Join-Path $RootDir "BpgViewerGUI\bin\$CsharpBuildMode\net8.0-windows"

if (-not (Test-Path $OutputDir)) {
    Write-Host "ERROR: Output directory not found: $OutputDir" -ForegroundColor Red
    exit 1
}

Write-Host "  Copying files from: $OutputDir" -ForegroundColor Gray
Write-Host "  Copying files to: $TestFolder" -ForegroundColor Gray

# Copy all files from output directory
Copy-Item "$OutputDir\*" -Destination $TestFolder -Recurse -Force

# Copy documentation
$DocsPath = Join-Path $RootDir "LESSONS_LEARNED_IMAGE_RENDERING.md"
if (Test-Path $DocsPath) {
    Copy-Item $DocsPath -Destination $TestFolder -Force
    Write-Host "  Copied documentation" -ForegroundColor Gray
}

Write-Host "  ✓ Files deployed successfully" -ForegroundColor Green
Write-Host ""

# Summary
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "Build Summary" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "Build Mode:      $BuildMode" -ForegroundColor White
Write-Host "Rust DLL:        $RustDllPath" -ForegroundColor White
Write-Host "C# Output:       $OutputDir" -ForegroundColor White
Write-Host "Test Folder:     $TestFolder" -ForegroundColor White
Write-Host ""
Write-Host "✓ Build and deployment complete!" -ForegroundColor Green
Write-Host ""
Write-Host "To run the application:" -ForegroundColor Yellow
Write-Host "  cd `"$TestFolder`"" -ForegroundColor Cyan
Write-Host "  .\BpgViewerGUI.exe" -ForegroundColor Cyan
Write-Host ""
