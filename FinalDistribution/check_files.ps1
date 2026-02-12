# Check critical files
Write-Host "=== File Check ===" -ForegroundColor Yellow

$files = @(
    "DocBrakeGUI.exe",
    "DocBrakeGUI.dll", 
    "bpg_viewer.dll",
    "openarc_ffi.dll"
)

foreach ($file in $files) {
    if (Test-Path $file) {
        $item = Get-Item $file
        Write-Host "✅ $file exists - Size: $($item.Length) bytes, Modified: $($item.LastWriteTime)" -ForegroundColor Green
    } else {
        Write-Host "❌ $file MISSING" -ForegroundColor Red
    }
}
