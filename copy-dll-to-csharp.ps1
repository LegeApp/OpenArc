# Copy bpg_viewer.dll to C# output directories after building
# Run this after building the Rust library

$ErrorActionPreference = "Stop"

Write-Host "Copying bpg_viewer.dll to C# output directories..." -ForegroundColor Cyan

# Source DLL
$sourceDll = "target\release\bpg_viewer.dll"

if (-not (Test-Path $sourceDll)) {
    Write-Host "ERROR: $sourceDll not found!" -ForegroundColor Red
    Write-Host "Please run: cargo build --release --package bpg-viewer" -ForegroundColor Yellow
    exit 1
}

Write-Host "Source DLL: $sourceDll" -ForegroundColor Green
Write-Host "DLL Size: $((Get-Item $sourceDll).Length / 1MB) MB" -ForegroundColor Gray

# Find all C# bin output directories
$binDirs = Get-ChildItem -Path "DocBrakeGUI" -Filter "bin" -Directory -Recurse -ErrorAction SilentlyContinue

if ($binDirs.Count -eq 0) {
    Write-Host "WARNING: No C# bin directories found!" -ForegroundColor Yellow
    Write-Host "This is normal if C# project hasn't been built yet." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Cyan
    Write-Host "  1. Open solution in Visual Studio" -ForegroundColor White
    Write-Host "  2. Build the solution (Ctrl+Shift+B)" -ForegroundColor White
    Write-Host "  3. Run this script again to copy the DLL" -ForegroundColor White
    Write-Host ""
    Write-Host "Or place the DLL manually in:" -ForegroundColor Cyan
    Write-Host "  DocBrakeGUI\bin\Debug\net8.0-windows10.0.26100.0\" -ForegroundColor White
    Write-Host "  DocBrakeGUI\bin\Release\net8.0-windows10.0.26100.0\" -ForegroundColor White
    exit 0
}

$copied = 0
foreach ($binDir in $binDirs) {
    # Find net8.0-windows* directories
    $netDirs = Get-ChildItem -Path $binDir.FullName -Filter "net8.0*" -Directory -ErrorAction SilentlyContinue

    foreach ($netDir in $netDirs) {
        $destPath = Join-Path $netDir.FullName "bpg_viewer.dll"

        try {
            Copy-Item $sourceDll $destPath -Force
            Write-Host "  ✓ Copied to: $destPath" -ForegroundColor Green
            $copied++
        } catch {
            Write-Host "  ✗ Failed to copy to: $destPath" -ForegroundColor Red
            Write-Host "    Error: $_" -ForegroundColor Red
        }
    }
}

if ($copied -eq 0) {
    Write-Host ""
    Write-Host "No DLL copies were made. C# bin directories exist but no net8.0* subdirectories found." -ForegroundColor Yellow
    Write-Host "Build the C# project first, then run this script again." -ForegroundColor Yellow
} else {
    Write-Host ""
    Write-Host "Successfully copied DLL to $copied location(s)!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next step: Run the application and check for the new function." -ForegroundColor Cyan
}
