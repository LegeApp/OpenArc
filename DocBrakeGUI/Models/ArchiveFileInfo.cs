using System;

namespace DocBrake.Models
{
    public class ArchiveFileInfo
    {
        public string Filename { get; set; } = string.Empty;
        public ulong OriginalSize { get; set; }
        public ulong CompressedSize { get; set; }
        public FileType FileType { get; set; }
        
        public double CompressionRatio => OriginalSize > 0 ? (double)CompressedSize / OriginalSize : 0;
        public string FormattedOriginalSize => FormatFileSize(OriginalSize);
        public string FormattedCompressedSize => FormatFileSize(CompressedSize);
        public string CompressionPercentage => $"{CompressionRatio * 100:F1}%";
        
        private static string FormatFileSize(ulong bytes)
        {
            string[] sizes = { "B", "KB", "MB", "GB", "TB" };
            double len = bytes;
            int order = 0;
            while (len >= 1024 && order < sizes.Length - 1)
            {
                order++;
                len = len / 1024;
            }
            return $"{len:0.##} {sizes[order]}";
        }
    }
}
