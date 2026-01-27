using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Media.Imaging;
using DocBrake.MediaBrowser.NativeInterop;
using DocBrake.MediaBrowser.Models;

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
        private readonly SemaphoreSlim _semaphore;
        private bool _disposed;

        public ThumbnailCacheService(int thumbnailWidth = 256, int thumbnailHeight = 256, int maxConcurrency = 12)
        {
            _thumbnailWidth = thumbnailWidth;
            _thumbnailHeight = thumbnailHeight;
            // High concurrency for responsive UI during loading
            _semaphore = new SemaphoreSlim(maxConcurrency);

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

            await _semaphore.WaitAsync(cancellationToken);

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
                _semaphore.Release();
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
                try
                {
                    cancellationToken.ThrowIfCancellationRequested();

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
                    int result = BpgViewerFFI.universal_thumbnail_generate_png(
                        handle,
                        item.FilePath,
                        cachePath);

                    if (result != 0)
                    {
                        item.HasError = true;
                        item.ErrorMessage = $"Decode failed ({result})";
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
                catch (Exception)
                {
                    item.HasError = true;
                    item.ErrorMessage = "Error";
                    item.IsLoading = false;
                    return false;
                }
                finally
                {
                    if (handle != IntPtr.Zero)
                    {
                        BpgViewerFFI.universal_thumbnail_free(handle);
                    }
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
                _semaphore.Dispose();
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
