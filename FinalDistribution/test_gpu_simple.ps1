# Simple GPU detection test
Add-Type -Path "DocBrakeGUI.dll"

$gpuService = [DocBrake.Services.NativeGpuService]::Instance

Write-Host "=== GPU Detection Results ==="
Write-Host "GPU Available: $($gpuService.HasGpu)"
Write-Host "CUDA Available: $($gpuService.HasCuda)"
Write-Host "OpenCL Available: $($gpuService.HasOpenCL)"
Write-Host "DirectML Available: $($gpuService.HasDirectML)"
Write-Host "Backend: $($gpuService.ActiveBackendName)"
Write-Host "Device: $($gpuService.DeviceName)"
