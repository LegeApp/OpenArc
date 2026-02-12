# Monitor debug.log for GPU activity
Write-Host "=== Debug Log Monitor ===" -ForegroundColor Yellow

$logPath = "debug.log"

# Clear existing log for fresh test
if (Test-Path $logPath) {
    Remove-Item $logPath -Force
    Write-Host "Cleared existing debug.log" -ForegroundColor Gray
}

Write-Host "Starting DocBrakeGUI..." -ForegroundColor Cyan
Write-Host "Monitoring debug.log for GPU activity..." -ForegroundColor White
Write-Host ""

# Start the process
$process = Start-Process -FilePath "DocBrakeGUI.exe" -PassThru -WindowStyle Normal

Write-Host "Process started with PID: $($process.Id)" -ForegroundColor Green
Write-Host "Press Ctrl+C to stop monitoring" -ForegroundColor Gray
Write-Host ""

# Monitor the log file
$lastSize = 0
try {
    while (!$process.HasExited) {
        if (Test-Path $logPath) {
            $currentSize = (Get-Item $logPath).Length
            if ($currentSize -ne $lastSize) {
                $newContent = Get-Content $logPath -Tail 10
                $newContent | ForEach-Object {
                    if ($_ -match "GPU|gpu") {
                        Write-Host $_ -ForegroundColor Green
                    } elseif ($_ -match "NativeGpuService") {
                        Write-Host $_ -ForegroundColor Cyan
                    } elseif ($_ -match "ThumbnailCache") {
                        Write-Host $_ -ForegroundColor Yellow
                    } else {
                        Write-Host $_ -ForegroundColor Gray
                    }
                }
                $lastSize = $currentSize
            }
        }
        Start-Sleep -Milliseconds 500
    }
} catch {
    Write-Host "Monitoring stopped" -ForegroundColor Red
}

Write-Host "`nProcess exited. Final log content:" -ForegroundColor Yellow
if (Test-Path $logPath) {
    Get-Content $logPath | ForEach-Object {
        if ($_ -match "GPU|gpu") {
            Write-Host $_ -ForegroundColor Green
        } elseif ($_ -match "NativeGpuService") {
            Write-Host $_ -ForegroundColor Cyan
        } elseif ($_ -match "ThumbnailCache") {
            Write-Host $_ -ForegroundColor Yellow
        } else {
            Write-Host $_ -ForegroundColor Gray
        }
    }
} else {
    Write-Host "No debug.log file found" -ForegroundColor Red
}
