using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
using Microsoft.Extensions.Logging;

namespace DocBrake.Services
{
    /// <summary>
    /// Service for detecting and accessing MTP (Media Transfer Protocol) devices like phones and cameras.
    /// MTP devices don't appear as drive letters - they appear in the Windows Shell namespace.
    /// </summary>
    public class MtpDeviceService : IDisposable
    {
        private readonly ILogger<MtpDeviceService>? _logger;
        private bool _disposed;

        // Shell32 COM GUIDs
        private static readonly Guid CLSID_ShellApplication = new("13709620-C279-11CE-A49E-444553540000");
        private static readonly Guid IID_IShellDispatch = new("D8F015C0-C278-11CE-A49E-444553540000");

        // Windows Portable Devices GUIDs
        private static readonly Guid CLSID_PortableDeviceManager = new("0af10cec-2ecd-4b92-9581-34f6ae0637f3");
        private static readonly Guid IID_IPortableDeviceManager = new("a1567595-4c2f-4574-a6fa-ecef917b9a40");

        // Shell namespace constants
        private const int CSIDL_DRIVES = 0x0011; // My Computer

        public MtpDeviceService(ILogger<MtpDeviceService>? logger = null)
        {
            _logger = logger;
        }

        /// <summary>
        /// Detects all MTP devices currently connected via Windows Portable Device API
        /// </summary>
        public List<MtpDevice> GetConnectedMtpDevices()
        {
            var devices = new List<MtpDevice>();

            try
            {
                // Method 1: Use Windows Portable Device Manager COM interface
                devices.AddRange(EnumerateViaPortableDeviceManager());

                // Method 2: Scan shell namespace for portable devices (backup approach)
                if (devices.Count == 0)
                {
                    devices.AddRange(EnumerateViaShellNamespace());
                }
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Failed to enumerate MTP devices");
            }

            return devices;
        }

        /// <summary>
        /// Enumerate MTP devices using Windows Portable Device Manager
        /// </summary>
        private List<MtpDevice> EnumerateViaPortableDeviceManager()
        {
            var devices = new List<MtpDevice>();

            try
            {
                // Create the portable device manager
                var deviceManagerType = Type.GetTypeFromCLSID(CLSID_PortableDeviceManager);
                if (deviceManagerType == null)
                {
                    _logger?.LogWarning("PortableDeviceManager COM type not available");
                    return devices;
                }

                var deviceManager = Activator.CreateInstance(deviceManagerType) as IPortableDeviceManager;
                if (deviceManager == null)
                {
                    _logger?.LogWarning("Failed to create PortableDeviceManager instance");
                    return devices;
                }

                try
                {
                    // Get device count
                    uint deviceCount = 0;
                    deviceManager.GetDevices(null, ref deviceCount);

                    if (deviceCount == 0)
                    {
                        _logger?.LogDebug("No portable devices found via WPD");
                        return devices;
                    }

                    // Get device IDs
                    var deviceIds = new string[deviceCount];
                    deviceManager.GetDevices(deviceIds, ref deviceCount);

                    foreach (var deviceId in deviceIds)
                    {
                        if (string.IsNullOrEmpty(deviceId))
                            continue;

                        var device = GetDeviceInfo(deviceManager, deviceId);
                        if (device != null)
                        {
                            devices.Add(device);
                            _logger?.LogInformation("Found MTP device: {Name} at {Path}", device.FriendlyName, device.DeviceId);
                        }
                    }
                }
                finally
                {
                    if (deviceManager != null)
                        Marshal.ReleaseComObject(deviceManager);
                }
            }
            catch (COMException ex)
            {
                _logger?.LogWarning(ex, "COM exception while enumerating portable devices (WPD might not be available)");
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Error enumerating via PortableDeviceManager");
            }

            return devices;
        }

