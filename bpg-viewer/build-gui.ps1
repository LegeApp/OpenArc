# Build script for BPG Viewer GUI
# Builds both Rust library and C# WPF application

param(
    [switch]$Clean,
    [switch]$Release = $true
)

$ErrorActionPreference = "Stop"

Write-Host "BPG Viewer GUI Build Script" -ForegroundColor Cyan
Write-Host "============================`n" -ForegroundColor Cyan

# Clean if requested
if ($Clean) {
    Write-Host "Cleaning build artifacts..." -ForegroundColor Yellow
    cargo clean
    if (Test-Path "BpgViewerGUI\bin") {
        Remove-Item "BpgViewerGUI\bin" -Recurse -Force
    }
    if (Test-Path "BpgViewerGUI\obj") {
        Remove-Item "BpgViewerGUI\obj" -Recurse -Force
    }
    Write-Host "Clean complete.`n" -ForegroundColor Green
}

# Step 1: Build Rust library (cdylib for Windows DLL)
Write-Host "[1/2] Building Rust library (bpg_viewer.dll)..." -ForegroundColor Cyan

$buildType = if ($Release) { "release" } else { "debug" }
$buildFlag = if ($Release) { "--release" } else { "" }

$cargoCmd = "cargo build $buildFlag --lib"
Write-Host "  Running: $cargoCmd" -ForegroundColor Gray
Invoke-Expression $cargoCmd

if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Rust build failed!" -ForegroundColor Red
    exit 1
}

# Check if DLL was created
$dllPath = "target\$buildType\bpg_viewer.dll"
if (!(Test-Path $dllPath)) {
    Write-Host "ERROR: DLL not found at $dllPath" -ForegroundColor Red
    Write-Host "Make sure Cargo.toml has crate-type = ['cdylib']" -ForegroundColor Yellow
    exit 1
}

Write-Host "  ✓ Rust library built: $dllPath`n" -ForegroundColor Green

# Step 2: Build C# GUI
Write-Host "[2/2] Building C# WPF GUI..." -ForegroundColor Cyan

$dotnetConfig = if ($Release) { "Release" } else { "Debug" }
$dotnetCmd = "dotnet build BpgViewerGUI\BpgViewerGUI.csproj -c $dotnetConfig"

Write-Host "  Running: $dotnetCmd" -ForegroundColor Gray
Invoke-Expression $dotnetCmd

if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: C# build failed!" -ForegroundColor Red
    exit 1
}

Write-Host "  ✓ C# GUI built successfully`n" -ForegroundColor Green

# Find the output EXE
$exePath = "BpgViewerGUI\bin\$dotnetConfig\net8.0-windows\BpgViewerGUI.exe"

if (Test-Path $exePath) {
    Write-Host "============================`n" -ForegroundColor Cyan
    Write-Host "Build Complete!" -ForegroundColor Green
    Write-Host "`nExecutable: $exePath" -ForegroundColor White
    Write-Host "DLL: $dllPath`n" -ForegroundColor White

    # Check if DLL was copied
    $copiedDll = "BpgViewerGUI\bin\$dotnetConfig\net8.0-windows\bpg_viewer.dll"
    if (Test-Path $copiedDll) {
        Write-Host "[OK] Native DLL copied to output directory" -ForegroundColor Green
    } else {
        Write-Host "[WARNING] Native DLL not found in output directory" -ForegroundColor Yellow
        Write-Host "  Manually copy: $dllPath" -ForegroundColor Yellow
        Write-Host "  To: BpgViewerGUI\bin\$dotnetConfig\net8.0-windows" -ForegroundColor Yellow
    }

    Write-Host "`nTo run:" -ForegroundColor Cyan
    Write-Host "  .\$exePath" -ForegroundColor White
    Write-Host "  .\$exePath image.bpg`n" -ForegroundColor White

} else {
    Write-Host "ERROR: Executable not found at expected path!" -ForegroundColor Red
    Write-Host "Expected: $exePath" -ForegroundColor Yellow
    exit 1
}
