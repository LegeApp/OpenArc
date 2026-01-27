using System;
using System.IO;
using System.Linq;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using DocBrake.MediaBrowser.NativeInterop;
using MetadataExtractor;

namespace DocBrake.MediaBrowser.Models
{
    /// <summary>
    /// Represents a decoded media image with managed resources
    /// (Currently supports BPG, extensible for future formats)
    /// </summary>
    public class MediaItem : IDisposable
    {
        private IntPtr _handle;
        private bool _disposed;

        public uint Width { get; private set; }
        public uint Height { get; private set; }
        public long FileSizeBytes { get; private set; }
        public string ColorSpace { get; private set; } = "Unknown";
        public string DateTaken { get; private set; } = "Unknown";
        public string CameraModel { get; private set; } = "Unknown Camera";
        public string LensModel { get; private set; } = "Unknown Lens";
        public WriteableBitmap? Bitmap { get; private set; }
        public string FileType { get; private set; } = "BPG"; // Extensible for future formats

        private MediaItem(IntPtr handle, uint width, uint height, long fileSizeBytes)
        {
            _handle = handle;
            Width = width;
            Height = height;
            FileSizeBytes = fileSizeBytes;

            // Get color space
            if (BpgViewerFFI.bpg_viewer_get_color_space(handle, out byte cs) == 0)
            {
                ColorSpace = GetColorSpaceName(cs);
            }

            // Get EXIF metadata
            ExtractMetadata(handle);
        }

        private MediaItem(WriteableBitmap bitmap, long fileSizeBytes, string fileType)
        {
            _handle = IntPtr.Zero;
            Bitmap = bitmap;
            Width = (uint)bitmap.PixelWidth;
            Height = (uint)bitmap.PixelHeight;
            FileSizeBytes = fileSizeBytes;
            FileType = fileType;
            ColorSpace = "RGB";
        }

        private unsafe void ExtractMetadata(IntPtr handle)
        {
            try
            {
                IntPtr dataPtr;
                UIntPtr size;

                if (BpgViewerFFI.bpg_viewer_get_exif_data(handle, out dataPtr, out size) == 0 && size.ToUInt64() > 0)
                {
                    byte[] exifBytes = new byte[size.ToUInt64()];
                    System.Runtime.InteropServices.Marshal.Copy(dataPtr, exifBytes, 0, exifBytes.Length);

                    using var stream = new System.IO.MemoryStream(exifBytes);
                    var directories = MetadataExtractor.ImageMetadataReader.ReadMetadata(stream);

                    // Extract Camera Model
                    var ifd0 = directories.OfType<MetadataExtractor.Formats.Exif.ExifIfd0Directory>().FirstOrDefault();
                    if (ifd0 != null)
                    {
                        var model = ifd0.GetDescription(MetadataExtractor.Formats.Exif.ExifIfd0Directory.TagModel);
                        if (!string.IsNullOrEmpty(model))
                            CameraModel = model;
                    }

                    // Extract Lens Model
                    var subIfd = directories.OfType<MetadataExtractor.Formats.Exif.ExifSubIfdDirectory>().FirstOrDefault();
                    if (subIfd != null)
                    {
                        var dateTaken = subIfd.GetDescription(MetadataExtractor.Formats.Exif.ExifSubIfdDirectory.TagDateTimeOriginal);
                        if (string.IsNullOrEmpty(dateTaken))
                            dateTaken = subIfd.GetDescription(MetadataExtractor.Formats.Exif.ExifSubIfdDirectory.TagDateTimeDigitized);
                        if (!string.IsNullOrEmpty(dateTaken))
                            DateTaken = dateTaken;

                        var lens = subIfd.GetDescription(MetadataExtractor.Formats.Exif.ExifSubIfdDirectory.TagLensModel);
                        if (!string.IsNullOrEmpty(lens))
                            LensModel = lens;
                    }
                }
            }
            catch
            {
                // Ignore metadata errors
            }
        }

        private string GetColorSpaceName(byte cs)
        {
            // BPGColorSpaceEnum from libbpg
            return cs switch
            {
                0 => "YCbCr (BT.601)",
                1 => "RGB",
                2 => "YCbCr (BT.709)",
                3 => "YCbCr (BT.2020)",
                4 => "YCbCr (BT.2020, Constant)",
                _ => $"Unknown ({cs})"
            };
        }

        /// <summary>
        /// Load a BPG file from disk
        /// </summary>
        public static MediaItem? LoadBpg(string filePath)
        {
            // Get file size for bitrate calculation
            long fileSizeBytes = 0;
            try
            {
                var fileInfo = new System.IO.FileInfo(filePath);
                fileSizeBytes = fileInfo.Length;
            }
            catch { }

            IntPtr handle = BpgViewerFFI.bpg_viewer_decode_file(filePath);
            if (handle == IntPtr.Zero)
            {
                return null;
            }

            int result = BpgViewerFFI.bpg_viewer_get_dimensions(handle, out uint width, out uint height);
            if (result != 0)
            {
                BpgViewerFFI.bpg_viewer_free_image(handle);
                return null;
            }

            var image = new MediaItem(handle, width, height, fileSizeBytes);

            // Decode to WriteableBitmap for WPF display
            if (!image.DecodeToWriteableBitmap())
            {
                image.Dispose();
                return null;
            }

            return image;
        }

