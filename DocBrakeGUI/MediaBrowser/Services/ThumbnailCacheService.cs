using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Media.Imaging;
using DocBrake.MediaBrowser.NativeInterop;
using DocBrake.MediaBrowser.Models;
using DocBrake.Services;

namespace DocBrake.MediaBrowser.Services
{
    /// <summary>
    /// Service for generating and caching thumbnails for all image formats
    /// </summary>
    public class ThumbnailCacheService : IDisposable
    {
        private readonly string _cacheDirectory;
        private readonly int _thumbnailWidth;
        private readonly int _thumbnailHeight;
        private readonly SemaphoreSlim _localSemaphore;
        private static readonly SemaphoreSlim _mtpSemaphore = new SemaphoreSlim(1, 1);
        private bool _disposed;

        public ThumbnailCacheService(int thumbnailWidth = 256, int thumbnailHeight = 256, int maxConcurrency = 12)
        {
            _thumbnailWidth = thumbnailWidth;
            _thumbnailHeight = thumbnailHeight;
            // Local files can be processed concurrently.
            _localSemaphore = new SemaphoreSlim(maxConcurrency);

            // Create cache directory in AppData\Local
            string appDataPath = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            _cacheDirectory = Path.Combine(
                appDataPath,
                "OpenArc",  // Updated from "BpgViewer" to "OpenArc"
                "Cache",
                "Thumbnails");

            Directory.CreateDirectory(_cacheDirectory);
        }

        public string CacheDirectory => _cacheDirectory;

        /// <summary>
        /// Generate or load cached thumbnail for an item
        /// </summary>
        public async Task<bool> LoadThumbnailAsync(ThumbnailItem item, CancellationToken cancellationToken = default)
        {
            if (_disposed)
                return false;

            // MTP/WPD access is fragile: only allow one in-flight MTP thumbnail at a time.
            // Local thumbnails can still run concurrently.
            SemaphoreSlim semaphoreToUse = item.IsMtpFile ? _mtpSemaphore : _localSemaphore;
            bool acquired = false;
            await semaphoreToUse.WaitAsync(cancellationToken);
            acquired = true;

            try
            {
                item.IsLoading = true;
                item.HasError = false;

                string cachePath = item.GetCachePath(_cacheDirectory);

                // Check if cached thumbnail exists
                if (File.Exists(cachePath))
                {
                    return await LoadFromCacheAsync(item, cachePath, cancellationToken);
                }

                // Generate new thumbnail
                return await GenerateThumbnailAsync(item, cachePath, cancellationToken);
            }
            catch (OperationCanceledException)
            {
                return false;
            }
            catch (Exception ex)
            {
                item.HasError = true;
                item.ErrorMessage = ex.Message;
                item.IsLoading = false;
                return false;
            }
            finally
            {
                if (acquired)
                    semaphoreToUse.Release();
            }
        }

        private async Task<bool> LoadFromCacheAsync(ThumbnailItem item, string cachePath, CancellationToken cancellationToken)
        {
            return await Task.Run(() =>
            {
                try
                {
                    cancellationToken.ThrowIfCancellationRequested();

                    // Use stream for faster loading
                    using var stream = new FileStream(cachePath, FileMode.Open, FileAccess.Read, FileShare.Read, 4096, FileOptions.SequentialScan);
                    var bitmap = new BitmapImage();
                    bitmap.BeginInit();
                    bitmap.CacheOption = BitmapCacheOption.OnLoad;
                    bitmap.StreamSource = stream;
                    bitmap.EndInit();
                    bitmap.Freeze(); // Make cross-thread accessible

                    item.ThumbnailImage = bitmap;
                    item.IsLoading = false;
                    return true;
                }
                catch (Exception)
                {
                    // Cache file might be corrupted, try regenerating
                    try { File.Delete(cachePath); } catch { }
                    item.HasError = true;
                    item.ErrorMessage = "Cache error";
                    item.IsLoading = false;
                    return false;
                }
            }, cancellationToken);
        }

