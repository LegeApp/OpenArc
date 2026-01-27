using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Extensions.Logging;
using DocBrake.Models;
using DocBrake.NativeInterop;

#nullable enable

namespace DocBrake.Services
{
    public class OpenArcProcessingService : IDocumentProcessingService
    {
        private readonly ILogger<OpenArcProcessingService> _logger;
        private volatile bool _cancelRequested;
        private readonly object _activeProcessLock = new();
        private Process? _activeProcess;

        public event EventHandler<DocumentProcessingProgress>? ProgressUpdated;
        public event EventHandler<DocumentProcessingResult>? ProcessingCompleted;
        public event EventHandler<string>? ProcessingError;

        public OpenArcProcessingService(ILogger<OpenArcProcessingService> logger)
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
        }

        public Task<bool> ValidateDocumentAsync(string filePath, CancellationToken cancellationToken = default)
        {
            return Task.FromResult(!string.IsNullOrEmpty(filePath) && File.Exists(filePath));
        }

        public async Task<DocumentProcessingResult> ProcessDocumentAsync(
            string inputPath,
            ProcessingOptions options,
            IProgress<DocumentProcessingProgress>? progress = null,
            CancellationToken cancellationToken = default)
        {
            var results = await ProcessDocumentsAsync(new[] { inputPath }, options, progress, cancellationToken);
            return results.FirstOrDefault() ?? new DocumentProcessingResult { Success = false, Error = "No result" };
        }