        /// <summary>
        /// Load any supported image using the universal native decoder
        /// Supports BPG, HEIC, RAW, DNG, JPEG2000, and standard formats
        /// </summary>
        public static MediaItem? LoadUniversal(string filePath)
        {
            long fileSizeBytes = 0;
            try
            {
                fileSizeBytes = new FileInfo(filePath).Length;
            }
            catch
            {
                fileSizeBytes = 0;
            }

            IntPtr handle = BpgViewerFFI.universal_image_decode_file(filePath);
            if (handle == IntPtr.Zero)
            {
                return LoadStandardImage(filePath);
            }

            try
            {
                int result = BpgViewerFFI.universal_image_get_dimensions(handle, out uint width, out uint height);
                if (result != 0 || width == 0 || height == 0)
                {
                    BpgViewerFFI.universal_image_free(handle);
                    return null;
                }

                // Create WriteableBitmap
                var bitmap = new WriteableBitmap(
                    (int)width,
                    (int)height,
                    96, 96,
                    PixelFormats.Bgra32,
                    null);

                bitmap.Lock();
                try
                {
                    int bufferSize = (int)height * bitmap.BackBufferStride;

                    int copyResult = BpgViewerFFI.universal_image_copy_to_buffer(
                        handle,
                        bitmap.BackBuffer,
                        (UIntPtr)bufferSize,
                        (UIntPtr)bitmap.BackBufferStride);

                    if (copyResult != 0)
                    {
                        return LoadStandardImage(filePath);
                    }

                    bitmap.AddDirtyRect(new System.Windows.Int32Rect(0, 0, (int)width, (int)height));
                }
                finally
                {
                    bitmap.Unlock();
                }

                bitmap.Freeze();

                var ext = Path.GetExtension(filePath);
                var fileType = string.IsNullOrWhiteSpace(ext)
                    ? "Image"
                    : ext.TrimStart('.').ToUpperInvariant();

                return new MediaItem(bitmap, fileSizeBytes, fileType);
            }
            finally
            {
                BpgViewerFFI.universal_image_free(handle);
            }
        }

        /// <summary>
        /// Load standard image using WPF codecs (fallback for unsupported formats)
        /// </summary>
        public static MediaItem? LoadStandardImage(string filePath)
        {
            try
            {
                long fileSizeBytes = 0;
                try
                {
                    fileSizeBytes = new FileInfo(filePath).Length;
                }
                catch
                {
                    fileSizeBytes = 0;
                }

                var bitmapImage = new BitmapImage();
                bitmapImage.BeginInit();
                bitmapImage.CacheOption = BitmapCacheOption.OnLoad;
                bitmapImage.CreateOptions = BitmapCreateOptions.PreservePixelFormat;
                bitmapImage.UriSource = new Uri(filePath, UriKind.Absolute);
                bitmapImage.EndInit();
                bitmapImage.Freeze();

                BitmapSource source = bitmapImage;
                if (source.Format != PixelFormats.Bgra32)
                {
                    var converted = new FormatConvertedBitmap();
                    converted.BeginInit();
                    converted.Source = source;
                    converted.DestinationFormat = PixelFormats.Bgra32;
                    converted.EndInit();
                    converted.Freeze();
                    source = converted;
                }

                var wb = new WriteableBitmap(source);
                wb.Freeze();

                var ext = Path.GetExtension(filePath);
                var fileType = string.IsNullOrWhiteSpace(ext)
                    ? "Image"
                    : ext.TrimStart('.').ToUpperInvariant();

                return new MediaItem(wb, fileSizeBytes, fileType);
            }
            catch
            {
                return null;
            }
        }