        private async Task<bool> GenerateThumbnailAsync(ThumbnailItem item, string cachePath, CancellationToken cancellationToken)
        {
            return await Task.Run(() =>
            {
                IntPtr handle = IntPtr.Zero;
                string? tempFilePath = null;
                bool isMtpFile = item.IsMtpFile;

                try
                {
                    cancellationToken.ThrowIfCancellationRequested();

                    // If this is an MTP file, use new GetMtpThumbnail that tries WPD thumbnails first
                    string sourceFilePath = item.FilePath;
                    if (isMtpFile && !string.IsNullOrEmpty(item.MtpDeviceId) && !string.IsNullOrEmpty(item.MtpObjectId))
                    {
                        // New approach: Try WPD thumbnail first (fast), fallback to full file (slow but sequential)
                        var mtpResult = DocBrake.NativeInterop.OpenArcFFI.GetMtpThumbnail(
                            item.MtpDeviceId, 
                            item.MtpObjectId, 
                            item.FileName,
                            (uint)_thumbnailWidth,
                            (uint)_thumbnailHeight);
                        
                        if (!mtpResult.success || string.IsNullOrEmpty(mtpResult.data))
                        {
                            item.HasError = true;
                            item.ErrorMessage = mtpResult.error ?? "Failed to get thumbnail from device";
                            item.IsLoading = false;
                            return false;
                        }
                        sourceFilePath = mtpResult.data;
                        tempFilePath = mtpResult.data; // Rust manages temp file cleanup
                    }

                    handle = BpgViewerFFI.universal_thumbnail_create_with_size(
                        (uint)_thumbnailWidth,
                        (uint)_thumbnailHeight);

                    if (handle == IntPtr.Zero)
                    {
                        item.HasError = true;
                        item.ErrorMessage = "Create handle failed";
                        item.IsLoading = false;
                        return false;
                    }

                    // Generate thumbnail using universal FFI
                    int thumbResult = BpgViewerFFI.universal_thumbnail_generate_png(
                        handle,
                        sourceFilePath,
                        cachePath);

                    if (thumbResult != 0)
                    {
                        item.HasError = true;
                        item.ErrorMessage = isMtpFile ? $"Decode failed from device ({thumbResult})" : $"Decode failed ({thumbResult})";
                        item.IsLoading = false;
                        return false;
                    }

                    cancellationToken.ThrowIfCancellationRequested();

                    // Load the generated thumbnail using stream
                    if (File.Exists(cachePath))
                    {
                        using var stream = new FileStream(cachePath, FileMode.Open, FileAccess.Read, FileShare.Read, 4096, FileOptions.SequentialScan);
                        var bitmap = new BitmapImage();
                        bitmap.BeginInit();
                        bitmap.CacheOption = BitmapCacheOption.OnLoad;
                        bitmap.StreamSource = stream;
                        bitmap.EndInit();
                        bitmap.Freeze();

                        item.ThumbnailImage = bitmap;
                        item.IsLoading = false;
                        return true;
                    }

                    item.HasError = true;
                    item.ErrorMessage = "No output";
                    item.IsLoading = false;
                    return false;
                }
                catch (OperationCanceledException)
                {
                    throw;
                }
                catch (Exception ex)
                {
                    item.HasError = true;
                    item.ErrorMessage = isMtpFile ? $"MTP Error: {ex.Message}" : "Error";
                    item.IsLoading = false;
                    return false;
                }
                finally
                {
                    if (handle != IntPtr.Zero)
                    {
                        BpgViewerFFI.universal_thumbnail_free(handle);
                    }
                    
                    // Clean up temp file if it was created (optional - could keep for cache)
                    // Note: We keep temp files for performance; they act as a second-level cache
                    // Rust MTP backend handles temp file management
                }
            }, cancellationToken);
        }

        /// <summary>
        /// Clear all cached thumbnails
        /// </summary>
        public void ClearCache()
        {
            try
            {
                if (Directory.Exists(_cacheDirectory))
                {
                    foreach (var file in Directory.GetFiles(_cacheDirectory, "*.png"))
                    {
                        try { File.Delete(file); } catch { }
                    }
                }
            }
            catch { }
        }

        /// <summary>
        /// Get cache size in bytes
        /// </summary>
        public long GetCacheSize()
        {
            try
            {
                if (!Directory.Exists(_cacheDirectory))
                    return 0;

                long size = 0;
                foreach (var file in Directory.GetFiles(_cacheDirectory, "*.png"))
                {
                    try
                    {
                        size += new FileInfo(file).Length;
                    }
                    catch { }
                }
                return size;
            }
            catch
            {
                return 0;
            }
        }

        public void Dispose()
        {
            if (!_disposed)
            {
                _localSemaphore.Dispose();
                _disposed = true;
            }
            GC.SuppressFinalize(this);
        }

        ~ThumbnailCacheService()
        {
            Dispose();
        }
    }
}
