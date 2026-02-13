using System;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using DocBrake.MediaBrowser.NativeInterop;

namespace DocBrake.MediaBrowser.Services
{
    /// <summary>
    /// Service for gathering media file statistics without loading thumbnails
    /// </summary>
    public class MediaStatsService
    {
        private static readonly string[] VideoExtensions = { ".mp4", ".mov", ".avi", ".mkv", ".webm", ".m4v", ".wmv", ".flv" };
        private static readonly string[] ImageExtensions = { ".jpg", ".jpeg", ".png", ".bmp", ".gif", ".webp", ".bpg", ".heic", ".heif", ".tiff", ".tif", ".dng", ".cr2", ".nef", ".arw", ".orf", ".jp2", ".j2k" };

        /// <summary>
        /// Media statistics for a file
        /// </summary>
        public class MediaStats
        {
            public string FileName { get; set; } = string.Empty;
            public string FileSize { get; set; } = "Unknown";
            public string Resolution { get; set; } = "Unknown";
            public string LastModified { get; set; } = "Unknown";
            public string Bitrate { get; set; } = ""; // Only for videos
            public string Duration { get; set; } = ""; // Only for videos
            public string Format { get; set; } = "Unknown";
            public bool IsVideo { get; set; }
            public bool HasError { get; set; }
            public string ErrorMessage { get; set; } = string.Empty;

            /// <summary>
            /// Formatted text for display in a card
            /// </summary>
            public string DisplayText
            {
                get
                {
                    var lines = new System.Collections.Generic.List<string>();

                    if (!string.IsNullOrEmpty(FileName))
                        lines.Add($"📄 {FileName}");

                    if (!string.IsNullOrEmpty(Resolution) && Resolution != "Unknown")
                        lines.Add($"📐 {Resolution}");

                    if (!string.IsNullOrEmpty(FileSize) && FileSize != "Unknown")
                        lines.Add($"💾 {FileSize}");

                    if (!string.IsNullOrEmpty(LastModified) && LastModified != "Unknown")
                        lines.Add($"📅 {LastModified}");

                    if (IsVideo)
                    {
                        if (!string.IsNullOrEmpty(Duration) && Duration != "Unknown")
                            lines.Add($"⏱️ {Duration}");

                        if (!string.IsNullOrEmpty(Bitrate) && Bitrate != "Unknown")
                            lines.Add($"📊 {Bitrate}");
                    }

                    if (!string.IsNullOrEmpty(Format) && Format != "Unknown")
                        lines.Add($"🔧 {Format}");

                    return string.Join("\n", lines);
                }
            }
        }

        /// <summary>
        /// Gather statistics for a media file
        /// </summary>
        public async Task<MediaStats> GatherStatsAsync(string filePath)
        {
            var stats = new MediaStats
            {
                FileName = Path.GetFileName(filePath)
            };

            try
            {
                // Get basic file info
                if (File.Exists(filePath))
                {
                    var fileInfo = new FileInfo(filePath);
                    stats.FileSize = FormatFileSize(fileInfo.Length);
                    stats.LastModified = fileInfo.LastWriteTime.ToString("yyyy-MM-dd HH:mm", CultureInfo.InvariantCulture);
                    stats.Format = Path.GetExtension(filePath).TrimStart('.').ToUpperInvariant();
                }

                var extension = Path.GetExtension(filePath).ToLowerInvariant();

                // Check if it's a video
                stats.IsVideo = Array.IndexOf(VideoExtensions, extension) >= 0;

                if (stats.IsVideo)
                {
                    // Try to get video metadata using ffprobe
                    await GatherVideoStatsAsync(filePath, stats);
                }
                else if (Array.IndexOf(ImageExtensions, extension) >= 0)
                {
                    // Try to get image dimensions quickly
                    await GatherImageStatsAsync(filePath, stats);
                }
            }
            catch (Exception ex)
            {
                stats.HasError = true;
                stats.ErrorMessage = ex.Message;
            }

            return stats;
        }

        /// <summary>
        /// Gather video statistics using ffprobe
        /// </summary>
        private async Task GatherVideoStatsAsync(string filePath, MediaStats stats)
        {
            try
            {
                // Use ffprobe to get video info
                var startInfo = new ProcessStartInfo
                {
                    FileName = "ffprobe",
                    Arguments = $"-v error -select_streams v:0 -show_entries stream=width,height,codec_name,bit_rate -show_entries format=duration,bit_rate -of default=noprint_wrappers=1 \"{filePath}\"",
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                    UseShellExecute = false,
                    CreateNoWindow = true
                };

                using var process = new Process { StartInfo = startInfo };
                process.Start();

                var output = await process.StandardOutput.ReadToEndAsync();
                await process.WaitForExitAsync();

                if (process.ExitCode == 0)
                {
                    ParseFFProbeOutput(output, stats);
                }
            }
            catch
            {
                // If ffprobe fails, leave stats at defaults
            }
        }

