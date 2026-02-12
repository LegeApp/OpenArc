# Verbose GPU test with enhanced logging
Write-Host "=== Starting GPU Test ===" -ForegroundColor Yellow

# Clear any existing console output
Clear-Host

# Start the process and capture output
$process = Start-Process -FilePath "DocBrakeGUI.exe" -PassThru -WindowStyle Normal

Write-Host "Process started with PID: $($process.Id)" -ForegroundColor Green
Write-Host "Watch for these messages:" -ForegroundColor Cyan
Write-Host "  - [NativeGpuService] GPU Library Load: SUCCESS/FAILED" -ForegroundColor White
Write-Host "  - GPU THUMBNAIL INITIALIZATION:" -ForegroundColor White
Write-Host "  - 🚀 GPU PROCESSING:" -ForegroundColor Green
Write-Host "  - ✅ GPU SUCCESS:" -ForegroundColor Green
Write-Host "  - 🔄 GPU FALLBACK:" -ForegroundColor Yellow
Write-Host "  - ⚠️ GPU UNAVAILABLE:" -ForegroundColor Red
Write-Host ""
Write-Host "Press Ctrl+C to stop monitoring" -ForegroundColor Gray

# Wait for the process
$process.WaitForExit()

Write-Host "Process exited with code: $($process.ExitCode)" -ForegroundColor Yellow
