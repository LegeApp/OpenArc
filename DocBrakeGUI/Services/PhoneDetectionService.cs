using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Microsoft.Extensions.Logging;
using DocBrake.NativeInterop;

namespace DocBrake.Services
{
    public interface IPhoneDetectionService
    {
        event EventHandler<string> PhoneConnected;
        event EventHandler<string> PhoneDisconnected;
        
        List<PhoneDevice> GetConnectedPhones();
        bool StartDetection();
        void StopDetection();
        
        /// <summary>
        /// Get detailed MTP device information if available
        /// </summary>
        MtpDevice? GetMtpDeviceInfo(string devicePath);
        
        /// <summary>
        /// Get storage locations within an MTP device
        /// </summary>
        List<MtpStorageInfo> GetMtpDeviceStorages(MtpDevice device);
    }

    public enum MobileDeviceType
    {
        Unknown,
        Phone,
        SDCard,
        Camera,
        USBStorage,
        MtpDevice  // New: specifically for MTP-connected devices
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
        
        /// <summary>
        /// True if this is an MTP device (not a regular drive)
        /// </summary>
        public bool IsMtpDevice { get; set; }
        
        /// <summary>
        /// The underlying MTP device info (if MTP)
        /// </summary>
        public MtpDevice? MtpDeviceInfo { get; set; }
        
        /// <summary>
        /// Available storages within the device (Internal storage, SD card, etc.)
        /// </summary>
        public List<MtpStorageInfo> Storages { get; set; } = new();
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
            // Now using OpenArcFFI directly for MTP access
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

                // Check MTP devices (media transfer protocol) - THIS IS THE KEY FOR PHONES!
                var mtpDevices = DetectMtpDevices();
                phones.AddRange(mtpDevices);

                // NOTE: Do NOT detect local directories as phones - staging area was being detected
                // var phoneDirs = DetectPhoneDirectories();
                // phones.AddRange(phoneDirs);
                
                _logger.LogInformation("Phone detection found {Count} devices ({MtpCount} MTP)", 
                    phones.Count, mtpDevices.Count);

                // Deduplicate by path (keep first occurrence) to prevent dictionary key collisions
                phones = phones
                    .GroupBy(p => p.Path, StringComparer.OrdinalIgnoreCase)
                    .Select(g => g.First())
                    .ToList();
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to detect connected phones");
            }

            return phones;
        }
        
        public MtpDevice? GetMtpDeviceInfo(string devicePath)
        {
            try
            {
                var result = OpenArcFFI.GetMtpDevices();
                if (!result.success || result.data == null)
                    return null;
                    
                // Find matching device by ID or name
                var mtpDevice = result.data.FirstOrDefault(d => 
                    d.id.Equals(devicePath, StringComparison.OrdinalIgnoreCase) ||
                    d.friendly_name.Equals(devicePath, StringComparison.OrdinalIgnoreCase));
                    
                if (mtpDevice == null)
                    return null;
                    
                // Convert MtpDeviceInfo to MtpDevice
                return new MtpDevice
                {
                    DeviceId = mtpDevice.id,
                    FriendlyName = mtpDevice.friendly_name,
                    DeviceType = mtpDevice.device_type switch
                    {
                        "phone" => MtpDeviceType.Phone,
                        "camera" => MtpDeviceType.Camera,
                        "media_player" => MtpDeviceType.MediaPlayer,
                        _ => MtpDeviceType.Unknown
                    }
                };
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to get MTP device info for {Path}", devicePath);
                return null;
            }
        }
        
        public List<MtpStorageInfo> GetMtpDeviceStorages(MtpDevice device)
        {
            try
            {
                // Browse the device root to get storage locations
                var result = OpenArcFFI.GetMtpFolderContents(device.DeviceId, null);
                if (!result.success || result.data == null)
                    return new List<MtpStorageInfo>();
                    
                // Convert root-level folders to storage info (they represent storages)
                return result.data
                    .Where(o => o.is_folder)
                    .Select(o => new MtpStorageInfo
                    {
                        Name = o.name,
                        Path = $"mtp://{device.DeviceId}/{o.id}",
                        IsInternal = o.name.Contains("Internal", StringComparison.OrdinalIgnoreCase) ||
                                    o.name.Contains("Phone", StringComparison.OrdinalIgnoreCase)
                    })
                    .ToList();
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to get storages for device {Device}", device.FriendlyName);
                return new List<MtpStorageInfo>();
            }
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
                // Use OpenArcFFI directly to enumerate MTP devices
                var result = OpenArcFFI.GetMtpDevices();
                if (!result.success || result.data == null)
                    return phones;
                
                foreach (var mtpDeviceInfo in result.data)
                {
                    // Convert MtpDeviceInfo to MtpDevice
                    var mtpDevice = new MtpDevice
                    {
                        DeviceId = mtpDeviceInfo.id,
                        FriendlyName = mtpDeviceInfo.friendly_name,
                        DeviceType = mtpDeviceInfo.device_type switch
                        {
                            "phone" => MtpDeviceType.Phone,
                            "camera" => MtpDeviceType.Camera,
                            "media_player" => MtpDeviceType.MediaPlayer,
                            _ => MtpDeviceType.Unknown
                        }
                    };
                    
                    // Get storages for this device
                    var storages = GetMtpDeviceStorages(mtpDevice);
                    
                    // Map MTP device type to our MobileDeviceType
                    var deviceCategory = mtpDevice.DeviceType switch
                    {
                        MtpDeviceType.Phone => MobileDeviceType.Phone,
                        MtpDeviceType.Camera => MobileDeviceType.Camera,
                        MtpDeviceType.MediaPlayer => MobileDeviceType.USBStorage,
                        _ => MobileDeviceType.MtpDevice
                    };

                    var phone = new PhoneDevice
                    {
                        Name = mtpDevice.FriendlyName,
                        Path = $"mtp://{mtpDevice.DeviceId}",
                        DeviceType = mtpDevice.DeviceType.ToString(),
                        DeviceCategory = deviceCategory,
                        ConnectedTime = DateTime.Now,
                        IsMtpDevice = true,
                        MtpDeviceInfo = mtpDevice,
                        Storages = storages,
                        TotalSpace = 0, // MTP doesn't easily expose space info
                        FreeSpace = 0,
                        HasDriveIcon = false,
                        VolumeLabel = mtpDevice.FriendlyName
                    };

                    phones.Add(phone);
                    _logger.LogInformation("Detected MTP device: {Name} ({Type}) with {StorageCount} storage(s)", 
                        mtpDevice.FriendlyName, mtpDevice.DeviceType, storages.Count);
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

                // Deduplicate paths (e.g., UserProfile\Pictures\Camera Roll might == MyPictures\Camera Roll)
                var seenPaths = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

                foreach (var path in commonPaths)
                {
                    if (Directory.Exists(path))
                    {
                        // Normalize path to catch duplicates
                        var fullPath = Path.GetFullPath(path);
                        if (!seenPaths.Add(fullPath))
                        {
                            continue; // Skip duplicate
                        }

                        var files = Directory.GetFiles(path, "*.*", SearchOption.TopDirectoryOnly);
                        if (files.Length > 0)
                        {
                            var dirInfo = new DirectoryInfo(path);
                            phones.Add(new PhoneDevice
                            {
                                Name = $"Local Phone Backup ({dirInfo.Name})",
                                Path = fullPath,
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
