# Simple test to check if debug.log is created
Write-Host "=== Simple Debug Log Test ===" -ForegroundColor Yellow

# Clear any existing logs
Remove-Item "debug.log" -Force -ErrorAction SilentlyContinue
Remove-Item "app_constructor.log" -Force -ErrorAction SilentlyContinue

Write-Host "Cleared existing logs" -ForegroundColor Gray
Write-Host "Starting DocBrakeGUI..." -ForegroundColor Cyan

# Start the process
$process = Start-Process -FilePath "DocBrakeGUI.exe" -PassThru -WindowStyle Normal

Write-Host "Process started with PID: $($process.Id)" -ForegroundColor Green

# Wait a moment for startup
Start-Sleep -Seconds 5

# Check for logs
Write-Host "`n=== Checking for log files ===" -ForegroundColor Yellow

if (Test-Path "debug.log") {
    Write-Host "✅ debug.log found!" -ForegroundColor Green
    Write-Host "Size: $((Get-Item 'debug.log').Length) bytes" -ForegroundColor White
    Write-Host "Content:" -ForegroundColor Gray
    Get-Content "debug.log" | ForEach-Object { Write-Host "  $_" -ForegroundColor White }
} else {
    Write-Host "❌ debug.log NOT found" -ForegroundColor Red
}

if (Test-Path "app_constructor.log") {
    Write-Host "`n✅ app_constructor.log found!" -ForegroundColor Green
    Write-Host "Content:" -ForegroundColor Gray
    Get-Content "app_constructor.log" | ForEach-Object { Write-Host "  $_" -ForegroundColor White }
} else {
    Write-Host "❌ app_constructor.log NOT found" -ForegroundColor Red
}

if (Test-Path "startup.log") {
    Write-Host "`n✅ startup.log found!" -ForegroundColor Green
    $gpuLines = Get-Content "startup.log" | Where-Object { $_ -match "GPU|gpu" }
    if ($gpuLines) {
        Write-Host "GPU-related entries:" -ForegroundColor Green
        $gpuLines | ForEach-Object { Write-Host "  $_" -ForegroundColor White }
    }
} else {
    Write-Host "❌ startup.log NOT found" -ForegroundColor Red
}

Write-Host "`nPress Enter to exit..." -ForegroundColor Gray
Read-Host

# Clean up
if ($process -and !$process.HasExited) {
    $process.Kill()
}
