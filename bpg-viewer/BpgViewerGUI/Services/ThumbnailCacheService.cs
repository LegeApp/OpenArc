using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Media.Imaging;
using BpgViewer.NativeInterop;
using BpgViewer.Models;

namespace BpgViewer.Services
{
    /// <summary>
    /// Service for generating and caching BPG thumbnails
    /// </summary>
    public class ThumbnailCacheService : IDisposable
    {
        private readonly string _cacheDirectory;
        private readonly int _thumbnailWidth;
        private readonly int _thumbnailHeight;
        private readonly IntPtr _thumbnailHandle;
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
                "BpgViewer",
                "Cache",
                "Thumbnails");

            Directory.CreateDirectory(_cacheDirectory);

            // Create thumbnail generator handle
            _thumbnailHandle = BpgViewerFFI.bpg_thumbnail_create_with_size(
                (uint)thumbnailWidth,
                (uint)thumbnailHeight);
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
                try
                {
                    cancellationToken.ThrowIfCancellationRequested();

                    // Generate thumbnail using FFI
                    int result = BpgViewerFFI.bpg_thumbnail_generate_png(
                        _thumbnailHandle,
                        item.FilePath,
                        cachePath);

                    if (result != 0)
                    {
                        item.HasError = true;
                        item.ErrorMessage = "Decode failed";
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
                if (_thumbnailHandle != IntPtr.Zero)
                {
                    BpgViewerFFI.bpg_thumbnail_free(_thumbnailHandle);
                }
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
