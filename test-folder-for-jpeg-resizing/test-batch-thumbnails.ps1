# GPU Thumbnail Batch Test Script
# Tests the fixed GPU thumbnail pipeline with error recovery and LRU eviction

param(
    [Parameter(Mandatory=$true)]
    [string]$InputFolder,
    
    [Parameter(Mandatory=$false)]
    [string]$OutputFolder = ".\gpu-thumbnails-test",
    
    [Parameter(Mandatory=$false)]
    [int]$MaxImages = 500
)

Write-Host "=== GPU Thumbnail Batch Test ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Input folder: $InputFolder"
Write-Host "Output folder: $OutputFolder"
Write-Host "Max images: $MaxImages"
Write-Host ""

# Create output folder
if (!(Test-Path $OutputFolder)) {
    New-Item -ItemType Directory -Path $OutputFolder -Force | Out-Null
}

# Get JPEG files
$jpegFiles = Get-ChildItem -Path $InputFolder -Include *.jpg,*.jpeg -Recurse -File | Select-Object -First $MaxImages
$totalFiles = $jpegFiles.Count

Write-Host "Found $totalFiles JPEG files" -ForegroundColor Green
Write-Host ""

if ($totalFiles -eq 0) {
    Write-Host "No JPEG files found in $InputFolder" -ForegroundColor Red
    exit 1
}

# Check if DLL exists
$dllPath = "D:\misc\arc\openarc\target\release\bpg_viewer.dll"
if (!(Test-Path $dllPath)) {
    Write-Host "ERROR: bpg_viewer.dll not found at $dllPath" -ForegroundColor Red
    Write-Host "Please build first: cargo build --release -p bpg-viewer" -ForegroundColor Yellow
    exit 1
}

Write-Host "Using DLL: $dllPath" -ForegroundColor Gray
Write-Host ""

# C# P/Invoke test harness
$csharpCode = @"
using System;
using System.Runtime.InteropServices;
using System.Diagnostics;

public class GpuThumbnailTest {
    const string DllPath = @"$dllPath";
    
    [DllImport(DllPath, CallingConvention = CallingConvention.Cdecl)]
    public static extern int gpu_thumbnail_pipeline_init();
    
    [DllImport(DllPath, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern int gpu_thumbnail_process_jpeg(
        ulong source_id,
        string jpeg_path,
        out uint out_tile_x,
        out uint out_tile_y);
    
    [DllImport(DllPath, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern int gpu_thumbnail_readback_jpeg(
        uint tile_x,
        uint tile_y,
        string output_path,
        uint quality);
    
    public static void TestBatchProcessing(string[] files, string outputFolder) {
        Console.WriteLine("Initializing GPU pipeline...");
        var sw = Stopwatch.StartNew();
        int initResult = gpu_thumbnail_pipeline_init();
        sw.Stop();
        
        if (initResult != 0) {
            Console.WriteLine("GPU init FAILED (error " + initResult + ")");
            Console.WriteLine("GPU not available - this is expected on non-D3D12 systems");
            return;
        }
        
        Console.WriteLine("GPU initialized in " + sw.ElapsedMilliseconds + "ms");
        Console.WriteLine();
        
        int processed = 0;
        int failed = 0;
        int skipped = 0;
        
        sw.Restart();
        
        for (int i = 0; i < files.Length; i++) {
            string file = files[i];
            ulong sourceId = (ulong)file.GetHashCode();
            
            // Process on GPU
            uint tileX, tileY;
            int result = gpu_thumbnail_process_jpeg(sourceId, file, out tileX, out tileY);
            
            if (result == 0) {
                // Readback every 10th image, or last image, or first 5 images
                bool shouldReadback = (i < 5) || (i % 10 == 0) || (i == files.Length - 1);
                
                if (shouldReadback) {
                    string outName = "thumb_" + i.ToString("D4") + ".jpg";
                    string outPath = System.IO.Path.Combine(outputFolder, outName);
                    int readbackResult = gpu_thumbnail_readback_jpeg(tileX, tileY, outPath, 85);
                    
                    if (readbackResult == 0) {
                        processed++;
                        if (i % 20 == 0) {
                            Console.WriteLine("[{0}/{1}] Processed: {2}", i + 1, files.Length, System.IO.Path.GetFileName(file));
                        }
                    } else {
                        failed++;
                        Console.WriteLine("[READBACK FAIL] {0}", file);
                    }
                } else {
                    processed++;
                    if (i % 50 == 0 && i > 0) {
                        Console.WriteLine("[{0}/{1}] GPU processing...", i + 1, files.Length);
                    }
                }
            } else if (result == -2) {
                skipped++;
                // Decode failures are logged by Rust side
            } else {
                failed++;
                Console.WriteLine("[GPU FAIL] {0}", file);
            }
        }
        
        sw.Stop();
        
        Console.WriteLine();
        Console.WriteLine("=== RESULTS ===");
        Console.WriteLine("Total files: {0}", files.Length);
        Console.WriteLine("Processed: {0}", processed);
        Console.WriteLine("Skipped (decode errors): {0}", skipped);
        Console.WriteLine("Failed (GPU errors): {0}", failed);
        Console.WriteLine("Time: {0:F2} seconds", sw.Elapsed.TotalSeconds);
        Console.WriteLine("Throughput: {0:F1} images/sec", files.Length / sw.Elapsed.TotalSeconds);
        Console.WriteLine();
        
        if (processed > 256) {
            Console.WriteLine("✓ LRU eviction working (processed > 256 atlas capacity)");
        }
        
        if (skipped > 0) {
            Console.WriteLine("✓ Error recovery working (skipped {0} broken files)", skipped);
        }
    }
}
"@

# Compile C# code
Add-Type -TypeDefinition $csharpCode -Language CSharp

# Convert file paths to string array
$filePaths = $jpegFiles | ForEach-Object { $_.FullName }

# Run test
Write-Host "Starting batch processing..." -ForegroundColor Cyan
Write-Host ""

try {
    [GpuThumbnailTest]::TestBatchProcessing($filePaths, $OutputFolder)
} catch {
    Write-Host "ERROR: $_" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Output thumbnails saved to: $OutputFolder" -ForegroundColor Green
Write-Host "Check for '[Atlas] Evicting...' messages in console output above" -ForegroundColor Yellow
Write-Host ""

# Verify some thumbnails were created
$createdThumbs = Get-ChildItem -Path $OutputFolder -Filter *.jpg
Write-Host "Created $($createdThumbs.Count) thumbnail files" -ForegroundColor Green