        /// <summary>
        /// Parse ffprobe output to extract video metadata
        /// </summary>
        private void ParseFFProbeOutput(string output, MediaStats stats)
        {
            int width = 0, height = 0;
            double duration = 0, bitrate = 0;
            string codec = string.Empty;

            foreach (var line in output.Split('\n'))
            {
                var parts = line.Split('=');
                if (parts.Length != 2) continue;

                var key = parts[0].Trim();
                var value = parts[1].Trim();

                switch (key)
                {
                    case "width":
                        int.TryParse(value, out width);
                        break;
                    case "height":
                        int.TryParse(value, out height);
                        break;
                    case "duration":
                        double.TryParse(value, NumberStyles.Any, CultureInfo.InvariantCulture, out duration);
                        break;
                    case "bit_rate":
                        if (double.TryParse(value, NumberStyles.Any, CultureInfo.InvariantCulture, out var br))
                            bitrate = br;
                        break;
                    case "codec_name":
                        codec = value.ToUpperInvariant();
                        break;
                }
            }

            if (width > 0 && height > 0)
                stats.Resolution = $"{width}×{height}";

            if (duration > 0)
                stats.Duration = FormatDuration(duration);

            if (bitrate > 0)
                stats.Bitrate = $"{bitrate / 1000000.0:F1} Mbps";

            if (!string.IsNullOrEmpty(codec))
                stats.Format = codec;
        }

        /// <summary>
        /// Gather image statistics (dimensions)
        /// Uses the universal image decoder for format support (BPG, HEIC, JPEG2000, RAW, etc.)
        /// Falls back to WPF/WIC for standard formats
        /// </summary>
        private async Task GatherImageStatsAsync(string filePath, MediaStats stats)
        {
            await Task.Run(() =>
            {
                try
                {
                    // Try using the universal native decoder first (supports BPG, HEIC, RAW, JPEG2000, etc.)
                    IntPtr handle = IntPtr.Zero;
                    try
                    {
                        handle = BpgViewerFFI.universal_image_decode_file(filePath);
                        if (handle != IntPtr.Zero)
                        {
                            if (BpgViewerFFI.universal_image_get_dimensions(handle, out uint width, out uint height) == 0
                                && width > 0 && height > 0)
                            {
                                stats.Resolution = $"{width}×{height}";
                                return; // Success!
                            }
                        }
                    }
                    finally
                    {
                        if (handle != IntPtr.Zero)
                            BpgViewerFFI.universal_image_free(handle);
                    }

                    // Fallback: Use WPF's BitmapDecoder for standard formats (JPEG, PNG, etc.)
                    using var stream = File.OpenRead(filePath);
                    var decoder = System.Windows.Media.Imaging.BitmapDecoder.Create(
                        stream,
                        System.Windows.Media.Imaging.BitmapCreateOptions.DelayCreation,
                        System.Windows.Media.Imaging.BitmapCacheOption.None);

                    if (decoder.Frames.Count > 0)
                    {
                        var frame = decoder.Frames[0];
                        stats.Resolution = $"{frame.PixelWidth}×{frame.PixelHeight}";
                    }
                }
                catch
                {
                    // If both methods fail, resolution stays as "Unknown"
                }
            });
        }

        /// <summary>
        /// Format file size in human-readable format
        /// </summary>
        private string FormatFileSize(long bytes)
        {
            if (bytes < 1024)
                return $"{bytes} B";
            if (bytes < 1024 * 1024)
                return $"{bytes / 1024.0:F1} KB";
            if (bytes < 1024 * 1024 * 1024)
                return $"{bytes / (1024.0 * 1024.0):F1} MB";
            return $"{bytes / (1024.0 * 1024.0 * 1024.0):F2} GB";
        }

        /// <summary>
        /// Format duration in human-readable format
        /// </summary>
        private string FormatDuration(double seconds)
        {
            var ts = TimeSpan.FromSeconds(seconds);
            if (ts.TotalHours >= 1)
                return $"{(int)ts.TotalHours}:{ts.Minutes:D2}:{ts.Seconds:D2}";
            return $"{ts.Minutes}:{ts.Seconds:D2}";
        }
    }
}
