using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using DocBrake.NativeInterop;

namespace DocBrake.Services
{
    /// <summary>
    /// Simple MTP file service using Rust backend.
    /// C# never sees MTP paths - only local file paths returned by Rust.
    /// </summary>
    public static class MtpFileService
    {
        // Cache mapping: "deviceId/objectId" -> MtpObjectInfo
        private static readonly Dictionary<string, MtpObjectInfo> _objectCache = new();
        
        // Navigation state: current device and folder being browsed
        private static string? _currentDeviceId;
        private static string? _currentFolderId;
        private static string? _currentDeviceName;
        private static readonly List<(string id, string name)> _breadcrumb = new();

        /// <summary>
        /// Get all connected MTP devices (phones, cameras, etc.)
        /// </summary>
        public static List<MtpDeviceInfo> GetDevices()
        {
            var result = OpenArcFFI.GetMtpDevices();
            if (!result.success || result.data == null)
                return new List<MtpDeviceInfo>();

            return result.data;
        }

        /// <summary>
        /// Start browsing a device - returns root level storages
        /// </summary>
        public static List<MtpItem> BrowseDevice(string deviceId, string deviceName)
        {
            _currentDeviceId = deviceId;
            _currentFolderId = null;
            _currentDeviceName = deviceName;
            _breadcrumb.Clear();
            _breadcrumb.Add((deviceId, deviceName));

            return ListCurrentFolder();
        }

        /// <summary>
        /// Navigate into a folder
        /// </summary>
        public static List<MtpItem> NavigateToFolder(string folderId, string folderName)
        {
            if (_currentDeviceId == null) return new List<MtpItem>();
            
            _currentFolderId = folderId;
            _breadcrumb.Add((folderId, folderName));

            return ListCurrentFolder();
        }

        /// <summary>
        /// Navigate back one level
        /// </summary>
        public static List<MtpItem> NavigateBack()
        {
            if (_breadcrumb.Count <= 1) return new List<MtpItem>();

            _breadcrumb.RemoveAt(_breadcrumb.Count - 1);
            
            if (_breadcrumb.Count == 1)
            {
                // Back to device root
                _currentFolderId = null;
            }
            else
            {
                _currentFolderId = _breadcrumb[^1].id;
            }

            return ListCurrentFolder();
        }

        /// <summary>
        /// Get current navigation breadcrumb
        /// </summary>
        public static IReadOnlyList<(string id, string name)> GetBreadcrumb() => _breadcrumb.AsReadOnly();

        /// <summary>
        /// List contents of current folder
        /// </summary>
        public static List<MtpItem> ListCurrentFolder()
        {
            if (_currentDeviceId == null) return new List<MtpItem>();

            var result = OpenArcFFI.GetMtpFolderContents(_currentDeviceId, _currentFolderId);
            if (!result.success || result.data == null)
                return new List<MtpItem>();

            var items = new List<MtpItem>();
            foreach (var obj in result.data.OrderBy(o => !o.is_folder).ThenBy(o => o.name))
            {
                var key = $"{_currentDeviceId}/{obj.id}";
                _objectCache[key] = obj;

                items.Add(new MtpItem
                {
                    Id = obj.id,
                    Name = obj.name,
                    IsFolder = obj.is_folder,
                    Size = obj.size,
                    DeviceId = _currentDeviceId,
                    // For files, we can get/cache local path on demand
                    LocalPath = obj.local_path
                });
            }

            return items;
        }

        /// <summary>
        /// Get a local file path for an MTP file. 
        /// Copies to temp if not already cached.
        /// This is the main way to access files - C# uses the returned local path.
        /// </summary>
        public static string? GetLocalPath(string deviceId, string objectId, string fileName)
        {
            // Check if already cached
            var cached = OpenArcFFI.GetCachedPath(deviceId, objectId);
            if (!string.IsNullOrEmpty(cached) && File.Exists(cached))
                return cached;

            // Cache to temp
            var result = OpenArcFFI.CacheFileToTemp(deviceId, objectId, fileName);
            return result.success ? result.data : null;
        }