        /// <summary>
        /// Get detailed device information for a specific device ID
        /// </summary>
        private MtpDevice? GetDeviceInfo(IPortableDeviceManager deviceManager, string deviceId)
        {
            try
            {
                // Get friendly name
                uint nameLength = 0;
                deviceManager.GetDeviceFriendlyName(deviceId, null, ref nameLength);
                var nameBuffer = new char[nameLength];
                deviceManager.GetDeviceFriendlyName(deviceId, nameBuffer, ref nameLength);
                var friendlyName = new string(nameBuffer).TrimEnd('\0');

                // Get manufacturer
                uint mfgLength = 0;
                deviceManager.GetDeviceManufacturer(deviceId, null, ref mfgLength);
                var mfgBuffer = new char[mfgLength];
                deviceManager.GetDeviceManufacturer(deviceId, mfgBuffer, ref mfgLength);
                var manufacturer = new string(mfgBuffer).TrimEnd('\0');

                // Get description
                uint descLength = 0;
                deviceManager.GetDeviceDescription(deviceId, null, ref descLength);
                var descBuffer = new char[descLength];
                deviceManager.GetDeviceDescription(deviceId, descBuffer, ref descLength);
                var description = new string(descBuffer).TrimEnd('\0');

                // Build the shell path for this device
                var shellPath = BuildShellPathForDevice(deviceId, friendlyName);

                return new MtpDevice
                {
                    DeviceId = deviceId,
                    FriendlyName = string.IsNullOrEmpty(friendlyName) ? "Unknown Device" : friendlyName,
                    Manufacturer = manufacturer,
                    Description = description,
                    ShellPath = shellPath,
                    DeviceType = DetermineMtpDeviceType(friendlyName, manufacturer, description),
                    ConnectedTime = DateTime.Now
                };
            }
            catch (Exception ex)
            {
                _logger?.LogWarning(ex, "Failed to get info for device {DeviceId}", deviceId);
                return null;
            }
        }

        /// <summary>
        /// Build a shell namespace path for accessing the device
        /// </summary>
        private string BuildShellPathForDevice(string deviceId, string friendlyName)
        {
            // The shell path format for MTP devices is typically:
            // ::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\<device-specific-path>
            // where the GUID is the My Computer shell folder

            // Try to construct a usable path
            // Format: Computer\<FriendlyName>\Internal storage or Computer\<FriendlyName>\Phone
            if (!string.IsNullOrEmpty(friendlyName))
            {
                return $"Computer\\{friendlyName}";
            }

            return deviceId;
        }

        /// <summary>
        /// Determine device type from name/manufacturer info
        /// </summary>
        private MtpDeviceType DetermineMtpDeviceType(string name, string manufacturer, string description)
        {
            var combined = $"{name} {manufacturer} {description}".ToLowerInvariant();

            // Phone indicators
            var phoneKeywords = new[] { "phone", "galaxy", "iphone", "pixel", "android", "samsung", "xiaomi", "huawei", "oneplus", "oppo", "vivo", "motorola", "lg", "sony xperia", "s24", "s23", "s22", "s21", "note" };
            if (phoneKeywords.Any(k => combined.Contains(k)))
                return MtpDeviceType.Phone;

            // Tablet indicators
            var tabletKeywords = new[] { "tablet", "ipad", "tab", "surface" };
            if (tabletKeywords.Any(k => combined.Contains(k)))
                return MtpDeviceType.Tablet;

            // Camera indicators
            var cameraKeywords = new[] { "camera", "canon", "nikon", "sony alpha", "fuji", "olympus", "panasonic lumix", "gopro", "dslr", "mirrorless" };
            if (cameraKeywords.Any(k => combined.Contains(k)))
                return MtpDeviceType.Camera;

            // Media player indicators
            var playerKeywords = new[] { "mp3", "mp4 player", "walkman", "ipod", "zune", "media player" };
            if (playerKeywords.Any(k => combined.Contains(k)))
                return MtpDeviceType.MediaPlayer;

            return MtpDeviceType.Unknown;
        }

