using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Extensions.Logging;

namespace DocBrake.Services
{
    public interface IStagingService
    {
        string StagingDirectory { get; }
        string BackupManifestPath { get; }

        Task<StagingResult> StageFilesAsync(
            IEnumerable<string> sourceFiles,
            IProgress<StagingProgress>? progress = null,
            CancellationToken cancellationToken = default);

        Task<List<string>> GetNewFilesAsync(string sourcePath, bool recursive = true);
        Task<BackupManifest> LoadBackupManifestAsync(string deviceId);
        Task SaveBackupManifestAsync(string deviceId, BackupManifest manifest);
        Task MarkFilesAsBackedUpAsync(string deviceId, IEnumerable<string> files, string archivePath);
        void ClearStagingDirectory();
        long GetStagingDirectorySize();
        List<string> GetStagedFiles();
    }

    public class StagingProgress
    {
        public int TotalFiles { get; set; }
        public int ProcessedFiles { get; set; }
        public long TotalBytes { get; set; }
        public long ProcessedBytes { get; set; }
        public string CurrentFile { get; set; } = string.Empty;
        public double ProgressPercent => TotalBytes > 0 ? (double)ProcessedBytes / TotalBytes * 100 : 0;
    }

    public class StagingResult
    {
        public bool Success { get; set; }
        public int FilesStaged { get; set; }
        public long BytesStaged { get; set; }
        public List<string> StagedFiles { get; set; } = new();
        public List<string> FailedFiles { get; set; } = new();
        public string? ErrorMessage { get; set; }
    }

    public class BackupManifest
    {
        public string DeviceId { get; set; } = string.Empty;
        public string DeviceName { get; set; } = string.Empty;
        public DateTime LastBackupTime { get; set; }
        public string LastArchivePath { get; set; } = string.Empty;
        public Dictionary<string, BackedUpFileInfo> BackedUpFiles { get; set; } = new();
    }

    public class BackedUpFileInfo
    {
        public string RelativePath { get; set; } = string.Empty;
        public long FileSize { get; set; }
        public DateTime LastModified { get; set; }
        public DateTime BackedUpTime { get; set; }
        public string ArchivePath { get; set; } = string.Empty;
        public string FileHash { get; set; } = string.Empty;
    }

    public class StagingService : IStagingService
    {
        private readonly ILogger<StagingService> _logger;
        private readonly string _stagingDirectory;
        private readonly string _manifestsDirectory;

        public string StagingDirectory => _stagingDirectory;
        public string BackupManifestPath => _manifestsDirectory;

