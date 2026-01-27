using System;
using System.Collections.Generic;

namespace DocBrake.Models
{
    public enum FileType
    {
        Unknown,
        Image,
        Video,
        Document,
        Archive,
        Folder
    }

    public class DocumentItem
    {
        public string FilePath { get; set; } = string.Empty;
        public string FileName { get; set; } = string.Empty;
        public FileType FileType { get; set; } = FileType.Unknown;
        public List<string> SourcePaths { get; set; } = new();
        public bool IsFolder => FileType == FileType.Folder;
        public DocumentStatus Status { get; set; } = DocumentStatus.Pending;
        public string? OutputPath { get; set; }
        public int Progress { get; set; } = 0;
        public string? ErrorMessage { get; set; }
        public DateTime AddedTime { get; set; } = DateTime.Now;
        public TimeSpan? ProcessingTime { get; set; }
        public long FileSize { get; set; }
        public long CompressedSize { get; set; }
        public double CompressionRatio => FileSize > 0 ? (double)CompressedSize / FileSize * 100 : 0;
    }

    public enum DocumentStatus
    {
        Pending,
        Processing,
        Completed,
        Error,
        Cancelled
    }

    public class ProcessingProgress
    {
        public int CurrentPage { get; set; }
        public int TotalPages { get; set; }
        public string Status { get; set; } = string.Empty;
        public double ProgressPercentage => TotalPages > 0 ? (double)CurrentPage / TotalPages * 100 : 0;
    }

    public class ProcessingResult
    {
        public bool Success { get; set; }
        public string? OutputPath { get; set; }
        public string? ErrorMessage { get; set; }
        public TimeSpan ProcessingTime { get; set; }
    }
}
