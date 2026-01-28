using System;
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
            public int BpgChromaFormat;      // 0=420, 1=444, 2=RGB
            public int BpgEncoderType;       // 0=default, 1=slow
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
    }
}
