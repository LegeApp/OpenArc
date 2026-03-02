using System;
using System.Runtime.InteropServices;

namespace DocBrake.MediaBrowser.NativeInterop
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

        public struct UniversalThumbnailHandle
        {
            public IntPtr Handle;
        }

        public struct UniversalImageHandle
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

        #region Universal Thumbnail Generation Functions

        /// <summary>
        /// Create a universal thumbnail generator with default settings (256x256)
        /// Supports all image formats: BPG, JPEG, PNG, TIFF, HEIC, RAW, DNG, etc.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr universal_thumbnail_create();

        /// <summary>
        /// Create a universal thumbnail generator with specific dimensions
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr universal_thumbnail_create_with_size(uint maxWidth, uint maxHeight);

        /// <summary>
        /// Generate thumbnail for any supported image format and save as PNG
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int universal_thumbnail_generate_png(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPStr)] string inputPath,
            [MarshalAs(UnmanagedType.LPStr)] string outputPath);

        /// <summary>
        /// Generate thumbnail for any supported image format and save as JPEG
        /// Quality: 1-100 (85 is recommended for thumbnails, ~3-5x smaller than PNG for photos)
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int universal_thumbnail_generate_jpeg(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPStr)] string inputPath,
            [MarshalAs(UnmanagedType.LPStr)] string outputPath,
            uint quality);

        /// <summary>
        /// Check if a file format is supported by the universal thumbnail generator
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int universal_thumbnail_is_supported([MarshalAs(UnmanagedType.LPStr)] string filePath);

        /// <summary>
        /// Free universal thumbnail generator handle
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void universal_thumbnail_free(IntPtr handle);

        #endregion

        #region Universal Image Decode Functions (Full Resolution BGRA)

        /// <summary>
        /// Decode any supported image file to full resolution BGRA
        /// Supports BPG, HEIC, RAW, DNG, JPEG2000, and standard image formats
        /// Returns IntPtr.Zero on failure
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern IntPtr universal_image_decode_file([MarshalAs(UnmanagedType.LPStr)] string path);

        /// <summary>
        /// Get image dimensions from universal image handle
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int universal_image_get_dimensions(
            IntPtr handle,
            out uint width,
            out uint height);

        /// <summary>
        /// Copy BGRA data to a provided buffer (e.g. WPF WriteableBitmap)
        /// Buffer must be at least stride * height bytes
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int universal_image_copy_to_buffer(
            IntPtr handle,
            IntPtr buffer,
            UIntPtr bufferSize,
            UIntPtr stride);

        /// <summary>
        /// Get BGRA data pointer and size from universal image handle
        /// The returned pointer is valid as long as the handle exists
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int universal_image_get_data(
            IntPtr handle,
            out IntPtr data,
            out UIntPtr size);

        /// <summary>
        /// Check if a file format is supported by the universal image decoder
        /// Returns 1 if supported, 0 otherwise
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int universal_image_is_supported([MarshalAs(UnmanagedType.LPStr)] string filePath);

        /// <summary>
        /// Free universal image handle
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void universal_image_free(IntPtr handle);

        #endregion

        #region GPU Thumbnail Pipeline (D3D12 YCbCr → RGB Resize)

        /// <summary>
        /// Initialize the GPU thumbnail pipeline (D3D12 multi-queue with atlas)
        /// Returns 0 on success, non-zero on failure
        /// Must be called once before using GPU thumbnail functions
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int gpu_thumbnail_pipeline_init();

        /// <summary>
        /// Process a JPEG file through the GPU pipeline (YCbCr → 256×256 resize)
        /// Returns 0 on success and outputs tile coordinates in the 4096×4096 atlas
        /// source_id: Unique ID for tracking this thumbnail operation (use GetHashCode or similar)
        /// path: Path to JPEG file to process
        /// tile_x, tile_y: Output coordinates of the 256×256 tile in the atlas
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int gpu_thumbnail_process_jpeg(
            ulong source_id,
            [MarshalAs(UnmanagedType.LPStr)] string path,
            out uint tile_x,
            out uint tile_y);

        /// <summary>
        /// Process any supported image file through the GPU thumbnail pipeline.
        /// Decodes with universal decoder, converts to YCbCr, resizes on GPU.
        /// For JPEG files, uses the fast native YCbCr path internally.
        /// For HEIC, PNG, TIFF, RAW, DNG, etc: decodes to RGB, converts to YCbCr, GPU resizes.
        /// Returns 0 on success, -1 on GPU failure, -2 on decode failure.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int gpu_thumbnail_process_file(
            ulong source_id,
            [MarshalAs(UnmanagedType.LPStr)] string path,
            out uint tile_x,
            out uint tile_y);

        /// <summary>
        /// Readback and encode a thumbnail tile to JPEG file
        /// tile_x, tile_y: Coordinates returned from gpu_thumbnail_process_jpeg
        /// output_path: Destination path for JPEG file
        /// quality: JPEG quality (1-100, typical: 85)
        /// Returns 0 on success, non-zero on failure
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int gpu_thumbnail_readback_jpeg(
            uint tile_x,
            uint tile_y,
            [MarshalAs(UnmanagedType.LPStr)] string output_path,
            uint quality);

        /// <summary>
        /// OPTIMIZED: All-in-one GPU thumbnail generation with automatic tile recycling.
        /// This is the RECOMMENDED API that combines decode + GPU process + readback + encode + TILE RELEASE.
        /// 
        /// Unlike the legacy two-call API (process + readback), this function:
        /// - Eliminates CPU pre-downscaling for typical photos (4000×3000 = 17.2 MB → no downscale)
        /// - Releases tiles immediately after encoding (removes 256-image ceiling)
        /// - Uses lock splitting for better concurrency (other threads work during JPEG encode)
        /// 
        /// Parameters:
        /// - source_id: Unique ID for this thumbnail (for atlas tracking)
        /// - input_path: Path to source image (JPEG, PNG, HEIC, etc.)
        /// - output_path: Path to save 256×256 JPEG thumbnail
        /// - quality: JPEG quality 1-100 (85 recommended)
        /// - is_jpeg: true if input is JPEG (enables fast YCbCr decode path)
        /// 
        /// Returns:
        /// - 0: Success
        /// - -1: GPU processing failed
        /// - -2: Decode/file error
        /// - -3: JPEG encoding failed
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int gpu_generate_thumbnail_optimized(
            ulong source_id,
            [MarshalAs(UnmanagedType.LPStr)] string input_path,
            [MarshalAs(UnmanagedType.LPStr)] string output_path,
            uint quality,
            [MarshalAs(UnmanagedType.I1)] bool is_jpeg);

        #endregion

        #region Async Thumbnail and Full-Image Loading (Phase 3)

        /// <summary>
        /// Callback signature for async full-image loading
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        public delegate void FullImageCallback(
            ulong userData,
            IntPtr dataPtr,
            uint width,
            uint height,
            UIntPtr stride,
            IntPtr error);

        /// <summary>
        /// Load full-resolution image asynchronously on the full-image thread pool
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern void fullimage_load_async(
            [MarshalAs(UnmanagedType.LPStr)] string path,
            FullImageCallback callback,
            ulong userData);

        /// <summary>
        /// Callback signature for async thumbnail generation
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        public delegate void ThumbnailCallback(ulong sourceId, int result, IntPtr error);

        /// <summary>
        /// Generate thumbnail asynchronously on the thumbnail thread pool (legacy)
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern void thumbnail_generate_async(
            ulong sourceId,
            [MarshalAs(UnmanagedType.LPStr)] string inputPath,
            [MarshalAs(UnmanagedType.LPStr)] string outputPath,
            byte quality,
            ThumbnailCallback callback);

        /// <summary>
        /// Generate thumbnail using Tokio async pipeline (RECOMMENDED)
        ///
        /// This is the new unified Tokio-based async API that replaces all previous
        /// thumbnail APIs. Uses CPU backend with fast_image_resize for best compatibility.
        ///
        /// Benefits:
        /// - No COM threading conflicts with WPF
        /// - Semaphore-based backpressure (max 8 concurrent)
        /// - Clean architecture with spawn_blocking for CPU work
        /// - Single global Tokio runtime
        ///
        /// Returns:
        /// - 0: Request accepted and queued
        /// - -1: Invalid arguments (null pointer or bad UTF-8)
        /// - -2: Pipeline initialization failed
        ///
        /// Callback receives:
        /// - result = 0: Success
        /// - result = -1: Generation failed (error string provided)
        /// - error: Error message (null on success, must free with bpg_error_free)
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        public static extern int bpg_generate_thumbnail_async(
            ulong sourceId,
            [MarshalAs(UnmanagedType.LPStr)] string inputPath,
            [MarshalAs(UnmanagedType.LPStr)] string outputPath,
            byte quality,
            ThumbnailCallback callback);

        /// <summary>
        /// Free error string allocated by pipeline callbacks
        /// Call this on any non-null error pointer received in callbacks.
        /// Safe to call with null pointer (no-op).
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void bpg_error_free(IntPtr errorPtr);

        /// <summary>
        /// Check if the pipeline is initialized (for testing/debugging)
        /// Returns 1 if initialized, 0 if not yet initialized.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int bpg_pipeline_is_initialized();

        /// <summary>
        /// Shutdown the pipeline and cleanup resources
        /// Should be called on process exit. After calling this,
        /// no more thumbnail requests can be processed.
        /// Returns 0 on success, -1 if already shut down.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int bpg_pipeline_shutdown();

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
