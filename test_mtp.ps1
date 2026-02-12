# Quick MTP test using the FFI directly
$ErrorActionPreference = "Stop"

$dllPath = "D:\misc\arc\openarc\Release\openarc_ffi.dll"
if (-not (Test-Path $dllPath)) {
    Write-Host "ERROR: DLL not found at $dllPath" -ForegroundColor Red
    exit 1
}

Write-Host "Using DLL: $dllPath" -ForegroundColor Cyan

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public class MtpTest {
    [DllImport(@"$dllPath", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr MtpListDevices();
    
    [DllImport(@"$dllPath", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr MtpListFolder(string deviceId, string objectId);
    
    [DllImport(@"$dllPath", CallingConvention = CallingConvention.Cdecl)]
    public static extern void MtpFreeString(IntPtr ptr);
}
"@

# Change to Release directory where DLL is
# Already changed above

try {
    Write-Host "Testing MTP Device Detection..." -ForegroundColor Cyan
    
    # List devices
    $devicesPtr = [MtpTest]::MtpListDevices()
    if ($devicesPtr -eq [IntPtr]::Zero) {
        Write-Host "ERROR: MtpListDevices returned null" -ForegroundColor Red
        exit 1
    }
    
    $devicesJson = [System.Runtime.InteropServices.Marshal]::PtrToStringAnsi($devicesPtr)
    [MtpTest]::MtpFreeString($devicesPtr)
    
    Write-Host "`nDevices Response:" -ForegroundColor Green
    Write-Host $devicesJson
    
    $devices = $devicesJson | ConvertFrom-Json
    
    if (-not $devices.success) {
        Write-Host "`nERROR: $($devices.error)" -ForegroundColor Red
        exit 1
    }
    
    if ($devices.data.Count -eq 0) {
        Write-Host "`nNo MTP devices detected" -ForegroundColor Yellow
        exit 0
    }
    
    Write-Host "`nFound $($devices.data.Count) MTP device(s):" -ForegroundColor Green
    foreach ($dev in $devices.data) {
        Write-Host "  - $($dev.friendly_name) (ID: $($dev.id), Type: $($dev.device_type))"
        
        # Try to list root folder
        Write-Host "    Listing root contents..." -ForegroundColor Cyan
        $folderPtr = [MtpTest]::MtpListFolder($dev.id, "")
        $folderJson = [System.Runtime.InteropServices.Marshal]::PtrToStringAnsi($folderPtr)
        [MtpTest]::MtpFreeString($folderPtr)
        
        Write-Host "    Response: $folderJson"
        
        $folder = $folderJson | ConvertFrom-Json
        if ($folder.success) {
            Write-Host "    Root contains $($folder.data.Count) objects:" -ForegroundColor Green
            foreach ($obj in $folder.data | Select-Object -First 10) {
                $icon = if ($obj.is_folder) { "📁" } else { "📄" }
                Write-Host "      $icon $($obj.name) (ID: $($obj.id))"
            }
            if ($folder.data.Count -gt 10) {
                Write-Host "      ... and $($folder.data.Count - 10) more"
            }
        } else {
            Write-Host "    ERROR: $($folder.error)" -ForegroundColor Red
        }
    }
    
    Write-Host "`nMTP Test Complete!" -ForegroundColor Green
    
} catch {
    Write-Host "`nERROR: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host $_.ScriptStackTrace
    exit 1
}
