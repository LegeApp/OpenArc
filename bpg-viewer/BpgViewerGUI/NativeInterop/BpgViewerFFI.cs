using System;
using System.Runtime.InteropServices;

namespace BpgViewer.NativeInterop
{
    /// <summary>
    /// FFI bindings to the BPG Viewer Rust library (bpg_viewer.dll)
    /// Matches the C API defined in include/bpg_viewer.h
    /// </summary>
    public static class BpgViewerFFI
    {
        private const string DllName = "bpg_viewer.dll";

        #region Error Codes

        public enum BpgViewerError
        {
            Success = 0,
            InvalidParam = -1,
            DecodeFailed = -2,
            EncodeFailed = -3,
            OutOfMemory = -4,
            IoError = -5
        }

        #endregion

        #region Opaque Handles

        public struct BpgImageHandle
        {
            public IntPtr Handle;
        }

        public struct BpgThumbnailHandle
        {
            public IntPtr Handle;
        }

        #endregion

        #region Image Decoding Functions

        /// <summary>
        /// Decode a BPG file and return a handle to the decoded image
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern IntPtr bpg_viewer_decode_file([MarshalAs(UnmanagedType.LPStr)] string path);

        /// <summary>
        /// Get image dimensions from handle
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int bpg_viewer_get_dimensions(
            IntPtr handle,
            out uint width,
            out uint height);

        /// <summary>
        /// Get image data pointer and size (original format)
        /// The returned pointer is valid as long as the handle exists
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int bpg_viewer_get_data(
            IntPtr handle,
            out IntPtr data,
            out UIntPtr size);

        /// <summary>
        /// Get RGBA32 data from image (performs conversion if needed)
        /// Caller must free the returned pointer with bpg_viewer_free_buffer
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int bpg_viewer_get_rgba32(
            IntPtr handle,
            out IntPtr data,
            out UIntPtr size);

        /// <summary>
        /// Get BGRA32 data from image (for WPF/Windows - no conversion needed)
        /// Caller must free the returned pointer with bpg_viewer_free_buffer
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int bpg_viewer_get_bgra32(
            IntPtr handle,
            out IntPtr data,
            out UIntPtr size);

        /// <summary>
        /// Free buffer allocated by bpg_viewer_get_rgba32 or bpg_viewer_get_bgra32
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void bpg_viewer_free_buffer(IntPtr ptr, UIntPtr size);

        /// <summary>
        /// Free decoded image handle
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void bpg_viewer_free_image(IntPtr handle);

        /// <summary>
        /// Get image color space
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int bpg_viewer_get_color_space(
            IntPtr handle,
            out byte colorSpace);

        /// <summary>
        /// Decode directly to a provided buffer (e.g. WPF WriteableBitmap)
        /// Performs color conversion (source -> sRGB) and format conversion (BGRA)
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int bpg_viewer_decode_to_buffer(
            IntPtr handle,
            IntPtr buffer,
            UIntPtr bufferSize,
            UIntPtr stride);

        /// <summary>
        /// Get EXIF data from image
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int bpg_viewer_get_exif_data(
            IntPtr handle,
            out IntPtr data,
            out UIntPtr size);

        #endregion

        #region Thumbnail Generation Functions

        /// <summary>
        /// Create a thumbnail generator with default settings (256x256)
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr bpg_thumbnail_create();

        /// <summary>
        /// Create a thumbnail generator with specific dimensions
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr bpg_thumbnail_create_with_size(uint maxWidth, uint maxHeight);

        /// <summary>
        /// Generate thumbnail and save as PNG
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int bpg_thumbnail_generate_png(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPStr)] string inputPath,
            [MarshalAs(UnmanagedType.LPStr)] string outputPath);

        /// <summary>
        /// Free thumbnail generator handle
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void bpg_thumbnail_free(IntPtr handle);

        #endregion

        #region Utility Functions

        /// <summary>
        /// Get library version string
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr bpg_viewer_version();

        /// <summary>
        /// Get version as managed string
        /// </summary>
        public static string GetVersion()
        {
            IntPtr versionPtr = bpg_viewer_version();
            return Marshal.PtrToStringAnsi(versionPtr) ?? "unknown";
        }

        #endregion
    }
}