        public StagingService(ILogger<StagingService> logger)
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));

            var appDataPath = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            var openArcPath = Path.Combine(appDataPath, "OpenArc");

            _stagingDirectory = Path.Combine(openArcPath, "Staging");
            _manifestsDirectory = Path.Combine(openArcPath, "BackupManifests");

            Directory.CreateDirectory(_stagingDirectory);
            Directory.CreateDirectory(_manifestsDirectory);

            _logger.LogInformation("Staging directory: {StagingDir}", _stagingDirectory);
        }

        public async Task<StagingResult> StageFilesAsync(
            IEnumerable<string> sourceFiles,
            IProgress<StagingProgress>? progress = null,
            CancellationToken cancellationToken = default)
        {
            var result = new StagingResult();
            var files = sourceFiles.ToList();

            if (files.Count == 0)
            {
                result.Success = true;
                return result;
            }

            var progressData = new StagingProgress
            {
                TotalFiles = files.Count,
                TotalBytes = files.Sum(f => new FileInfo(f).Length)
            };

            progress?.Report(progressData);

            foreach (var sourceFile in files)
            {
                if (cancellationToken.IsCancellationRequested)
                    break;

                try
                {
                    var fileName = Path.GetFileName(sourceFile);
                    var destPath = GetUniqueStagingPath(fileName);

                    progressData.CurrentFile = fileName;
                    progress?.Report(progressData);

                    await CopyFileAsync(sourceFile, destPath, cancellationToken);

                    result.StagedFiles.Add(destPath);
                    result.FilesStaged++;
                    result.BytesStaged += new FileInfo(destPath).Length;

                    progressData.ProcessedFiles++;
                    progressData.ProcessedBytes += new FileInfo(destPath).Length;
                    progress?.Report(progressData);

                    _logger.LogDebug("Staged file: {Source} -> {Dest}", sourceFile, destPath);
                }
                catch (Exception ex)
                {
                    result.FailedFiles.Add(sourceFile);
                    _logger.LogWarning(ex, "Failed to stage file: {File}", sourceFile);
                }
            }

            result.Success = result.FailedFiles.Count == 0;
            if (!result.Success)
            {
                result.ErrorMessage = $"Failed to stage {result.FailedFiles.Count} files";
            }

            _logger.LogInformation("Staging complete: {Staged} files, {Failed} failed", 
                result.FilesStaged, result.FailedFiles.Count);

            return result;
        }

        public async Task<List<string>> GetNewFilesAsync(string sourcePath, bool recursive = true)
        {
            var newFiles = new List<string>();

            if (!Directory.Exists(sourcePath))
                return newFiles;

            // Generate a device ID from the source path
            var deviceId = GenerateDeviceId(sourcePath);
            var manifest = await LoadBackupManifestAsync(deviceId);

            var searchOption = recursive ? SearchOption.AllDirectories : SearchOption.TopDirectoryOnly;
            var mediaExtensions = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
            {
                ".jpg", ".jpeg", ".png", ".bmp", ".tiff", ".tif", ".webp", ".gif",
                ".heic", ".heif", ".dng", ".raw", ".cr2", ".nef", ".arw", ".orf",
                ".rw2", ".raf", ".3fr", ".fff", ".dcr", ".kdc", ".srf", ".sr2",
                ".erf", ".mef", ".mrw", ".nrw", ".pef", ".iiq", ".x3f", ".bpg",
                ".mp4", ".mov", ".avi", ".mkv", ".webm", ".m4v", ".wmv", ".3gp"
            };

            try
            {
                var allFiles = Directory.EnumerateFiles(sourcePath, "*.*", searchOption)
                    .Where(f => mediaExtensions.Contains(Path.GetExtension(f)));

                foreach (var file in allFiles)
                {
                    var relativePath = GetRelativePath(sourcePath, file);
                    var fileInfo = new FileInfo(file);

                    // Check if file is in manifest
                    if (manifest.BackedUpFiles.TryGetValue(relativePath, out var backedUp))
                    {
                        // File exists in manifest - check if it's changed
                        if (fileInfo.Length == backedUp.FileSize &&
                            Math.Abs((fileInfo.LastWriteTimeUtc - backedUp.LastModified).TotalSeconds) < 2)
                        {
                            // File unchanged, skip
                            continue;
                        }
                    }

                    newFiles.Add(file);
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error scanning for new files in {Path}", sourcePath);
            }

            _logger.LogInformation("Found {Count} new files in {Path}", newFiles.Count, sourcePath);
            return newFiles;
        }

        public async Task<BackupManifest> LoadBackupManifestAsync(string deviceId)
        {
            var manifestPath = GetManifestPath(deviceId);

            if (!File.Exists(manifestPath))
            {
                return new BackupManifest { DeviceId = deviceId };
            }

            try
            {
                var json = await File.ReadAllTextAsync(manifestPath);
                var manifest = JsonSerializer.Deserialize<BackupManifest>(json);
                return manifest ?? new BackupManifest { DeviceId = deviceId };
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Failed to load backup manifest for device {DeviceId}", deviceId);
                return new BackupManifest { DeviceId = deviceId };
            }
        }

        public async Task SaveBackupManifestAsync(string deviceId, BackupManifest manifest)
        {
            var manifestPath = GetManifestPath(deviceId);

            try
            {
                var json = JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true });
                await File.WriteAllTextAsync(manifestPath, json);
                _logger.LogInformation("Saved backup manifest for device {DeviceId}", deviceId);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to save backup manifest for device {DeviceId}", deviceId);
                throw;
            }
        }

        public async Task MarkFilesAsBackedUpAsync(string deviceId, IEnumerable<string> files, string archivePath)
        {
            var manifest = await LoadBackupManifestAsync(deviceId);
            manifest.LastBackupTime = DateTime.UtcNow;
            manifest.LastArchivePath = archivePath;

            foreach (var file in files)
            {
                try
                {
                    var fileInfo = new FileInfo(file);
                    // Try to get relative path from device root
                    var relativePath = file;
                    
                    // For files in staging, use just the filename
                    if (file.StartsWith(_stagingDirectory, StringComparison.OrdinalIgnoreCase))
                    {
                        relativePath = Path.GetFileName(file);
                    }

                    manifest.BackedUpFiles[relativePath] = new BackedUpFileInfo
                    {
                        RelativePath = relativePath,
                        FileSize = fileInfo.Length,
                        LastModified = fileInfo.LastWriteTimeUtc,
                        BackedUpTime = DateTime.UtcNow,
                        ArchivePath = archivePath
                    };
                }
                catch (Exception ex)
                {
                    _logger.LogWarning(ex, "Failed to record backup info for file: {File}", file);
                }
            }

            await SaveBackupManifestAsync(deviceId, manifest);
        }

        public void ClearStagingDirectory()
        {
            try
            {
                if (Directory.Exists(_stagingDirectory))
                {
                    foreach (var file in Directory.GetFiles(_stagingDirectory))
                    {
                        try { File.Delete(file); } catch { }
                    }

                    foreach (var dir in Directory.GetDirectories(_stagingDirectory))
                    {
                        try { Directory.Delete(dir, true); } catch { }
                    }
                }

                _logger.LogInformation("Cleared staging directory");
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to clear staging directory");
            }
        }

        public long GetStagingDirectorySize()
        {
            try
            {
                if (!Directory.Exists(_stagingDirectory))
                    return 0;

                return Directory.GetFiles(_stagingDirectory, "*", SearchOption.AllDirectories)
                    .Sum(f => new FileInfo(f).Length);
            }
            catch
            {
                return 0;
            }
        }

        public List<string> GetStagedFiles()
        {
            try
            {
                if (!Directory.Exists(_stagingDirectory))
                    return new List<string>();

                return Directory.GetFiles(_stagingDirectory, "*", SearchOption.AllDirectories).ToList();
            }
            catch
            {
                return new List<string>();
            }
        }

        private string GetUniqueStagingPath(string fileName)
        {
            var destPath = Path.Combine(_stagingDirectory, fileName);

            if (!File.Exists(destPath))
                return destPath;

            var baseName = Path.GetFileNameWithoutExtension(fileName);
            var extension = Path.GetExtension(fileName);
            var counter = 1;

            while (File.Exists(destPath))
            {
                destPath = Path.Combine(_stagingDirectory, $"{baseName}_{counter}{extension}");
                counter++;
            }

            return destPath;
        }

        private async Task CopyFileAsync(string source, string dest, CancellationToken cancellationToken)
        {
            const int bufferSize = 81920; // 80KB buffer for USB performance

            using var sourceStream = new FileStream(source, FileMode.Open, FileAccess.Read, FileShare.Read, bufferSize, true);
            using var destStream = new FileStream(dest, FileMode.Create, FileAccess.Write, FileShare.None, bufferSize, true);

            await sourceStream.CopyToAsync(destStream, bufferSize, cancellationToken);

            // Preserve original timestamps
            var fileInfo = new FileInfo(source);
            File.SetCreationTimeUtc(dest, fileInfo.CreationTimeUtc);
            File.SetLastWriteTimeUtc(dest, fileInfo.LastWriteTimeUtc);
        }

        private string GetManifestPath(string deviceId)
        {
            // Sanitize device ID for filename
            var safeId = string.Join("_", deviceId.Split(Path.GetInvalidFileNameChars()));
            return Path.Combine(_manifestsDirectory, $"{safeId}.json");
        }

        private string GenerateDeviceId(string path)
        {
            // Use volume label + drive letter as device ID
            try
            {
                var driveLetter = Path.GetPathRoot(path);
                if (!string.IsNullOrEmpty(driveLetter))
                {
                    var driveInfo = new DriveInfo(driveLetter);
                    if (driveInfo.IsReady)
                    {
                        return $"{driveInfo.VolumeLabel}_{driveInfo.Name.TrimEnd('\\')}";
                    }
                }
            }
            catch { }

            // Fallback to path hash
            return $"device_{path.GetHashCode():X8}";
        }

        private string GetRelativePath(string basePath, string fullPath)
        {
            if (!basePath.EndsWith(Path.DirectorySeparatorChar.ToString()))
                basePath += Path.DirectorySeparatorChar;

            if (fullPath.StartsWith(basePath, StringComparison.OrdinalIgnoreCase))
                return fullPath.Substring(basePath.Length);

            return fullPath;
        }
    }
}