        /// <summary>
        /// Get local path for an MTP item (convenience overload)
        /// </summary>
        public static string? GetLocalPath(MtpItem item)
        {
            if (item.IsFolder) return null;
            return GetLocalPath(item.DeviceId, item.Id, item.Name);
        }

        /// <summary>
        /// Get all files in current folder with their local paths (for thumbnail loading etc.)
        /// Files are cached to temp as needed.
        /// </summary>
        public static IEnumerable<(MtpItem item, string localPath)> GetFilesWithLocalPaths()
        {
            var items = ListCurrentFolder().Where(i => !i.IsFolder);
            
            foreach (var item in items)
            {
                var localPath = GetLocalPath(item);
                if (!string.IsNullOrEmpty(localPath))
                {
                    yield return (item, localPath);
                }
            }
        }

        /// <summary>
        /// Check if a folder has subfolders (for tree expansion)
        /// </summary>
        public static bool HasSubfolders(string deviceId, string? folderId)
        {
            var result = OpenArcFFI.GetMtpFolderContents(deviceId, folderId);
            return result.success && result.data?.Any(o => o.is_folder) == true;
        }

        /// <summary>
        /// Clear the temp file cache
        /// </summary>
        public static void ClearCache()
        {
            OpenArcFFI.ClearMtpCache();
            _objectCache.Clear();
        }

        /// <summary>
        /// Reset navigation state (when disconnecting/switching devices)
        /// </summary>
        public static void ResetNavigation()
        {
            _currentDeviceId = null;
            _currentFolderId = null;
            _currentDeviceName = null;
            _breadcrumb.Clear();
        }

        /// <summary>
        /// Check if path looks like an MTP path (for backward compatibility)
        /// </summary>
        public static bool IsMtpPath(string path)
        {
            return path.StartsWith("mtp://", StringComparison.OrdinalIgnoreCase) ||
                   path.StartsWith("::{", StringComparison.Ordinal) ||
                   (path.StartsWith("Computer\\", StringComparison.OrdinalIgnoreCase) && !path.Contains(":\\"));
        }
    }

    /// <summary>
    /// Simple MTP item (file or folder)
    /// </summary>
    public class MtpItem
    {
        public string Id { get; set; } = string.Empty;
        public string Name { get; set; } = string.Empty;
        public bool IsFolder { get; set; }
        public ulong Size { get; set; }
        public string DeviceId { get; set; } = string.Empty;
        
        /// <summary>
        /// Local temp path if file has been cached. 
        /// Call MtpFileService.GetLocalPath() to ensure it's populated.
        /// </summary>
        public string? LocalPath { get; set; }

        /// <summary>
        /// Get appropriate icon
        /// </summary>
        public string Icon
        {
            get
            {
                if (IsFolder)
                {
                    var lower = Name.ToLowerInvariant();
                    if (lower == "dcim" || lower == "camera") return "📸";
                    if (lower.Contains("internal") || lower == "phone") return "💾";
                    if (lower.Contains("sd") || lower == "card") return "💳";
                    if (lower == "pictures" || lower == "photos") return "🖼️";
                    if (lower == "downloads") return "📥";
                    return "📁";
                }
                
                var ext = Path.GetExtension(Name).ToLowerInvariant();
                return ext switch
                {
                    ".jpg" or ".jpeg" or ".png" or ".gif" or ".bmp" or ".webp" => "🖼️",
                    ".mp4" or ".avi" or ".mkv" or ".mov" => "🎬",
                    ".mp3" or ".wav" or ".flac" or ".aac" => "🎵",
                    ".pdf" => "📄",
                    ".doc" or ".docx" => "📝",
                    _ => "📄"
                };
            }
        }
    }
}
