using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Microsoft.Extensions.Logging;

namespace DocBrake.Services
{
    public interface IPhoneDetectionService
    {
        event EventHandler<string> PhoneConnected;
        event EventHandler<string> PhoneDisconnected;
        
        List<PhoneDevice> GetConnectedPhones();
        bool StartDetection();
        void StopDetection();
    }

    public class PhoneDevice
    {
        public string Name { get; set; } = string.Empty;
        public string Path { get; set; } = string.Empty;
        public string DeviceType { get; set; } = string.Empty;
        public DateTime ConnectedTime { get; set; } = DateTime.Now;
        public ulong TotalSpace { get; set; }
        public ulong FreeSpace { get; set; }
    }

    public class PhoneDetectionService : IPhoneDetectionService
    {
        private readonly ILogger<PhoneDetectionService> _logger;
        private bool _isDetecting;
        private System.Timers.Timer? _detectionTimer;
        private HashSet<string> _previousDevices = new();

        public event EventHandler<string>? PhoneConnected;
        public event EventHandler<string>? PhoneDisconnected;

        public PhoneDetectionService(ILogger<PhoneDetectionService> logger)
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
        }

        public List<PhoneDevice> GetConnectedPhones()
        {
            var phones = new List<PhoneDevice>();

            try
            {
                // Check common phone mount points on Windows
                var drives = DriveInfo.GetDrives()
                    .Where(d => d.DriveType == DriveType.Removable && d.IsReady)
                    .ToList();

                foreach (var drive in drives)
                {
                    var phone = DetectPhoneOnDrive(drive);
                    if (phone != null)
                    {
                        phones.Add(phone);
                    }
                }

                // Check MTP devices (media transfer protocol)
                var mtpDevices = DetectMtpDevices();
                phones.AddRange(mtpDevices);

                // Check common phone directories
                var phoneDirs = DetectPhoneDirectories();
                phones.AddRange(phoneDirs);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to detect connected phones");
            }

            return phones;
        }

        public bool StartDetection()
        {
            if (_isDetecting)
                return false;

            _isDetecting = true;
            _detectionTimer = new System.Timers.Timer(5000); // Check every 5 seconds
            _detectionTimer.Elapsed += OnDetectionTimer;
            _detectionTimer.Start();

            _logger.LogInformation("Phone detection started");
            return true;
        }

        public void StopDetection()
        {
            if (!_isDetecting)
                return;

            _isDetecting = false;
            _detectionTimer?.Stop();
            _detectionTimer?.Dispose();
            _detectionTimer = null;

            _logger.LogInformation("Phone detection stopped");
        }

        private void OnDetectionTimer(object? sender, System.Timers.ElapsedEventArgs e)
        {
            try
            {
                var currentPhones = GetConnectedPhones();
                var currentPaths = new HashSet<string>(currentPhones.Select(p => p.Path));

                // Check for newly connected phones
                foreach (var phone in currentPhones)
                {
                    if (!_previousDevices.Contains(phone.Path))
                    {
                        PhoneConnected?.Invoke(this, phone.Name);
                        _logger.LogInformation($"Phone connected: {phone.Name} at {phone.Path}");
                    }
                }

                // Check for disconnected phones
                foreach (var path in _previousDevices)
                {
                    if (!currentPaths.Contains(path))
                    {
                        PhoneDisconnected?.Invoke(this, path);
                        _logger.LogInformation($"Phone disconnected: {path}");
                    }
                }

                _previousDevices = currentPaths;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error during phone detection");
            }
        }

        private PhoneDevice? DetectPhoneOnDrive(DriveInfo drive)
        {
            try
            {
                // Check for phone-specific directories and files
                var rootPath = drive.RootDirectory.FullName;
                var phoneIndicators = new[]
                {
                    "DCIM", "Pictures", "Camera", "Movies", "Android", "iOS",
                    "100ANDRO", "100MEDIA", "100APPLE", "100Canon"
                };

                var hasPhoneIndicators = phoneIndicators
                    .Any(indicator => Directory.Exists(Path.Combine(rootPath, indicator)));

                if (!hasPhoneIndicators)
                    return null;

                // Check for media files
                var mediaExtensions = new[] { ".bpg", ".jpg", ".jpeg", ".png", ".bmp", ".tiff", ".tif", ".webp", ".gif", ".heic", ".heif", ".dng", ".raw", ".cr2", ".nef", ".arw", ".orf", ".rw2", ".raf", ".3fr", ".fff", ".dcr", ".kdc", ".srf", ".sr2", ".erf", ".mef", ".mrw", ".nrw", ".pef", ".iiq", ".x3f", ".jp2", ".j2k", ".j2c", ".jpc", ".jpt", ".jph", ".jhc", ".mp4", ".mov", ".avi", ".mkv", ".webm" };
                var hasMediaFiles = false;

                foreach (var indicator in phoneIndicators)
                {
                    var indicatorPath = Path.Combine(rootPath, indicator);
                    if (Directory.Exists(indicatorPath))
                    {
                        hasMediaFiles = Directory.GetFiles(indicatorPath, "*.*", SearchOption.TopDirectoryOnly)
                            .Any(file => mediaExtensions.Contains(Path.GetExtension(file).ToLower()));
                        
                        if (hasMediaFiles)
                            break;
                    }
                }

                if (!hasMediaFiles)
                    return null;

                return new PhoneDevice
                {
                    Name = $"{drive.VolumeLabel} ({drive.Name})",
                    Path = rootPath,
                    DeviceType = "USB Storage",
                    ConnectedTime = DateTime.Now,
                    TotalSpace = (ulong)drive.TotalSize,
                    FreeSpace = (ulong)drive.AvailableFreeSpace
                };
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error detecting phone on drive {drive.Name}");
                return null;
            }
        }

        private List<PhoneDevice> DetectMtpDevices()
        {
            var phones = new List<PhoneDevice>();

            try
            {
                // Check for MTP devices in Windows shell
                // This is a simplified implementation
                var mtpPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyComputer), "Portable Devices");
                if (Directory.Exists(mtpPath))
                {
                    // In a full implementation, this would use Windows Shell APIs
                    // For now, we'll just check common MTP mount points
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error detecting MTP devices");
            }

            return phones;
        }

        private List<PhoneDevice> DetectPhoneDirectories()
        {
            var phones = new List<PhoneDevice>();

            try
            {
                // Check common phone backup directories
                var commonPaths = new[]
                {
                    Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Pictures", "Camera Roll"),
                    Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Pictures", "Phone"),
                    Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyPictures), "Camera Roll"),
                };

                foreach (var path in commonPaths)
                {
                    if (Directory.Exists(path))
                    {
                        var files = Directory.GetFiles(path, "*.*", SearchOption.TopDirectoryOnly);
                        if (files.Length > 0)
                        {
                            var dirInfo = new DirectoryInfo(path);
                            phones.Add(new PhoneDevice
                            {
                                Name = $"Local Phone Backup ({dirInfo.Name})",
                                Path = path,
                                DeviceType = "Local Directory",
                                ConnectedTime = DateTime.Now,
                                TotalSpace = 0,
                                FreeSpace = 0
                            });
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error detecting phone directories");
            }

            return phones;
        }
    }
}
