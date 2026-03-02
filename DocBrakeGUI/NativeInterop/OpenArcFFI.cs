using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace DocBrake.NativeInterop
{
    /// <summary>
    /// FFI interface to Rust OpenArc backend.
    /// Settings match openarc-core OrchestratorSettings and openarc-ffi CompressionSettings.
    /// </summary>
    public static class OpenArcFFI
    {
        private const string DllName = "openarc_ffi.dll";

        [StructLayout(LayoutKind.Sequential)]
        public struct CompressionSettings
        {
            public int BpgQuality;           // 0-51, lower = better quality (CLI default: 25)
            [MarshalAs(UnmanagedType.I1)]
            public bool BpgLossless;         // Enable lossless BPG compression
            public int BpgBitDepth;          // 8-12 bit depth
            public int BpgEncoderType;       // 0=default, 1=slow (JCTVC when available)
            public int BpgCompressionLevel;  // 1-9
            public int VideoCodec;           // 0=H264, 1=H265
            public int VideoSpeed;           // 0=Fast, 1=Medium, 2=Slow
            public int VideoCrf;             // 0-51, lower = better quality (CLI default: 23)
            public int CompressionLevel;     // ArcMax compression level (1-22)
            [MarshalAs(UnmanagedType.I1)]
            public bool EnableCatalog;       // Enable incremental backup tracking
            [MarshalAs(UnmanagedType.I1)]
            public bool EnableDedup;         // Enable file deduplication
            [MarshalAs(UnmanagedType.I1)]
            public bool SkipAlreadyCompressedVideos; // Skip re-encoding already compressed videos
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct ProgressInfo
        {
            public int CurrentFile;
            public int TotalFiles;
            public double ProgressPercent;
            [MarshalAs(UnmanagedType.LPStr)]
            public string CurrentFileName;
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct ArchiveFileInfo
        {
            [MarshalAs(UnmanagedType.LPStr)]
            public string Filename;
            public ulong OriginalSize;
            public ulong CompressedSize;
            public int FileType; // 0=unknown, 1=image, 2=video, 3=document
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct ArchiveRecordInfo
        {
            public long Id;
            [MarshalAs(UnmanagedType.LPStr)]
            public string ArchivePath;
            public ulong ArchiveSize;
            public ulong CreationDate;
            [MarshalAs(UnmanagedType.LPStr)]
            public string OriginalLocation;
            [MarshalAs(UnmanagedType.LPStr)]
            public string DestinationLocation;
            [MarshalAs(UnmanagedType.LPStr)]
            public string Description;
            public uint FileCount;
        }

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        public delegate void ProgressCallback(ProgressInfo progress);

        // Archive creation
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int CreateArchive(
            [MarshalAs(UnmanagedType.LPStr)] string outputPath,
            [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPStr)] string[] inputFiles,
            int fileCount,
            ref CompressionSettings settings,
            ProgressCallback callback);

        // Archive extraction
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int ExtractArchive(
            [MarshalAs(UnmanagedType.LPStr)] string archivePath,
            [MarshalAs(UnmanagedType.LPStr)] string outputDir,
            ProgressCallback callback);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int ExtractArchiveEntry(
            [MarshalAs(UnmanagedType.LPStr)] string archivePath,
            [MarshalAs(UnmanagedType.LPStr)] string entryName,
            [MarshalAs(UnmanagedType.LPStr)] string outputPath);

        // Archive verification (integrity check via HASHES.sha256)
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int VerifyArchive(
            [MarshalAs(UnmanagedType.LPStr)] string archivePath);

        // File type detection
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int DetectFileType([MarshalAs(UnmanagedType.LPStr)] string filePath);

        // Get last error message
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr GetOpenArcError();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern IntPtr PhoneGetStatusJson([MarshalAs(UnmanagedType.LPStr)] string phoneRoot);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int PhoneArchivePendingFiles(
            [MarshalAs(UnmanagedType.LPStr)] string phoneRoot,
            [MarshalAs(UnmanagedType.LPStr)] string outputPath,
            ref CompressionSettings settings,
            ProgressCallback? callback);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void FreeCString(IntPtr ptr);

        // Archive listing
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int ListArchive(
            [MarshalAs(UnmanagedType.LPStr)] string archivePath,
            out int fileCount,
            out IntPtr files);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void FreeArchiveFileList(IntPtr files, int count);

        // Update archive destination
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int UpdateArchiveDestination(
            [MarshalAs(UnmanagedType.LPStr)] string catalogDbPath,
            [MarshalAs(UnmanagedType.LPStr)] string archivePath,
            [MarshalAs(UnmanagedType.LPStr)] string destinationPath);

        // Get all archives
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int GetAllArchives(
            [MarshalAs(UnmanagedType.LPStr)] string catalogDbPath,
            out int archiveCount,
            out IntPtr archives);

        // Free archives array
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void FreeArchivesArray(IntPtr archives, int count);

        // Single file encoding
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int EncodeBpgFile(
            [MarshalAs(UnmanagedType.LPStr)] string inputPath,
            [MarshalAs(UnmanagedType.LPStr)] string outputPath,
            ref CompressionSettings settings);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int EncodeVideoFile(
            [MarshalAs(UnmanagedType.LPStr)] string inputPath,
            [MarshalAs(UnmanagedType.LPStr)] string outputPath,
            ref CompressionSettings settings);

        public static string GetLastErrorMessage()
        {
            IntPtr ptr = GetOpenArcError();
            return ptr != IntPtr.Zero ? Marshal.PtrToStringAnsi(ptr) ?? "Unknown error" : "No error";
        }

        public static string GetPhoneStatusJson(string phoneRoot)
        {
            IntPtr ptr = PhoneGetStatusJson(phoneRoot);
            if (ptr == IntPtr.Zero)
                return string.Empty;

            try
            {
                return Marshal.PtrToStringAnsi(ptr) ?? string.Empty;
            }
            finally
            {
                FreeCString(ptr);
            }
        }

        // ============ MTP (Media Transfer Protocol) Functions ============
        // These provide proper MTP device access via Windows Portable Devices API
        // Much more reliable than Shell COM navigation

        /// <summary>
        /// Lists all connected MTP devices. Returns JSON string with array of devices.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr MtpListDevices();

        /// <summary>
        /// Lists contents of a folder on an MTP device. Returns JSON string with array of objects.
        /// </summary>
        /// <param name="deviceId">Device ID from MtpListDevices</param>
        /// <param name="objectId">Object ID of folder to list, or null/empty for root</param>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        private static extern IntPtr MtpListFolder(
            [MarshalAs(UnmanagedType.LPStr)] string deviceId,
            [MarshalAs(UnmanagedType.LPStr)] string? objectId);

        /// <summary>
        /// Copies a file from MTP device to local filesystem.
        /// </summary>
        /// <param name="deviceId">Device ID from MtpListDevices</param>
        /// <param name="objectId">Object ID of file to copy</param>
        /// <param name="localPath">Local destination path</param>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        private static extern IntPtr MtpCopyFileToLocal(
            [MarshalAs(UnmanagedType.LPStr)] string deviceId,
            [MarshalAs(UnmanagedType.LPStr)] string objectId,
            [MarshalAs(UnmanagedType.LPStr)] string localPath);

        /// <summary>
        /// Reads a file from MTP device directly into memory.
        /// </summary>
        /// <param name="deviceId">Device ID from MtpListDevices</param>
        /// <param name="objectId">Object ID of file to read</param>
        /// <param name="dataSize">Output: size of data read</param>
        /// <returns>Pointer to data buffer (must be freed with MtpFreeData)</returns>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        private static extern IntPtr MtpReadFileToMemory(
            [MarshalAs(UnmanagedType.LPStr)] string deviceId,
            [MarshalAs(UnmanagedType.LPStr)] string objectId,
            out ulong dataSize);

        /// <summary>
        /// Frees string returned by MTP functions.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void MtpFreeString(IntPtr ptr);

        /// <summary>
        /// Frees data buffer returned by MtpReadFileToMemory.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void MtpFreeData(IntPtr ptr, ulong size);

        /// <summary>
        /// Get thumbnail for MTP file - tries WPD native thumbnail first, falls back to full file.
        /// Returns local path to thumbnail/full file that can be used for thumbnail generation.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        private static extern IntPtr MtpGetThumbnail(
            [MarshalAs(UnmanagedType.LPStr)] string deviceId,
            [MarshalAs(UnmanagedType.LPStr)] string objectId,
            [MarshalAs(UnmanagedType.LPStr)] string originalName,
            uint targetWidth,
            uint targetHeight);

        /// <summary>
        /// Cache a file from MTP device to local temp directory.
        /// Returns local path to cached file that C# can use directly.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        private static extern IntPtr MtpCacheFileToTemp(
            [MarshalAs(UnmanagedType.LPStr)] string deviceId,
            [MarshalAs(UnmanagedType.LPStr)] string objectId,
            [MarshalAs(UnmanagedType.LPStr)] string originalName);

        /// <summary>
        /// Get cached local path for an MTP object if already cached.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        private static extern IntPtr MtpGetCachedPath(
            [MarshalAs(UnmanagedType.LPStr)] string deviceId,
            [MarshalAs(UnmanagedType.LPStr)] string objectId);

        /// <summary>
        /// Clear all cached MTP temp files.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr MtpClearCache();

        // ============ MTP Managed Wrappers ============

        /// <summary>
        /// Get list of connected MTP devices (phones, cameras, etc.)
        /// </summary>
        public static MtpResult<List<MtpDeviceInfo>> GetMtpDevices()
        {
            IntPtr ptr = MtpListDevices();
            if (ptr == IntPtr.Zero)
                return MtpResult<List<MtpDeviceInfo>>.Failure("MTP service returned null");

            try
            {
                string json = Marshal.PtrToStringAnsi(ptr) ?? "{}";
                return System.Text.Json.JsonSerializer.Deserialize<MtpResult<List<MtpDeviceInfo>>>(json) 
                    ?? MtpResult<List<MtpDeviceInfo>>.Failure("Failed to deserialize MTP response");
            }
            finally
            {
                MtpFreeString(ptr);
            }
        }

        /// <summary>
        /// List contents of a folder on MTP device
        /// </summary>
        public static MtpResult<List<MtpObjectInfo>> GetMtpFolderContents(string deviceId, string? objectId = null)
        {
            IntPtr ptr = MtpListFolder(deviceId, objectId);
            if (ptr == IntPtr.Zero)
                return MtpResult<List<MtpObjectInfo>>.Failure("MTP service returned null");

            try
            {
                string json = Marshal.PtrToStringAnsi(ptr) ?? "{}";
                return System.Text.Json.JsonSerializer.Deserialize<MtpResult<List<MtpObjectInfo>>>(json)
                    ?? MtpResult<List<MtpObjectInfo>>.Failure("Failed to deserialize MTP response");
            }
            finally
            {
                MtpFreeString(ptr);
            }
        }

        /// <summary>
        /// Copy a file from MTP device to local path
        /// </summary>
        public static MtpResult<string> CopyMtpFileToLocal(string deviceId, string objectId, string localPath)
        {
            IntPtr ptr = MtpCopyFileToLocal(deviceId, objectId, localPath);
            if (ptr == IntPtr.Zero)
                return MtpResult<string>.Failure("MTP service returned null");

            try
            {
                string json = Marshal.PtrToStringAnsi(ptr) ?? "{}";
                return System.Text.Json.JsonSerializer.Deserialize<MtpResult<string>>(json)
                    ?? MtpResult<string>.Failure("Failed to deserialize MTP response");
            }
            finally
            {
                MtpFreeString(ptr);
            }
        }

        /// <summary>
        /// Read a file from MTP device directly into memory
        /// </summary>
        public static MtpResult<byte[]> ReadMtpFileToMemory(string deviceId, string objectId)
        {
            IntPtr ptr = MtpReadFileToMemory(deviceId, objectId, out ulong dataSize);
            if (ptr == IntPtr.Zero)
                return MtpResult<byte[]>.Failure("MTP service returned null or file read failed");

            try
            {
                if (dataSize == 0)
                    return MtpResult<byte[]>.Success(Array.Empty<byte>());

                byte[] data = new byte[dataSize];
                Marshal.Copy(ptr, data, 0, (int)dataSize);
                return MtpResult<byte[]>.Success(data);
            }
            finally
            {
                MtpFreeData(ptr, dataSize);
            }
        }

        /// <summary>
        /// Get thumbnail for MTP file - tries native WPD thumbnail first, falls back to full file.
        /// Automatically handles serialization to prevent race conditions.
        /// </summary>
        public static MtpResult<string> GetMtpThumbnail(string deviceId, string objectId, string originalName, uint width = 256, uint height = 256)
        {
            IntPtr ptr = MtpGetThumbnail(deviceId, objectId, originalName, width, height);
            if (ptr == IntPtr.Zero)
                return MtpResult<string>.Failure("MTP service returned null");

            try
            {
                string json = Marshal.PtrToStringAnsi(ptr) ?? "{}";
                return System.Text.Json.JsonSerializer.Deserialize<MtpResult<string>>(json)
                    ?? MtpResult<string>.Failure("Failed to deserialize MTP response");
            }
            finally
            {
                MtpFreeString(ptr);
            }
        }

        /// <summary>
        /// Cache a file from MTP device to local temp and return the local path.
        /// This is the primary way to access MTP files - C# gets a local path it can use directly.
        /// </summary>
        public static MtpResult<string> CacheFileToTemp(string deviceId, string objectId, string originalName)
        {
            IntPtr ptr = MtpCacheFileToTemp(deviceId, objectId, originalName);
            if (ptr == IntPtr.Zero)
                return MtpResult<string>.Failure("MTP service returned null");

            try
            {
                string json = Marshal.PtrToStringAnsi(ptr) ?? "{}";
                return System.Text.Json.JsonSerializer.Deserialize<MtpResult<string>>(json)
                    ?? MtpResult<string>.Failure("Failed to deserialize MTP response");
            }
            finally
            {
                MtpFreeString(ptr);
            }
        }

        /// <summary>
        /// Get the cached local path for an MTP object if already cached.
        /// Returns empty string if not cached.
        /// </summary>
        public static string? GetCachedPath(string deviceId, string objectId)
        {
            IntPtr ptr = MtpGetCachedPath(deviceId, objectId);
            if (ptr == IntPtr.Zero)
                return null;

            try
            {
                string json = Marshal.PtrToStringAnsi(ptr) ?? "{}";
                var result = System.Text.Json.JsonSerializer.Deserialize<MtpResult<string>>(json);
                return result?.success == true && !string.IsNullOrEmpty(result.data) ? result.data : null;
            }
            finally
            {
                MtpFreeString(ptr);
            }
        }

        /// <summary>
        /// Clear the MTP file cache (removes temp files).
        /// </summary>
        public static void ClearMtpCache()
        {
            IntPtr ptr = MtpClearCache();
            if (ptr != IntPtr.Zero)
                MtpFreeString(ptr);
        }
    }

    // ============ MTP Data Types ============

    /// <summary>
    /// Generic result wrapper for MTP operations
    /// </summary>
    public class MtpResult<T>
    {
        public bool success { get; set; }
        public T? data { get; set; }
        public string? error { get; set; }

        public static MtpResult<T> Success(T data) => new() { success = true, data = data };
        public static MtpResult<T> Failure(string error) => new() { success = false, error = error };
    }

    /// <summary>
    /// Information about an MTP device
    /// </summary>
    public class MtpDeviceInfo
    {
        public string id { get; set; } = string.Empty;
        public string friendly_name { get; set; } = string.Empty;
        public string device_type { get; set; } = "unknown"; // "phone", "camera", "media_player", "unknown"
    }

    /// <summary>
    /// Information about an object (file or folder) on MTP device.
    /// For files, local_path will be populated after CacheFileToTemp is called.
    /// </summary>
    public class MtpObjectInfo
    {
        public string id { get; set; } = string.Empty;
        public string name { get; set; } = string.Empty;
        public string parent_id { get; set; } = string.Empty;
        public bool is_folder { get; set; }
        public ulong size { get; set; }
        /// <summary>
        /// Local temp path if file was cached. Null until CacheFileToTemp is called.
        /// </summary>
        public string? local_path { get; set; }
    }
}