        /// <summary>
        /// Fallback: Enumerate devices by scanning the shell namespace for portable devices
        /// </summary>
        private List<MtpDevice> EnumerateViaShellNamespace()
        {
            var devices = new List<MtpDevice>();

            try
            {
                // Use Shell.Application COM object
                var shellType = Type.GetTypeFromProgID("Shell.Application");
                if (shellType == null)
                {
                    _logger?.LogWarning("Shell.Application not available");
                    return devices;
                }

                dynamic? shell = Activator.CreateInstance(shellType);
                if (shell == null)
                    return devices;

                try
                {
                    // Get "My Computer" namespace (CSIDL_DRIVES = 17)
                    dynamic myComputer = shell.Namespace(CSIDL_DRIVES);
                    if (myComputer == null)
                        return devices;

                    // Iterate through items in My Computer
                    foreach (dynamic item in myComputer.Items())
                    {
                        try
                        {
                            string name = item.Name;
                            string path = item.Path;

                            // MTP devices have paths that start with :: (shell namespace)
                            // and contain USB device identifiers
                            if (!string.IsNullOrEmpty(path) && path.StartsWith("::") && 
                                (path.Contains("usb#", StringComparison.OrdinalIgnoreCase) ||
                                 path.Contains("mtp", StringComparison.OrdinalIgnoreCase) ||
                                 path.Contains("vid_", StringComparison.OrdinalIgnoreCase)))
                            {
                                var device = new MtpDevice
                                {
                                    DeviceId = path,
                                    FriendlyName = name,
                                    ShellPath = path,
                                    DeviceType = DetermineMtpDeviceType(name, "", ""),
                                    ConnectedTime = DateTime.Now
                                };

                                devices.Add(device);
                                _logger?.LogInformation("Found MTP device via shell: {Name} at {Path}", name, path);
                            }
                        }
                        catch (Exception ex)
                        {
                            _logger?.LogDebug(ex, "Error processing shell item");
                        }
                    }
                }
                finally
                {
                    if (shell != null)
                        Marshal.ReleaseComObject(shell);
                }
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Error enumerating via Shell namespace");
            }

            return devices;
        }

        /// <summary>
        /// Get the storage folders within an MTP device (e.g., "Internal storage", "SD card")
        /// </summary>
        public List<MtpStorageInfo> GetDeviceStorages(MtpDevice device)
        {
            var storages = new List<MtpStorageInfo>();

            try
            {
                var shellType = Type.GetTypeFromProgID("Shell.Application");
                if (shellType == null)
                    return storages;

                dynamic? shell = Activator.CreateInstance(shellType);
                if (shell == null)
                    return storages;

                try
                {
                    // Navigate to the device in shell namespace
                    dynamic? deviceFolder = shell.Namespace(device.ShellPath);
                    if (deviceFolder == null)
                    {
                        // Try alternate path format
                        dynamic myComputer = shell.Namespace(CSIDL_DRIVES);
                        if (myComputer == null)
                            return storages;

                        foreach (dynamic item in myComputer.Items())
                        {
                            string name = item.Name;
                            if (name.Equals(device.FriendlyName, StringComparison.OrdinalIgnoreCase))
                            {
                                deviceFolder = item.GetFolder;
                                break;
                            }
                        }
                    }

                    if (deviceFolder == null)
                        return storages;

                    // Get storage folders (Internal storage, SD card, etc.)
                    foreach (dynamic storage in deviceFolder.Items())
                    {
                        try
                        {
                            string storageName = storage.Name;
                            string storagePath = storage.Path;

                            storages.Add(new MtpStorageInfo
                            {
                                Name = storageName,
                                Path = storagePath,
                                IsInternal = storageName.Contains("Internal", StringComparison.OrdinalIgnoreCase) ||
                                            storageName.Contains("Phone", StringComparison.OrdinalIgnoreCase)
                            });

                            _logger?.LogDebug("Found storage: {Name} at {Path}", storageName, storagePath);
                        }
                        catch { }
                    }
                }
                finally
                {
                    if (shell != null)
                        Marshal.ReleaseComObject(shell);
                }
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Error getting device storages for {Device}", device.FriendlyName);
            }

            return storages;
        }

