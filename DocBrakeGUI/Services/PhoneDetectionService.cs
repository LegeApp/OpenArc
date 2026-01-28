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

    public enum MobileDeviceType
    {
        Unknown,
        Phone,
        SDCard,
        Camera,
        USBStorage
    }

    public class PhoneDevice
    {
        public string Name { get; set; } = string.Empty;
        public string Path { get; set; } = string.Empty;
        public string DeviceType { get; set; } = string.Empty;
        public MobileDeviceType DeviceCategory { get; set; } = MobileDeviceType.Unknown;
        public DateTime ConnectedTime { get; set; } = DateTime.Now;
        public ulong TotalSpace { get; set; }
        public ulong FreeSpace { get; set; }
        public bool HasDriveIcon { get; set; }
        public string VolumeLabel { get; set; } = string.Empty;
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
                var rootPath = drive.RootDirectory.FullName;
                var volumeLabel = drive.VolumeLabel ?? string.Empty;
                var totalSizeGB = drive.TotalSize / (1024.0 * 1024.0 * 1024.0);
                var hasDriveIcon = CheckForDriveIcon(rootPath);

                // Detect SD card by multiple heuristics
                var isSDCard = IsSDCard(volumeLabel, totalSizeGB, hasDriveIcon);

                // Check for phone/camera-specific directories
                var phoneIndicators = new[]
                {
                    "DCIM", "Pictures", "Camera", "Movies", "Android", "iOS",
                    "100ANDRO", "100MEDIA", "100APPLE", "100Canon"
                };

                var hasPhoneIndicators = phoneIndicators
                    .Any(indicator => Directory.Exists(Path.Combine(rootPath, indicator)));

                // SD cards and cameras are always considered valid even without explicit phone indicators
                if (!hasPhoneIndicators && !isSDCard)
                    return null;

                // Check for media files if we have indicators
                var hasMediaFiles = false;
                if (hasPhoneIndicators)
                {
                    var mediaExtensions = new[] { ".bpg", ".jpg", ".jpeg", ".png", ".bmp", ".tiff", ".tif", ".webp", ".gif", ".heic", ".heif", ".dng", ".raw", ".cr2", ".nef", ".arw", ".orf", ".rw2", ".raf", ".3fr", ".fff", ".dcr", ".kdc", ".srf", ".sr2", ".erf", ".mef", ".mrw", ".nrw", ".pef", ".iiq", ".x3f", ".jp2", ".j2k", ".j2c", ".jpc", ".jpt", ".jph", ".jhc", ".mp4", ".mov", ".avi", ".mkv", ".webm" };

                    foreach (var indicator in phoneIndicators)
                    {
                        var indicatorPath = Path.Combine(rootPath, indicator);
                        if (Directory.Exists(indicatorPath))
                        {
                            try
                            {
                                hasMediaFiles = Directory.GetFiles(indicatorPath, "*.*", SearchOption.TopDirectoryOnly)
                                    .Any(file => mediaExtensions.Contains(Path.GetExtension(file).ToLower()));
                            }
                            catch { }

                            if (hasMediaFiles)
                                break;
                        }
                    }
                }

                // SD cards are always valid targets
                if (!hasMediaFiles && !isSDCard)
                    return null;

                // Determine device category
                var deviceCategory = DetermineDeviceCategory(volumeLabel, totalSizeGB, hasDriveIcon, hasPhoneIndicators, rootPath);
                var deviceTypeName = GetDeviceTypeName(deviceCategory);

                return new PhoneDevice
                {
                    Name = $"{(string.IsNullOrEmpty(volumeLabel) ? "Removable" : volumeLabel)} ({drive.Name})",
                    Path = rootPath,
                    DeviceType = deviceTypeName,
                    DeviceCategory = deviceCategory,
                    ConnectedTime = DateTime.Now,
                    TotalSpace = (ulong)drive.TotalSize,
                    FreeSpace = (ulong)drive.AvailableFreeSpace,
                    HasDriveIcon = hasDriveIcon,
                    VolumeLabel = volumeLabel
                };
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, $"Error detecting phone on drive {drive.Name}");
                return null;
            }
        }

        /// <summary>
        /// Check if the drive has an embedded icon (autorun.inf with icon, or icon file).
        /// SD cards and cameras often have custom icons; desktop drives and phones typically don't.
        /// </summary>
        private bool CheckForDriveIcon(string rootPath)
        {
            try
            {
                // Check for autorun.inf with icon reference
                var autorunPath = Path.Combine(rootPath, "autorun.inf");
                if (File.Exists(autorunPath))
                {
                    var content = File.ReadAllText(autorunPath);
                    if (content.Contains("icon", StringComparison.OrdinalIgnoreCase))
                        return true;
                }

                // Check for common icon files at root
                var iconFiles = new[] { "icon.ico", "device.ico", "drive.ico", ".VolumeIcon.icns" };
                if (iconFiles.Any(f => File.Exists(Path.Combine(rootPath, f))))
                    return true;
            }
            catch { }

            return false;
        }

        /// <summary>
        /// Detect SD card by SDHC/SDXC label keywords, typical size range, or icon presence.
        /// </summary>
        private bool IsSDCard(string volumeLabel, double totalSizeGB, bool hasDriveIcon)
        {
            // Check volume label for SD card indicators
            var sdLabels = new[] { "SDHC", "SDXC", "SD CARD", "SDCARD", "EOS_DIGITAL", "CANON", "NIKON", "SONY", "LUMIX", "FUJI" };
            if (sdLabels.Any(label => volumeLabel.Contains(label, StringComparison.OrdinalIgnoreCase)))
                return true;

            // SD cards typically range from 2GB to 512GB (common: 16-256GB)
            // Desktop drives are usually 500GB+, phones vary but often show as MTP not drive
            bool typicalSDSize = totalSizeGB >= 2 && totalSizeGB <= 512;

            // If it has a drive icon and is in typical SD size range, likely SD card
            if (hasDriveIcon && typicalSDSize)
                return true;

            return false;
        }

        /// <summary>
        /// Determine the specific device category based on multiple heuristics.
        /// </summary>
        private MobileDeviceType DetermineDeviceCategory(string volumeLabel, double totalSizeGB, bool hasDriveIcon, bool hasPhoneIndicators, string rootPath)
        {
            // Check for Android folder = Phone
            if (Directory.Exists(Path.Combine(rootPath, "Android")))
                return MobileDeviceType.Phone;

            // Camera-specific labels
            var cameraLabels = new[] { "EOS_DIGITAL", "CANON", "NIKON", "SONY", "LUMIX", "FUJI", "OLYMPUS", "PENTAX" };
            if (cameraLabels.Any(label => volumeLabel.Contains(label, StringComparison.OrdinalIgnoreCase)))
                return MobileDeviceType.Camera;

            // SD card indicators
            var sdLabels = new[] { "SDHC", "SDXC", "SD CARD", "SDCARD" };
            if (sdLabels.Any(label => volumeLabel.Contains(label, StringComparison.OrdinalIgnoreCase)))
                return MobileDeviceType.SDCard;

            // Has DCIM but no Android = likely camera or SD card
            if (Directory.Exists(Path.Combine(rootPath, "DCIM")) && !Directory.Exists(Path.Combine(rootPath, "Android")))
            {
                // Small size with icon = SD card, otherwise camera
                if (hasDriveIcon || totalSizeGB <= 256)
                    return MobileDeviceType.SDCard;
                return MobileDeviceType.Camera;
            }

            // Generic removable with phone indicators
            if (hasPhoneIndicators)
                return MobileDeviceType.Phone;

            return MobileDeviceType.USBStorage;
        }

        private string GetDeviceTypeName(MobileDeviceType category)
        {
            return category switch
            {
                MobileDeviceType.Phone => "Phone",
                MobileDeviceType.SDCard => "SD Card",
                MobileDeviceType.Camera => "Camera",
                MobileDeviceType.USBStorage => "USB Storage",
                _ => "Unknown Device"
            };
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
