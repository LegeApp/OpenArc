# Test GPU initialization at startup
Write-Host "=== Testing GPU Startup Initialization ===" -ForegroundColor Yellow

# Clear log file for clean test
$logPath = "app_constructor.log"
if (Test-Path $logPath) {
    Remove-Item $logPath -Force
}

Write-Host "Starting DocBrakeGUI and monitoring startup..." -ForegroundColor Cyan
Write-Host "Watch for GPU initialization messages immediately after startup" -ForegroundColor White

# Start the process
$process = Start-Process -FilePath "DocBrakeGUI.exe" -PassThru -WindowStyle Normal

Write-Host "Process started with PID: $($process.Id)" -ForegroundColor Green

# Wait a moment for startup, then check the log
Start-Sleep -Seconds 3

if (Test-Path $logPath) {
    Write-Host "`n=== Startup Log Content ===" -ForegroundColor Yellow
    $logContent = Get-Content $logPath
    $gpuLines = $logContent | Where-Object { $_ -match "GPU|gpu" }
    
    if ($gpuLines) {
        Write-Host "GPU-related log entries found:" -ForegroundColor Green
        $gpuLines | ForEach-Object { Write-Host "  $_" -ForegroundColor White }
    } else {
        Write-Host "No GPU-related entries found in startup log" -ForegroundColor Red
    }
    
    Write-Host "`n=== Full Startup Log ===" -ForegroundColor Gray
    $logContent | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
} else {
    Write-Host "No startup log file found" -ForegroundColor Red
}

Write-Host "`nPress Enter to exit..." -ForegroundColor Gray
Read-Host

# Clean up
if ($process -and !$process.HasExited) {
    $process.Kill()
}