        /// <summary>
        /// Enumerate files in an MTP folder path (e.g., DCIM/Camera)
        /// </summary>
        public List<MtpFileInfo> EnumerateFiles(string folderPath, bool recursive = false, string[]? extensions = null)
        {
            var files = new List<MtpFileInfo>();

            try
            {
                var shellType = Type.GetTypeFromProgID("Shell.Application");
                if (shellType == null)
                    return files;

                dynamic? shell = Activator.CreateInstance(shellType);
                if (shell == null)
                    return files;

                try
                {
                    dynamic? folder = shell.Namespace(folderPath);
                    if (folder == null)
                        return files;

                    EnumerateFolderItems(folder, files, recursive, extensions);
                }
                finally
                {
                    if (shell != null)
                        Marshal.ReleaseComObject(shell);
                }
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Error enumerating files in {Path}", folderPath);
            }

            return files;
        }

        private void EnumerateFolderItems(dynamic folder, List<MtpFileInfo> files, bool recursive, string[]? extensions)
        {
            try
            {
                foreach (dynamic item in folder.Items())
                {
                    try
                    {
                        bool isFolder = item.IsFolder;
                        string name = item.Name;
                        string path = item.Path;

                        if (isFolder)
                        {
                            if (recursive)
                            {
                                dynamic? subFolder = item.GetFolder;
                                if (subFolder != null)
                                {
                                    EnumerateFolderItems(subFolder, files, recursive, extensions);
                                }
                            }
                        }
                        else
                        {
                            // Check extension filter
                            if (extensions != null && extensions.Length > 0)
                            {
                                var ext = Path.GetExtension(name)?.ToLowerInvariant() ?? "";
                                if (!extensions.Contains(ext))
                                    continue;
                            }

                            // Get file size (property index 1 = Size)
                            long size = 0;
                            try
                            {
                                dynamic parent = folder.Self;
                                string sizeStr = parent.GetDetailsOf(item, 1);
                                if (!string.IsNullOrEmpty(sizeStr))
                                {
                                    sizeStr = new string(sizeStr.Where(c => char.IsDigit(c)).ToArray());
                                    long.TryParse(sizeStr, out size);
                                }
                            }
                            catch { }

                            // Get date modified (property index 3)
                            DateTime modified = DateTime.Now;
                            try
                            {
                                dynamic parent = folder.Self;
                                string dateStr = parent.GetDetailsOf(item, 3);
                                DateTime.TryParse(dateStr, out modified);
                            }
                            catch { }

                            files.Add(new MtpFileInfo
                            {
                                Name = name,
                                Path = path,
                                Size = size,
                                DateModified = modified,
                                Extension = Path.GetExtension(name)?.ToLowerInvariant() ?? ""
                            });
                        }
                    }
                    catch { }
                }
            }
            catch (Exception ex)
            {
                _logger?.LogDebug(ex, "Error iterating folder items");
            }
        }

        /// <summary>
        /// Copy a file from MTP device to local filesystem
        /// </summary>
        public bool CopyFileToLocal(string mtpFilePath, string localDestination)
        {
            try
            {
                var shellType = Type.GetTypeFromProgID("Shell.Application");
                if (shellType == null)
                    return false;

                dynamic? shell = Activator.CreateInstance(shellType);
                if (shell == null)
                    return false;

                try
                {
                    // Get the parent folder of the source file
                    string sourceDir = Path.GetDirectoryName(mtpFilePath) ?? mtpFilePath;
                    string fileName = Path.GetFileName(mtpFilePath);

                    dynamic? sourceFolder = shell.Namespace(sourceDir);
                    if (sourceFolder == null)
                        return false;

                    // Find the file item
                    dynamic? fileItem = null;
                    foreach (dynamic item in sourceFolder.Items())
                    {
                        if (item.Name.Equals(fileName, StringComparison.OrdinalIgnoreCase))
                        {
                            fileItem = item;
                            break;
                        }
                    }

                    if (fileItem == null)
                        return false;

                    // Get destination folder
                    string destDir = Path.GetDirectoryName(localDestination) ?? localDestination;
                    Directory.CreateDirectory(destDir);

                    dynamic? destFolder = shell.Namespace(destDir);
                    if (destFolder == null)
                        return false;

                    // Copy the file (16 = respond "Yes to All")
                    destFolder.CopyHere(fileItem, 16);

                    // Rename if needed
                    string destFileName = Path.GetFileName(localDestination);
                    if (!fileName.Equals(destFileName, StringComparison.OrdinalIgnoreCase))
                    {
                        string copiedPath = Path.Combine(destDir, fileName);
                        if (File.Exists(copiedPath))
                        {
                            File.Move(copiedPath, localDestination, true);
                        }
                    }

                    return File.Exists(localDestination);
                }
                finally
                {
                    if (shell != null)
                        Marshal.ReleaseComObject(shell);
                }
            }
            catch (Exception ex)
            {
                _logger?.LogError(ex, "Error copying file from MTP: {Source} to {Dest}", mtpFilePath, localDestination);
                return false;
            }
        }