        /// <summary>
        /// Decode the BPG image to a WriteableBitmap for WPF rendering
        /// </summary>
        private unsafe bool DecodeToWriteableBitmap()
        {
            if (_handle == IntPtr.Zero)
                return false;

            try
            {
                string logFile = System.IO.Path.Combine(
                    System.IO.Path.GetTempPath(), "bpg_viewer_debug.log");

                void Log(string msg)
                {
                    System.Diagnostics.Debug.WriteLine(msg);
                    try { System.IO.File.AppendAllText(logFile, msg + "\n"); } catch { }
                }

                Log($"=== DECODING IMAGE ===");
                Log($"Image dimensions: {Width}x{Height}");

                // Create WriteableBitmap with correct format
                Bitmap = new WriteableBitmap(
                    (int)Width,
                    (int)Height,
                    96, 96,
                    PixelFormats.Bgra32,
                    null);

                Log($"WriteableBitmap created: {Bitmap.PixelWidth}x{Bitmap.PixelHeight}");
                Log($"BackBufferStride: {Bitmap.BackBufferStride} (expected: {Width * 4})");

                Bitmap.Lock();

                try
                {
                    int bufferSize = (int)Height * Bitmap.BackBufferStride;
                    Log($"Buffer size: {bufferSize} bytes");

                    // Use the proper decode_to_buffer method which:
                    // - Writes directly to WriteableBitmap buffer
                    // - Handles stride correctly
                    // - Performs color space conversion (source colorspace -> sRGB)
                    // - Converts to BGRA32 format
                    int result = BpgViewerFFI.bpg_viewer_decode_to_buffer(
                        _handle,
                        Bitmap.BackBuffer,
                        (UIntPtr)bufferSize,
                        (UIntPtr)Bitmap.BackBufferStride);

                    Log($"decode_to_buffer result: {result}");

                    if (result != 0)
                    {
                        Log($"ERROR: decode_to_buffer failed with code {result}");
                        return false;
                    }

                    Bitmap.AddDirtyRect(new System.Windows.Int32Rect(0, 0, (int)Width, (int)Height));

                    // Sample some pixels to verify data
                    byte* ptr = (byte*)Bitmap.BackBuffer;
                    int stride = Bitmap.BackBufferStride;

                    // Sample pixel at (0,0)
                    int idx0 = 0;
                    Log($"Pixel (0,0): B={ptr[idx0]}, G={ptr[idx0+1]}, R={ptr[idx0+2]}, A={ptr[idx0+3]}");

                    // Sample pixel at (100,0)
                    int idx1 = 100 * 4;
                    Log($"Pixel (100,0): B={ptr[idx1]}, G={ptr[idx1+1]}, R={ptr[idx1+2]}, A={ptr[idx1+3]}");

                    // Sample pixel at (0,100)
                    int idx2 = 100 * stride;
                    Log($"Pixel (0,100): B={ptr[idx2]}, G={ptr[idx2+1]}, R={ptr[idx2+2]}, A={ptr[idx2+3]}");

                    // Sample pixel at (Width-1, Height-1)
                    int idxLast = ((int)Height - 1) * stride + ((int)Width - 1) * 4;
                    Log($"Pixel ({Width-1},{Height-1}): B={ptr[idxLast]}, G={ptr[idxLast+1]}, R={ptr[idxLast+2]}, A={ptr[idxLast+3]}");

                    Log($"=== FINAL BITMAP CHECK ===");
                    Log($"Bitmap.PixelWidth: {Bitmap.PixelWidth}");
                    Log($"Bitmap.PixelHeight: {Bitmap.PixelHeight}");
                    Log($"Bitmap.Width: {Bitmap.Width}");
                    Log($"Bitmap.Height: {Bitmap.Height}");
                    Log($"Bitmap.DpiX: {Bitmap.DpiX}");
                    Log($"Bitmap.DpiY: {Bitmap.DpiY}");
                    Log($"Bitmap.Format: {Bitmap.Format}");

                    // Check if pixels vary across the image (to detect if it's repetitive data)
                    uint sum0 = 0, sum1000 = 0, sumMid = 0, sumEnd = 0;
                    for (int i = 0; i < 100; i++)
                    {
                        sum0 += ptr[i];
                        if (stride * 1000 + i < (int)Height * stride)
                            sum1000 += ptr[stride * 1000 + i];
                        if ((int)Height / 2 * stride + i < (int)Height * stride)
                            sumMid += ptr[(int)Height / 2 * stride + i];
                        if (((int)Height - 1) * stride + i < (int)Height * stride)
                            sumEnd += ptr[((int)Height - 1) * stride + i];
                    }
                    Log($"Pixel data checksum (first 100 bytes of different rows):");
                    Log($"  Row 0: {sum0}");
                    Log($"  Row 1000: {sum1000}");
                    Log($"  Row {Height/2}: {sumMid}");
                    Log($"  Row {Height-1}: {sumEnd}");
                    Log($"  (If these are all similar, data might be repetitive/corrupted)");

                    Log($"=== DECODING COMPLETE ===");
                    Log($"Log file: {logFile}");
                    return true;
                }
                finally
                {
                    Bitmap.Unlock();
                }
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine($"DecodeToWriteableBitmap failed: {ex.Message}");
                System.Diagnostics.Debug.WriteLine($"Stack trace: {ex.StackTrace}");
                return false;
            }
        }

        /// <summary>
        /// Get the aspect ratio of the image
        /// </summary>
        public double AspectRatio => (double)Width / Height;

        /// <summary>
        /// Calculate bitrate in bits per pixel (bpp)
        /// </summary>
        public double BitsPerPixel
        {
            get
            {
                if (Width == 0 || Height == 0 || FileSizeBytes == 0)
                    return 0;
                long totalPixels = (long)Width * Height;
                return (FileSizeBytes * 8.0) / totalPixels;
            }
        }

        public void Dispose()
        {
            if (!_disposed)
            {
                if (_handle != IntPtr.Zero)
                {
                    BpgViewerFFI.bpg_viewer_free_image(_handle);
                    _handle = IntPtr.Zero;
                }
                Bitmap = null;
                _disposed = true;
            }
            GC.SuppressFinalize(this);
        }

        ~MediaItem()
        {
            Dispose();
        }
    }
}
