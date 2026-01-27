using Microsoft.Extensions.Logging;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using DocBrake.Models;
using DocBrake.Services;
using DocBrake.NativeInterop;

namespace DocBrake.Services
{
    public interface ICLIInterface
    {
        Task<int> ProcessCommandLineAsync(string[] args);
    }

    public class CLIInterface : ICLIInterface
    {
        private readonly ILogger<CLIInterface> _logger;
        private readonly IDocumentProcessingService _processingService;
        private readonly ISettingsService _settingsService;

        public CLIInterface(
            ILogger<CLIInterface> logger,
            IDocumentProcessingService processingService,
            ISettingsService settingsService)
        {
            _logger = logger;
            _processingService = processingService;
            _settingsService = settingsService;
        }

        public Task<int> ProcessCommandLineAsync(string[] args)
        {
            Console.WriteLine("DocBrake CLI Interface");
            Console.WriteLine("======================");

            if (args.Length == 0)
            {
                ShowHelp();
                return Task.FromResult(0);
            }

            try
            {
                var command = args[0].ToLower();
                var commandArgs = args.Skip(1).ToArray();

                switch (command)
                {
                    case "help":
                    case "--help":
                    case "-h":
                        ShowHelp();
                        return Task.FromResult(0);

                    case "test":
                        return RunTestAsync(commandArgs);

                    case "process":
                        return ProcessFilesAsync(commandArgs);

                    case "settings":
                        return Task.FromResult(ShowSettings());

                    case "detect":
                        return TestDetectionAsync(commandArgs);

                    case "test-archive":
                        return TestArchiveCreationAsync();

                    case "create-archive":
                        return CreateArchiveDirectAsync(commandArgs);

                    case "test-jpeg":
                        return TestJpegArchiveAsync(commandArgs);

                    default:
                        Console.WriteLine($"Unknown command: {command}");
                        ShowHelp();
                        return Task.FromResult(1);
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine($"CLI Error: {ex.Message}");
                Console.WriteLine($"Stack Trace: {ex.StackTrace}");
                return Task.FromResult(1);
            }
        }

        private void ShowHelp()
        {
            Console.WriteLine("Available commands:");
            Console.WriteLine("  help                    - Show this help message");
            Console.WriteLine("  test [test_name]        - Run tests (all, ffi, processing, detection)");
            Console.WriteLine("  test-archive            - Test direct archive creation via FFI");
            Console.WriteLine("  create-archive <out> <files> - Create archive directly via FFI");
            Console.WriteLine("  process <files>         - Process specified files via service");
            Console.WriteLine("  settings                - Show current settings");
            Console.WriteLine("  detect <file>           - Test file type detection");
            Console.WriteLine();
            Console.WriteLine("Examples:");
            Console.WriteLine("  DocBrakeGUI.exe test ffi");
            Console.WriteLine("  DocBrakeGUI.exe test-archive");
            Console.WriteLine("  DocBrakeGUI.exe create-archive output.oarc file1.jpg file2.pdf");
            Console.WriteLine("  DocBrakeGUI.exe process \"file1.jpg\" \"file2.pdf\"");
            Console.WriteLine("  DocBrakeGUI.exe detect test.jpg");
        }

        private Task<int> RunTestAsync(string[] args)
        {
            var testName = args.Length > 0 ? args[0].ToLower() : "all";

            Console.WriteLine($"Running tests: {testName}");
            Console.WriteLine("========================");

            try
            {
                switch (testName)
                {
                    case "all":
                        return RunAllTestsAsync();

                    case "ffi":
                        return TestFFIAsync();

                    case "processing":
                        return TestProcessingAsync();

                    case "detection":
                        return TestDetectionAsync(Array.Empty<string>());

                    default:
                        Console.WriteLine($"Unknown test: {testName}");
                        return Task.FromResult(1);
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Test failed: {ex.Message}");
                Console.WriteLine($"Stack Trace: {ex.StackTrace}");
                return Task.FromResult(1);
            }
        }

        private async Task<int> RunAllTestsAsync()
        {
            var results = new List<(string test, int result)>();

            Console.WriteLine("Running FFI Test...");
            results.Add(("FFI", await TestFFIAsync()));
            Console.WriteLine();

            Console.WriteLine("Running Processing Test...");
            results.Add(("Processing", await TestProcessingAsync()));
            Console.WriteLine();

            Console.WriteLine("Running Detection Test...");
            results.Add(("Detection", await TestDetectionAsync(Array.Empty<string>())));
            Console.WriteLine();

            Console.WriteLine("Test Results Summary:");
            Console.WriteLine("======================");
            foreach (var (test, result) in results)
            {
                Console.WriteLine($"{test}: {(result == 0 ? "PASS" : "FAIL")}");
            }

            var overallResult = results.All(r => r.result == 0) ? 0 : 1;
            Console.WriteLine($"Overall: {(overallResult == 0 ? "PASS" : "FAIL")}");

            return overallResult;
        }

        private Task<int> TestFFIAsync()
        {
            try
            {
                Console.WriteLine("Testing FFI function availability...");

                // Test if we can call GetLastError (should always work)
                var errorMsg = OpenArcFFI.GetLastErrorMessage();
                Console.WriteLine($"FFI GetLastError test: SUCCESS");
                Console.WriteLine($"  Last error: '{errorMsg}'");

                // Test file type detection with a non-existent file
                var fileType = OpenArcFFI.DetectFileType("non_existent_file.jpg");
                Console.WriteLine($"FFI DetectFileType test: SUCCESS");
                Console.WriteLine($"  File type result: {fileType}");

                Console.WriteLine("FFI tests completed successfully");
                return Task.FromResult(0);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"FFI test failed: {ex.Message}");
                Console.WriteLine($"Stack Trace: {ex.StackTrace}");
                return Task.FromResult(1);
            }
        }

        private async Task<int> TestProcessingAsync()
        {
            try
            {
                Console.WriteLine("Testing processing service...");

                // Create a temporary test file
                var testFile = Path.GetTempFileName();
                try
                {
                    File.WriteAllText(testFile, "test content");
                    Console.WriteLine($"Created test file: {testFile}");

                    var settings = _settingsService.LoadSettings();
                    Console.WriteLine($"Loaded settings successfully");

                    // Test processing (this will likely fail but we can see the error)
                    Console.WriteLine("Attempting to process test file...");
                    await _processingService.ProcessDocumentsAsync(new[] { testFile }, settings);

                    Console.WriteLine("Processing test completed");
                    return 0;
                }
                finally
                {
                    if (File.Exists(testFile))
                    {
                        File.Delete(testFile);
                        Console.WriteLine($"Cleaned up test file: {testFile}");
                    }
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Processing test failed: {ex.Message}");
                Console.WriteLine($"Stack Trace: {ex.StackTrace}");
                return 1;
            }
        }

        private Task<int> TestDetectionAsync(string[] args)
        {
            try
            {
                Console.WriteLine("Testing file type detection...");

                if (args.Length > 0)
                {
                    // Test specific file
                    var testFile = args[0];
                    if (!File.Exists(testFile))
                    {
                        Console.WriteLine($"File not found: {testFile}");
                        return Task.FromResult(1);
                    }

                    var fileType = OpenArcFFI.DetectFileType(testFile);
                    Console.WriteLine($"File: {testFile}");
                    Console.WriteLine($"Detected type: {GetFileTypeString(fileType)}");
                }
                else
                {
                    // Test with some common file types (create temporary files)
                    var testFiles = new[]
                    {
                        ("test.jpg", "image"),
                        ("test.mp4", "video"),
                        ("test.pdf", "document"),
                        ("test.txt", "document")
                    };

                    foreach (var (filename, expectedType) in testFiles)
                    {
                        var testFile = Path.GetTempFileName();
                        var testPath = Path.ChangeExtension(testFile, Path.GetExtension(filename));
                        File.Move(testFile, testPath);

                        try
                        {
                            File.WriteAllText(testPath, $"test {expectedType} content");
                            var fileType = OpenArcFFI.DetectFileType(testPath);
                            Console.WriteLine($"{filename}: {GetFileTypeString(fileType)} (expected: {expectedType})");
                        }
                        finally
                        {
                            if (File.Exists(testPath))
                            {
                                File.Delete(testPath);
                            }
                        }
                    }
                }

                Console.WriteLine("Detection test completed");
                return Task.FromResult(0);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Detection test failed: {ex.Message}");
                Console.WriteLine($"Stack Trace: {ex.StackTrace}");
                return Task.FromResult(1);
            }
        }

        private async Task<int> ProcessFilesAsync(string[] args)
        {
            if (args.Length == 0)
            {
                Console.WriteLine("Error: No files specified for processing");
                Console.WriteLine("Usage: process <file1> <file2> ...");
                return 1;
            }

            try
            {
                var files = new List<string>();
                foreach (var arg in args)
                {
                    if (File.Exists(arg))
                    {
                        files.Add(arg);
                    }
                    else
                    {
                        Console.WriteLine($"Warning: File not found: {arg}");
                    }
                }

                if (files.Count == 0)
                {
                    Console.WriteLine("Error: No valid files found");
                    return 1;
                }

                Console.WriteLine($"Processing {files.Count} files...");
                foreach (var file in files)
                {
                    Console.WriteLine($"  {file}");
                }

                var settings = _settingsService.LoadSettings();
                Console.WriteLine("Starting processing...");
                
                await _processingService.ProcessDocumentsAsync(files.ToArray(), settings);

                Console.WriteLine("Processing completed");
                return 0;
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Processing failed: {ex.Message}");
                Console.WriteLine($"Stack Trace: {ex.StackTrace}");
                return 1;
            }
        }

        private int ShowSettings()
        {
            try
            {
                var settings = _settingsService.LoadSettings();
                
                Console.WriteLine("Current Settings:");
                Console.WriteLine("==================");
                Console.WriteLine($"BPG Quality: {settings.BpgQuality}");
                Console.WriteLine($"BPG Lossless: {settings.BpgLossless}");
                Console.WriteLine($"BPG Bit Depth: {settings.BpgBitDepth}");
                Console.WriteLine($"BPG Chroma Format: {settings.BpgChromaFormat}");
                Console.WriteLine($"BPG Encoder Type: {settings.BpgEncoderType}");
                Console.WriteLine($"BPG Compression Level: {settings.BpgCompressionLevel}");
                Console.WriteLine($"Video Codec: {settings.VideoCodec}");
                Console.WriteLine($"Video Speed: {settings.VideoSpeed}");
                Console.WriteLine($"Video CRF: {settings.VideoCrf}");
                Console.WriteLine($"Archive Compression Level: {settings.CompressionLevel}");
                Console.WriteLine($"Enable Catalog: {settings.EnableCatalog}");
                Console.WriteLine($"Enable Dedup: {settings.EnableDedup}");

                return 0;
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Failed to show settings: {ex.Message}");
                return 1;
            }
        }

        private string GetFileTypeString(int fileType)
        {
            return fileType switch
            {
                0 => "Unknown",
                1 => "Image",
                2 => "Video", 
                3 => "Document",
                _ => $"Other({fileType})"
            };
        }

        private Task<int> TestArchiveCreationAsync()
        {
            Console.WriteLine("Testing direct archive creation via FFI...");
            Console.WriteLine("==========================================");

            string testDir = Path.Combine(Path.GetTempPath(), "docbrake_test_" + Guid.NewGuid().ToString("N").Substring(0, 8));
            string outputArchive = Path.Combine(testDir, "test_output.oarc");

            try
            {
                Directory.CreateDirectory(testDir);
                Console.WriteLine($"Test directory: {testDir}");

                // Create test files
                var testFile1 = Path.Combine(testDir, "test1.txt");
                var testFile2 = Path.Combine(testDir, "test2.txt");
                File.WriteAllText(testFile1, "This is test file 1 content for archive testing.");
                File.WriteAllText(testFile2, "This is test file 2 content for archive testing.");
                Console.WriteLine($"Created test files: {testFile1}, {testFile2}");

                var inputFiles = new[] { testFile1, testFile2 };

                // Create compression settings
                var settings = new OpenArcFFI.CompressionSettings
                {
                    BpgQuality = 28,
                    BpgLossless = false,
                    BpgBitDepth = 8,
                    BpgChromaFormat = 0,
                    BpgEncoderType = 0,
                    BpgCompressionLevel = 5,
                    VideoCodec = 0,
                    VideoSpeed = 1,
                    VideoCrf = 23,
                    CompressionLevel = 9,
                    EnableCatalog = false,
                    EnableDedup = false,
                    SkipAlreadyCompressedVideos = false
                };

                Console.WriteLine("Calling OpenArcFFI.CreateArchive...");
                Console.WriteLine($"  Output: {outputArchive}");
                Console.WriteLine($"  Files: {string.Join(", ", inputFiles)}");
                Console.WriteLine($"  Compression Level: {settings.CompressionLevel}");

                int result = OpenArcFFI.CreateArchive(outputArchive, inputFiles, inputFiles.Length, ref settings, null!);

                Console.WriteLine($"CreateArchive returned: {result}");

                // FFI returns file count on success, negative on error
                if (result < 0)
                {
                    var errorMsg = OpenArcFFI.GetLastErrorMessage();
                    Console.WriteLine($"ERROR: {errorMsg}");
                    return Task.FromResult(1);
                }

                Console.WriteLine($"Processed {result} files");

                // Check if archive was created
                if (File.Exists(outputArchive))
                {
                    var fileInfo = new FileInfo(outputArchive);
                    Console.WriteLine($"SUCCESS: Archive created at {outputArchive}");
                    Console.WriteLine($"  Size: {fileInfo.Length} bytes");
                    return Task.FromResult(0);
                }
                else
                {
                    Console.WriteLine("ERROR: Archive file was not created");
                    return Task.FromResult(1);
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Exception during archive test: {ex.Message}");
                Console.WriteLine($"Stack trace: {ex.StackTrace}");
                return Task.FromResult(1);
            }
            finally
            {
                // Cleanup
                try
                {
                    if (Directory.Exists(testDir))
                    {
                        Directory.Delete(testDir, true);
                        Console.WriteLine($"Cleaned up test directory: {testDir}");
                    }
                }
                catch { }
            }
        }

        private Task<int> TestJpegArchiveAsync(string[] args)
        {
            Console.WriteLine("Testing JPEG archive creation with catalog+dedup (GUI-like settings)...");
            Console.WriteLine("======================================================================");

            string testDir = Path.Combine(Path.GetTempPath(), "docbrake_jpeg_test_" + Guid.NewGuid().ToString("N").Substring(0, 8));
            string outputArchive = Path.Combine(testDir, "test_jpeg_output.oarc");

            try
            {
                Directory.CreateDirectory(testDir);

                // Use provided JPEG file or look for test_image.jpg next to exe
                string jpegPath;
                if (args.Length > 0 && File.Exists(args[0]))
                {
                    jpegPath = args[0];
                }
                else
                {
                    jpegPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "test_image.jpg");
                    if (!File.Exists(jpegPath))
                    {
                        Console.WriteLine("ERROR: No JPEG file provided and test_image.jpg not found next to exe");
                        Console.WriteLine("Usage: test-jpeg [path_to_jpeg]");
                        return Task.FromResult(1);
                    }
                }
                Console.WriteLine($"Using JPEG: {jpegPath} ({new FileInfo(jpegPath).Length} bytes)");

                var inputFiles = new[] { jpegPath };

                // Use settings matching GUI defaults (catalog + dedup enabled)
                // Test with lossless = true to match user's scenario
                var settings = new OpenArcFFI.CompressionSettings
                {
                    BpgQuality = 25,
                    BpgLossless = true,
                    BpgBitDepth = 8,
                    BpgChromaFormat = 1,  // 4:2:0 (GUI default)
                    BpgEncoderType = 0,
                    BpgCompressionLevel = 8,
                    VideoCodec = 0,
                    VideoSpeed = 1,
                    VideoCrf = 23,
                    CompressionLevel = 3,
                    EnableCatalog = true,   // GUI default
                    EnableDedup = true,     // GUI default
                    SkipAlreadyCompressedVideos = true
                };

                Console.WriteLine($"Settings: Quality={settings.BpgQuality}, Lossless={settings.BpgLossless}, Catalog={settings.EnableCatalog}, Dedup={settings.EnableDedup}");
                Console.WriteLine($"Output: {outputArchive}");
                Console.WriteLine("Calling CreateArchive with progress callback...");

                OpenArcFFI.ProgressCallback progressCb = p =>
                {
                    Console.WriteLine($"  Progress: {p.CurrentFile}/{p.TotalFiles} ({p.ProgressPercent:F1}%) - {p.CurrentFileName}");
                };

                int result = OpenArcFFI.CreateArchive(outputArchive, inputFiles, inputFiles.Length, ref settings, progressCb);
                GC.KeepAlive(progressCb);
                Console.WriteLine($"CreateArchive returned: {result}");

                if (result < 0)
                {
                    var errorMsg = OpenArcFFI.GetLastErrorMessage();
                    Console.WriteLine($"ERROR: {errorMsg}");
                    return Task.FromResult(1);
                }

                Console.WriteLine($"SUCCESS: Processed {result} file(s)");
                if (File.Exists(outputArchive))
                {
                    Console.WriteLine($"  Archive size: {new FileInfo(outputArchive).Length} bytes");
                }
                return Task.FromResult(0);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Exception: {ex.Message}");
                Console.WriteLine($"Stack: {ex.StackTrace}");
                return Task.FromResult(1);
            }
            finally
            {
                try { if (Directory.Exists(testDir)) Directory.Delete(testDir, true); } catch { }
            }
        }

        private Task<int> CreateArchiveDirectAsync(string[] args)
        {
            if (args.Length < 2)
            {
                Console.WriteLine("Usage: create-archive <output.oarc> <file1> [file2] ...");
                return Task.FromResult(1);
            }

            string outputPath = args[0];
            var inputFiles = args.Skip(1).ToArray();

            Console.WriteLine($"Creating archive: {outputPath}");
            Console.WriteLine($"Input files: {inputFiles.Length}");

            // Validate input files
            var validFiles = new List<string>();
            foreach (var file in inputFiles)
            {
                if (File.Exists(file))
                {
                    validFiles.Add(file);
                    Console.WriteLine($"  + {file}");
                }
                else
                {
                    Console.WriteLine($"  - {file} (NOT FOUND)");
                }
            }

            if (validFiles.Count == 0)
            {
                Console.WriteLine("ERROR: No valid input files found");
                return Task.FromResult(1);
            }

            try
            {
                var settings = new OpenArcFFI.CompressionSettings
                {
                    BpgQuality = 28,
                    BpgLossless = false,
                    BpgBitDepth = 8,
                    BpgChromaFormat = 0,
                    BpgEncoderType = 0,
                    BpgCompressionLevel = 5,
                    VideoCodec = 0,
                    VideoSpeed = 1,
                    VideoCrf = 23,
                    CompressionLevel = 9,
                    EnableCatalog = false,
                    EnableDedup = false,
                    SkipAlreadyCompressedVideos = false
                };

                Console.WriteLine("Calling OpenArcFFI.CreateArchive...");
                int result = OpenArcFFI.CreateArchive(outputPath, validFiles.ToArray(), validFiles.Count, ref settings, null!);

                Console.WriteLine($"CreateArchive returned: {result}");

                // FFI returns file count on success, negative on error
                if (result < 0)
                {
                    var errorMsg = OpenArcFFI.GetLastErrorMessage();
                    Console.WriteLine($"ERROR: {errorMsg}");
                    return Task.FromResult(1);
                }

                Console.WriteLine($"Processed {result} files");

                if (File.Exists(outputPath))
                {
                    var fileInfo = new FileInfo(outputPath);
                    Console.WriteLine($"SUCCESS: Archive created");
                    Console.WriteLine($"  Path: {outputPath}");
                    Console.WriteLine($"  Size: {fileInfo.Length} bytes");
                    return Task.FromResult(0);
                }
                else
                {
                    Console.WriteLine("ERROR: Archive file was not created");
                    return Task.FromResult(1);
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Exception: {ex.Message}");
                Console.WriteLine($"Stack trace: {ex.StackTrace}");
                return Task.FromResult(1);
            }
        }
    }
}