        public void Dispose()
        {
            if (!_disposed)
            {
                _disposed = true;
            }
        }
    }

    /// <summary>
    /// Information about a detected MTP device
    /// </summary>
    public class MtpDevice
    {
        public string DeviceId { get; set; } = string.Empty;
        public string FriendlyName { get; set; } = string.Empty;
        public string Manufacturer { get; set; } = string.Empty;
        public string Description { get; set; } = string.Empty;
        public string ShellPath { get; set; } = string.Empty;
        public MtpDeviceType DeviceType { get; set; } = MtpDeviceType.Unknown;
        public DateTime ConnectedTime { get; set; } = DateTime.Now;
    }

    /// <summary>
    /// Type of MTP device
    /// </summary>
    public enum MtpDeviceType
    {
        Unknown,
        Phone,
        Tablet,
        Camera,
        MediaPlayer
    }

    /// <summary>
    /// Storage information within an MTP device
    /// </summary>
    public class MtpStorageInfo
    {
        public string Name { get; set; } = string.Empty;
        public string Path { get; set; } = string.Empty;
        public bool IsInternal { get; set; }
    }

    /// <summary>
    /// File information from MTP device
    /// </summary>
    public class MtpFileInfo
    {
        public string Name { get; set; } = string.Empty;
        public string Path { get; set; } = string.Empty;
        public long Size { get; set; }
        public DateTime DateModified { get; set; }
        public string Extension { get; set; } = string.Empty;
    }

    /// <summary>
    /// COM interface for Windows Portable Device Manager
    /// </summary>
    [ComImport]
    [Guid("a1567595-4c2f-4574-a6fa-ecef917b9a40")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IPortableDeviceManager
    {
        void RefreshDeviceList();

        void GetDevices(
            [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPWStr, SizeParamIndex = 1)] string[]? pPnPDeviceIDs,
            ref uint pcPnPDeviceIDs);

        void GetDeviceFriendlyName(
            [MarshalAs(UnmanagedType.LPWStr)] string pszPnPDeviceID,
            [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 2)] char[]? pDeviceFriendlyName,
            ref uint pcchDeviceFriendlyName);

        void GetDeviceDescription(
            [MarshalAs(UnmanagedType.LPWStr)] string pszPnPDeviceID,
            [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 2)] char[]? pDeviceDescription,
            ref uint pcchDeviceDescription);

        void GetDeviceManufacturer(
            [MarshalAs(UnmanagedType.LPWStr)] string pszPnPDeviceID,
            [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 2)] char[]? pDeviceManufacturer,
            ref uint pcchDeviceManufacturer);

        void GetDeviceProperty(
            [MarshalAs(UnmanagedType.LPWStr)] string pszPnPDeviceID,
            [MarshalAs(UnmanagedType.LPWStr)] string pszDevicePropertyName,
            [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 3)] byte[]? pData,
            ref uint pcbData,
            ref uint pdwType);

        void GetPrivateDevices(
            [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPWStr, SizeParamIndex = 1)] string[]? pPnPDeviceIDs,
            ref uint pcPnPDeviceIDs);
    }
}
