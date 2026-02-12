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
        private static bool _gpuAvailable = false;
        private static bool _gpuInitialized = false;

        private static void DebugLog(string message)
        {
            try
            {
                var logPath = System.IO.Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "debug.log");
                var timestamp = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss.fff");
                System.IO.File.AppendAllText(logPath, $"[{timestamp}] [ThumbnailCache] {message}\n");
            }
            catch
            {
                // Ignore logging errors
            }
        }

        public ThumbnailCacheService(int thumbnailWidth = 128, int thumbnailHeight = 128, int maxConcurrency = 8)
        {
            _thumbnailWidth = thumbnailWidth;
            _thumbnailHeight = thumbnailHeight;

            // Tokio async pipeline manages its own concurrency (8 concurrent operations)
            // Semaphore here is just for rate limiting C# side requests
            _localSemaphore = new SemaphoreSlim(maxConcurrency);

            // Create cache directory in AppData\Local
            string appDataPath = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            _cacheDirectory = Path.Combine(
                appDataPath,
                "OpenArc",
                "Cache",
                "Thumbnails");

            Directory.CreateDirectory(_cacheDirectory);

            // Tokio async pipeline initialization (CPU-only mode)
            if (!_gpuInitialized)
            {
                DebugLog("Initializing Tokio async thumbnail pipeline");
                _gpuInitialized = true;
                _gpuAvailable = false; // GPU disabled, CPU-only

                var msg = "================================================\n" +
                         "THUMBNAIL PIPELINE: Tokio Async (CPU-only)\n" +
                         "Mode: CPU with fast_image_resize\n" +
                         "Concurrency: 8 parallel operations\n" +
                         "Expected: 50-200ms per thumbnail\n" +
                         "================================================";

                Console.WriteLine(msg);
                Console.Out.Flush();

                System.Diagnostics.Debug.WriteLine("[ThumbnailCache] Tokio async pipeline initialized (CPU-only)");
                System.Diagnostics.Trace.WriteLine("[ThumbnailCache] Tokio async pipeline initialized (CPU-only)");

                DebugLog("Tokio async thumbnail pipeline ready");
            }
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

        private bool TryUnifiedThumbnail(string sourceFilePath, string fileName, string cachePath)
        {
            Console.WriteLine($">> PROCESSING: {fileName}");
            Console.Out.Flush();

            // Use ManualResetEventSlim to wait for async completion
            using var completionEvent = new ManualResetEventSlim(false);
            int result = -1;

            try
            {
                ulong sourceId = (ulong)sourceFilePath.GetHashCode();
                IntPtr errorPtr = IntPtr.Zero;

                // IMPORTANT: Keep callback delegate alive to prevent GC collection
                // Store in variable before passing to native code
                BpgViewerFFI.ThumbnailCallback callback = (sid, res, errPtr) => {
                    result = res;
                    errorPtr = errPtr;
                    completionEvent.Set();
                };

                // Use new Tokio async pipeline (CPU-only, no COM conflicts)
                int queueResult = BpgViewerFFI.bpg_generate_thumbnail_async(
                    sourceId,
                    sourceFilePath,
                    cachePath,
                    85, // quality
                    callback);

                if (queueResult != 0)
                {
                    Console.WriteLine($"!! QUEUE FAILED: {fileName} (error: {queueResult})");
                    Console.Out.Flush();
                    return false;
                }

                // Wait for completion with timeout
                if (!completionEvent.Wait(TimeSpan.FromSeconds(30)))
                {
                    Console.WriteLine($"!! TIMEOUT: {fileName}");
                    Console.Out.Flush();
                    GC.KeepAlive(callback); // Ensure callback stays alive
                    return false;
                }

                // Keep callback alive until after it's been invoked
                GC.KeepAlive(callback);

                if (result != 0)
                {
                    string errorMsg = "";
                    if (errorPtr != IntPtr.Zero)
                    {
                        errorMsg = System.Runtime.InteropServices.Marshal.PtrToStringAnsi(errorPtr) ?? "";
                        BpgViewerFFI.bpg_error_free(errorPtr);
                    }
                    Console.WriteLine($"!! THUMBNAIL FAILED: {fileName} (error: {result}, message: {errorMsg})");
                    Console.Out.Flush();
                    return false;
                }

                if (!File.Exists(cachePath))
                {
                    Console.WriteLine($"!! OUTPUT MISSING: {fileName}");
                    Console.Out.Flush();
                    return false;
                }

                Console.WriteLine($"<< SUCCESS (CPU): {fileName}");
                Console.Out.Flush();
                return true;
            }
            catch (Exception ex)
            {
                Console.WriteLine($"!! EXCEPTION: {fileName} - {ex.Message}");
                Console.Out.Flush();
                return false;
            }
        }

        private async Task<bool> GenerateThumbnailAsync(ThumbnailItem item, string cachePath, CancellationToken cancellationToken)
        {
            return await Task.Run(() =>
            {
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

                    // Use unified thumbnail API (auto-selects GPU or CPU backend)
                    // This replaces the old two-path approach (GPU then CPU fallback)
                    if (TryUnifiedThumbnail(sourceFilePath, Path.GetFileName(sourceFilePath), cachePath))
                    {
                        // Success - load thumbnail
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
                    
                    // Unified thumbnail failed
                    item.HasError = true;
                    item.ErrorMessage = "Thumbnail generation failed";
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
                    foreach (var file in Directory.GetFiles(_cacheDirectory, "*.jpg"))
                    {
                        try { File.Delete(file); } catch { }
                    }
                    // Also clean up legacy PNG cache files
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
                foreach (var file in Directory.GetFiles(_cacheDirectory, "*.jpg"))
                {
                    try
                    {
                        size += new FileInfo(file).Length;
                    }
                    catch { }
                }
                // Also count legacy PNG cache files
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