        public Task<List<DocumentProcessingResult>> ProcessDocumentsAsync(
            IEnumerable<string> inputPaths,
            ProcessingOptions options,
            IProgress<DocumentProcessingProgress>? progress = null,
            CancellationToken cancellationToken = default)
        {
            if (options == null) throw new ArgumentNullException(nameof(options));

            var paths = inputPaths?.Where(p => !string.IsNullOrWhiteSpace(p)).ToArray() ?? Array.Empty<string>();
            if (paths.Length == 0)
            {
                return Task.FromResult(new List<DocumentProcessingResult>());
            }

            if (string.IsNullOrWhiteSpace(options.OutputArchivePath))
            {
                return Task.FromResult(new List<DocumentProcessingResult>
                {
                    new DocumentProcessingResult { Success = false, Error = "Output archive path is required" }
                });
            }

            return Task.Run(async () =>
            {
                _cancelRequested = false;

                var exePath = TryResolveOpenArcCliPath();
                if (!string.IsNullOrWhiteSpace(exePath))
                {
                    try
                    {
                        return await ProcessDocumentsViaCliAsync(exePath!, paths, options, progress, cancellationToken);
                    }
                    catch (OperationCanceledException)
                    {
                        return paths.Select(_ => new DocumentProcessingResult
                        {
                            Success = false,
                            Error = "Cancelled"
                        }).ToList();
                    }
                }

                var results = paths.Select(_ => new DocumentProcessingResult()).ToList();

                try
                {
                    var settings = new OpenArcFFI.CompressionSettings
                    {
                        BpgQuality = options.BpgQuality,
                        BpgLossless = options.BpgLossless,
                        BpgBitDepth = options.BpgBitDepth,
                        BpgChromaFormat = options.BpgChromaFormat,
                        BpgEncoderType = options.BpgEncoderType,
                        BpgCompressionLevel = options.BpgCompressionLevel,
                        VideoCodec = (int)options.VideoCodec,
                        VideoSpeed = (int)options.VideoSpeed,
                        VideoCrf = options.VideoCrf,
                        CompressionLevel = options.CompressionLevel,
                        EnableCatalog = options.EnableCatalog,
                        EnableDedup = options.EnableDedup,
                        SkipAlreadyCompressedVideos = options.SkipAlreadyCompressedVideos
                    };

                    OpenArcFFI.ProgressCallback cb = p =>
                    {
                        if (_cancelRequested || cancellationToken.IsCancellationRequested)
                            return;

                        var current = Math.Max(0, p.CurrentFile);
                        var total = Math.Max(0, p.TotalFiles);
                        var msg = $"Archiving {current}/{total}";

                        var progressUpdate = new DocumentProcessingProgress
                        {
                            Current = current,
                            Total = total,
                            Status = string.IsNullOrWhiteSpace(p.CurrentFileName) ? msg : $"{msg}: {p.CurrentFileName}"
                        };

                        progress?.Report(progressUpdate);
                        ProgressUpdated?.Invoke(this, progressUpdate);
                    };

                    var rc = OpenArcFFI.CreateArchive(
                        options.OutputArchivePath,
                        paths,
                        paths.Length,
                        ref settings,
                        cb);

                    // Prevent GC from collecting the callback delegate during native execution
                    GC.KeepAlive(cb);

                    if (_cancelRequested || cancellationToken.IsCancellationRequested)
                    {
                        return paths.Select(_ => new DocumentProcessingResult
                        {
                            Success = false,
                            Error = "Cancelled"
                        }).ToList();
                    }

                    if (rc < 0)
                    {
                        var err = OpenArcFFI.GetLastErrorMessage();
                        ProcessingError?.Invoke(this, err);
                        return paths.Select(_ => new DocumentProcessingResult
                        {
                            Success = false,
                            Error = err
                        }).ToList();
                    }

                    for (int i = 0; i < results.Count; i++)
                    {
                        results[i].Success = true;
                        results[i].OutputPath = options.OutputArchivePath;
                    }

                    var completed = new DocumentProcessingResult
                    {
                        Success = true,
                        OutputPath = options.OutputArchivePath
                    };

                    ProcessingCompleted?.Invoke(this, completed);
                    return results;
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "OpenArc processing failed");
                    ProcessingError?.Invoke(this, ex.Message);

                    return paths.Select(_ => new DocumentProcessingResult
                    {
                        Success = false,
                        Error = ex.Message
                    }).ToList();
                }
            }, cancellationToken);
        }

        public void CancelProcessing()
        {
            _cancelRequested = true;

            lock (_activeProcessLock)
            {
                if (_activeProcess != null)
                {
                    try
                    {
                        if (!_activeProcess.HasExited)
                        {
                            _activeProcess.Kill(entireProcessTree: true);
                        }
                    }
                    catch
                    {
                    }
                }
            }
        }

        private static string? TryResolveOpenArcCliPath()
        {
            try
            {
                var baseDir = AppContext.BaseDirectory;
                if (string.IsNullOrWhiteSpace(baseDir))
                    return null;

                var candidate = Path.Combine(baseDir, "openarc.exe");
                if (File.Exists(candidate))
                    return candidate;
            }
            catch
            {
            }

            return null;
        }

        private async Task<List<DocumentProcessingResult>> ProcessDocumentsViaCliAsync(
            string openarcExe,
            string[] inputFiles,
            ProcessingOptions options,
            IProgress<DocumentProcessingProgress>? progress,
            CancellationToken cancellationToken)
        {
            var results = inputFiles.Select(_ => new DocumentProcessingResult()).ToList();

            var psi = new ProcessStartInfo
            {
                FileName = openarcExe,
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                CreateNoWindow = true,
                WorkingDirectory = Path.GetDirectoryName(openarcExe) ?? Environment.CurrentDirectory
            };

            // openarc create --output <file> <inputs...>
            psi.ArgumentList.Add("create");
            psi.ArgumentList.Add("--output");
            psi.ArgumentList.Add(options.OutputArchivePath);

            // Settings mapping (matches openarc-ffi mapping)
            psi.ArgumentList.Add("--bpg-quality");
            psi.ArgumentList.Add(options.BpgQuality.ToString());
            if (options.BpgLossless)
                psi.ArgumentList.Add("--bpg-lossless");

            var videoPreset = (options.VideoCodec, options.VideoSpeed) switch
            {
                (Models.VideoCodec.H264, Models.VideoSpeed.Medium) => 0,
                (Models.VideoCodec.H265, Models.VideoSpeed.Medium) => 1,
                (Models.VideoCodec.H264, Models.VideoSpeed.Fast) => 2,
                (Models.VideoCodec.H265, Models.VideoSpeed.Slow) => 3,
                (Models.VideoCodec.H264, _) => 2,
                (Models.VideoCodec.H265, _) => 1,
                _ => 0
            };

            psi.ArgumentList.Add("--video-preset");
            psi.ArgumentList.Add(videoPreset.ToString());
            psi.ArgumentList.Add("--video-crf");
            psi.ArgumentList.Add(options.VideoCrf.ToString());
            psi.ArgumentList.Add("--compression-level");
            psi.ArgumentList.Add(options.CompressionLevel.ToString());

            if (!options.EnableCatalog)
                psi.ArgumentList.Add("--no-catalog");
            if (!options.EnableDedup)
                psi.ArgumentList.Add("--no-dedup");
            if (!options.SkipAlreadyCompressedVideos)
                psi.ArgumentList.Add("--no-skip-compressed");

            foreach (var f in inputFiles)
            {
                psi.ArgumentList.Add(f);
            }

            progress?.Report(new DocumentProcessingProgress
            {
                Current = 0,
                Total = inputFiles.Length,
                Status = $"Starting archive process ({inputFiles.Length} file(s))..."
            });

            var stdout = new List<string>();
            var stderr = new List<string>();

            using var proc = new Process { StartInfo = psi, EnableRaisingEvents = true };
            proc.OutputDataReceived += (_, e) => { if (e.Data != null) lock (stdout) stdout.Add(e.Data); };
            proc.ErrorDataReceived += (_, e) => { if (e.Data != null) lock (stderr) stderr.Add(e.Data); };

            lock (_activeProcessLock)
            {
                _activeProcess = proc;
            }

            try
            {
                if (!proc.Start())
                {
                    return inputFiles.Select(_ => new DocumentProcessingResult
                    {
                        Success = false,
                        Error = "Failed to start openarc.exe"
                    }).ToList();
                }

                proc.BeginOutputReadLine();
                proc.BeginErrorReadLine();

                using var reg = cancellationToken.Register(() =>
                {
                    try
                    {
                        if (!proc.HasExited)
                            proc.Kill(entireProcessTree: true);
                    }
                    catch
                    {
                    }
                });

                await proc.WaitForExitAsync(cancellationToken);

                if (_cancelRequested || cancellationToken.IsCancellationRequested)
                    throw new OperationCanceledException(cancellationToken);

                if (proc.ExitCode != 0)
                {
                    string err;
                    lock (stderr)
                    {
                        err = stderr.Count > 0 ? string.Join(Environment.NewLine, stderr.TakeLast(50)) : $"openarc.exe exited with code {proc.ExitCode}";
                    }

                    ProcessingError?.Invoke(this, err);
                    return inputFiles.Select(_ => new DocumentProcessingResult
                    {
                        Success = false,
                        Error = err
                    }).ToList();
                }

                progress?.Report(new DocumentProcessingProgress
                {
                    Current = inputFiles.Length,
                    Total = inputFiles.Length,
                    Status = "Archive created"
                });
                ProgressUpdated?.Invoke(this, new DocumentProcessingProgress
                {
                    Current = inputFiles.Length,
                    Total = inputFiles.Length,
                    Status = "Archive created"
                });

                for (int i = 0; i < results.Count; i++)
                {
                    results[i].Success = true;
                    results[i].OutputPath = options.OutputArchivePath;
                }

                ProcessingCompleted?.Invoke(this, new DocumentProcessingResult
                {
                    Success = true,
                    OutputPath = options.OutputArchivePath
                });

                return results;
            }
            finally
            {
                lock (_activeProcessLock)
                {
                    if (ReferenceEquals(_activeProcess, proc))
                        _activeProcess = null;
                }
            }
        }

        public async Task<bool> ExtractArchiveAsync(
            string archivePath,
            string outputDirectory,
            IProgress<DocumentProcessingProgress>? progress = null,
            CancellationToken cancellationToken = default)
        {
            if (string.IsNullOrWhiteSpace(archivePath) || string.IsNullOrWhiteSpace(outputDirectory))
            {
                ProcessingError?.Invoke(this, "Archive path and output directory are required");
                return false;
            }

            return await Task.Run(() =>
            {
                _cancelRequested = false;

                try
                {
                    OpenArcFFI.ProgressCallback cb = p =>
                    {
                        if (_cancelRequested || cancellationToken.IsCancellationRequested)
                            return;

                        var current = Math.Max(0, p.CurrentFile);
                        var total = Math.Max(0, p.TotalFiles);
                        var msg = $"Extracting {current}/{total}";

                        var progressUpdate = new DocumentProcessingProgress
                        {
                            Current = current,
                            Total = total,
                            Status = string.IsNullOrWhiteSpace(p.CurrentFileName) ? msg : $"{msg}: {p.CurrentFileName}"
                        };

                        progress?.Report(progressUpdate);
                        ProgressUpdated?.Invoke(this, progressUpdate);
                    };

                    var rc = OpenArcFFI.ExtractArchive(archivePath, outputDirectory, cb);

                    // Prevent GC from collecting the callback delegate during native execution
                    GC.KeepAlive(cb);

                    if (_cancelRequested || cancellationToken.IsCancellationRequested)
                    {
                        return false;
                    }

                    if (rc < 0)
                    {
                        var err = OpenArcFFI.GetLastErrorMessage();
                        ProcessingError?.Invoke(this, err);
                        return false;
                    }

                    var completed = new DocumentProcessingResult
                    {
                        Success = true,
                        OutputPath = outputDirectory
                    };

                    ProcessingCompleted?.Invoke(this, completed);
                    return true;
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "Archive extraction failed");
                    ProcessingError?.Invoke(this, ex.Message);
                    return false;
                }
            }, cancellationToken);
        }

        public async Task<List<ArchiveFileInfo>> ListArchiveAsync(string archivePath, CancellationToken cancellationToken = default)
        {
            if (string.IsNullOrWhiteSpace(archivePath))
            {
                ProcessingError?.Invoke(this, "Archive path is required");
                return new List<ArchiveFileInfo>();
            }

            return await Task.Run(() =>
            {
                try
                {
                    var result = OpenArcFFI.ListArchive(archivePath, out int fileCount, out IntPtr filesPtr);

                    if (result < 0)
                    {
                        var err = OpenArcFFI.GetLastErrorMessage();
                        ProcessingError?.Invoke(this, err);
                        return new List<ArchiveFileInfo>();
                    }

                    var fileList = new List<ArchiveFileInfo>();
                    
                    if (fileCount > 0 && filesPtr != IntPtr.Zero)
                    {
                        var ptrSize = Marshal.SizeOf<OpenArcFFI.ArchiveFileInfo>();
                        for (int i = 0; i < fileCount; i++)
                        {
                            var ptr = new IntPtr(filesPtr.ToInt64() + i * ptrSize);
                            var ffiInfo = Marshal.PtrToStructure<OpenArcFFI.ArchiveFileInfo>(ptr);
                            
                            fileList.Add(new ArchiveFileInfo
                            {
                                Filename = ffiInfo.Filename ?? $"File {i + 1}",
                                OriginalSize = ffiInfo.OriginalSize,
                                CompressedSize = ffiInfo.CompressedSize,
                                FileType = (FileType)ffiInfo.FileType
                            });
                        }
                    }

                    return fileList;
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "Archive listing failed");
                    ProcessingError?.Invoke(this, ex.Message);
                    return new List<ArchiveFileInfo>();
                }
            }, cancellationToken);
        }

        public async Task<bool> EncodeBpgFileAsync(string inputPath, string outputPath, ProcessingOptions options, CancellationToken cancellationToken = default)
        {
            if (string.IsNullOrWhiteSpace(inputPath) || string.IsNullOrWhiteSpace(outputPath))
            {
                ProcessingError?.Invoke(this, "Input and output paths are required");
                return false;
            }

            return await Task.Run(() =>
            {
                _cancelRequested = false;

                try
                {
                    var settings = new OpenArcFFI.CompressionSettings
                    {
                        BpgQuality = options.BpgQuality,
                        BpgLossless = options.BpgLossless,
                        BpgBitDepth = options.BpgBitDepth,
                        BpgChromaFormat = options.BpgChromaFormat,
                        BpgEncoderType = options.BpgEncoderType,
                        BpgCompressionLevel = options.BpgCompressionLevel,
                        VideoCodec = (int)options.VideoCodec,
                        VideoSpeed = (int)options.VideoSpeed,
                        VideoCrf = options.VideoCrf,
                        CompressionLevel = options.CompressionLevel,
                        EnableCatalog = options.EnableCatalog,
                        EnableDedup = options.EnableDedup,
                        SkipAlreadyCompressedVideos = options.SkipAlreadyCompressedVideos
                    };

                    var rc = OpenArcFFI.EncodeBpgFile(inputPath, outputPath, ref settings);

                    if (_cancelRequested || cancellationToken.IsCancellationRequested)
                    {
                        return false;
                    }

                    if (rc < 0)
                    {
                        var err = OpenArcFFI.GetLastErrorMessage();
                        ProcessingError?.Invoke(this, err);
                        return false;
                    }

                    return true;
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "BPG encoding failed");
                    ProcessingError?.Invoke(this, ex.Message);
                    return false;
                }
            }, cancellationToken);
        }

        public async Task<bool> EncodeVideoFileAsync(string inputPath, string outputPath, ProcessingOptions options, CancellationToken cancellationToken = default)
        {
            if (string.IsNullOrWhiteSpace(inputPath) || string.IsNullOrWhiteSpace(outputPath))
            {
                ProcessingError?.Invoke(this, "Input and output paths are required");
                return false;
            }

            return await Task.Run(() =>
            {
                _cancelRequested = false;

                try
                {
                    var settings = new OpenArcFFI.CompressionSettings
                    {
                        BpgQuality = options.BpgQuality,
                        BpgLossless = options.BpgLossless,
                        BpgBitDepth = options.BpgBitDepth,
                        BpgChromaFormat = options.BpgChromaFormat,
                        BpgEncoderType = options.BpgEncoderType,
                        BpgCompressionLevel = options.BpgCompressionLevel,
                        VideoCodec = (int)options.VideoCodec,
                        VideoSpeed = (int)options.VideoSpeed,
                        VideoCrf = options.VideoCrf,
                        CompressionLevel = options.CompressionLevel,
                        EnableCatalog = options.EnableCatalog,
                        EnableDedup = options.EnableDedup,
                        SkipAlreadyCompressedVideos = options.SkipAlreadyCompressedVideos
                    };

                    var rc = OpenArcFFI.EncodeVideoFile(inputPath, outputPath, ref settings);

                    if (_cancelRequested || cancellationToken.IsCancellationRequested)
                    {
                        return false;
                    }

                    if (rc < 0)
                    {
                        var err = OpenArcFFI.GetLastErrorMessage();
                        ProcessingError?.Invoke(this, err);
                        return false;
                    }

                    return true;
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "Video encoding failed");
                    ProcessingError?.Invoke(this, ex.Message);
                    return false;
                }
            }, cancellationToken);
        }
    }
}
