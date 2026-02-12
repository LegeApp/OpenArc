# Simple MTP test
$dllPath = "D:\misc\arc\openarc\Release\openarc_ffi.dll"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public class MtpTester {
    [DllImport(@"$dllPath", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr MtpListDevices();
    
    [DllImport(@"$dllPath", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr MtpListFolder(string deviceId, string objectId);
    
    [DllImport(@"$dllPath", CallingConvention = CallingConvention.Cdecl)]
    public static extern void MtpFreeString(IntPtr ptr);
}
"@

Write-Host "Testing MTP..." -ForegroundColor Cyan

# List devices
$ptr = [MtpTester]::MtpListDevices()
$json = [System.Runtime.InteropServices.Marshal]::PtrToStringAnsi($ptr)
[MtpTester]::MtpFreeString($ptr)

Write-Host "Devices JSON:" -ForegroundColor Green
Write-Host $json

$devices = $json | ConvertFrom-Json

if ($devices.success) {
    Write-Host "`nFound $($devices.data.Count) device(s)" -ForegroundColor Green
    foreach ($dev in $devices.data) {
        Write-Host "  Device: $($dev.friendly_name) (Type: $($dev.device_type))"
        Write-Host "  ID: $($dev.id)"
        
        # List root
        $ptr2 = [MtpTester]::MtpListFolder($dev.id, "")
        $json2 = [System.Runtime.InteropServices.Marshal]::PtrToStringAnsi($ptr2)
        [MtpTester]::MtpFreeString($ptr2)
        
        Write-Host "  Root listing:" -ForegroundColor Cyan
        Write-Host "  $json2"
        
        $root = $json2 | ConvertFrom-Json
        if ($root.success) {
            Write-Host "  Root has $($root.data.Count) items" -ForegroundColor Green
            $root.data | Select-Object -First 5 | ForEach-Object {
                $type = if ($_.is_folder) { "FOLDER" } else { "FILE" }
                Write-Host "    [$type] $($_.name)"
            }
        } else {
            Write-Host "  ERROR: $($root.error)" -ForegroundColor Red
        }
    }
} else {
    Write-Host "ERROR: $($devices.error)" -ForegroundColor Red
}
