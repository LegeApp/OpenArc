using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace DocBrake.MediaBrowser.Models
{
    /// <summary>
    /// Represents a thumbnail item in the catalog view
    /// </summary>
    public class ThumbnailItem : INotifyPropertyChanged, IDisposable
    {
        private BitmapSource? _thumbnailImage;
        private bool _isLoading;
        private bool _hasError;
        private string _errorMessage = string.Empty;
        private bool _isSelected;
        private bool _isChecked;
        private bool _disposed;
        private string _fileName;
        private long _fileSize;

        // Extension color mapping - pastel/primary colors
        private static readonly Dictionary<string, Color> ExtensionColors = new(StringComparer.OrdinalIgnoreCase)
        {
            // Images
            { ".jpg", Color.FromRgb(255, 182, 193) },   // Light Pink
            { ".jpeg", Color.FromRgb(255, 182, 193) },  // Light Pink
            { ".png", Color.FromRgb(173, 216, 230) },   // Light Blue
            { ".gif", Color.FromRgb(221, 160, 221) },   // Plum
            { ".bmp", Color.FromRgb(255, 218, 185) },   // Peach
            { ".webp", Color.FromRgb(152, 251, 152) },  // Pale Green
            { ".heic", Color.FromRgb(255, 228, 181) },  // Moccasin
            { ".heif", Color.FromRgb(255, 228, 181) },  // Moccasin
            { ".tiff", Color.FromRgb(176, 196, 222) },  // Light Steel Blue
            { ".tif", Color.FromRgb(176, 196, 222) },   // Light Steel Blue
            { ".bpg", Color.FromRgb(255, 160, 122) },   // Light Salmon
            
            // RAW formats
            { ".raw", Color.FromRgb(144, 238, 144) },   // Light Green
            { ".dng", Color.FromRgb(135, 206, 235) },   // Sky Blue
            { ".cr2", Color.FromRgb(255, 255, 224) },   // Light Yellow
            { ".nef", Color.FromRgb(250, 250, 210) },   // Light Goldenrod
            { ".arw", Color.FromRgb(240, 230, 140) },   // Khaki
            { ".orf", Color.FromRgb(238, 232, 170) },   // Pale Goldenrod
            
            // Videos
            { ".mp4", Color.FromRgb(255, 127, 80) },    // Coral
            { ".mov", Color.FromRgb(255, 99, 71) },     // Tomato
            { ".avi", Color.FromRgb(255, 140, 0) },     // Dark Orange
            { ".mkv", Color.FromRgb(255, 165, 0) },     // Orange
            { ".webm", Color.FromRgb(255, 215, 0) },    // Gold
            
            // JPEG 2000
            { ".jp2", Color.FromRgb(175, 238, 238) },   // Pale Turquoise
            { ".j2k", Color.FromRgb(175, 238, 238) },   // Pale Turquoise
        };

        private static readonly Color DefaultColor = Color.FromRgb(200, 200, 200); // Light Gray

        public string FilePath { get; }
        
        public string FileName 
        { 
            get => _fileName;
            set
            {
                _fileName = value;
                OnPropertyChanged();
            }
        }
        
        public string FileNameWithoutExtension => Path.GetFileNameWithoutExtension(_fileName);
        public string FileExtension => Path.GetExtension(_fileName);
        
        public long FileSize 
        { 
            get => _fileSize;
            set
            {
                _fileSize = value;
                OnPropertyChanged();
                OnPropertyChanged(nameof(FileSizeFormatted));
            }
        }

        public DateTime LastModified { get; }
        
        /// <summary>
        /// MTP device ID if this file is on an MTP device
        /// </summary>
        public string? MtpDeviceId { get; set; }
        
        /// <summary>
        /// MTP object ID if this file is on an MTP device
        /// </summary>
        public string? MtpObjectId { get; set; }
        
        /// <summary>
        /// True if this is an MTP file
        /// </summary>
        public bool IsMtpFile => !string.IsNullOrEmpty(MtpDeviceId);
        
        /// <summary>
        /// The resolved local path for this file (for MTP files, this is the cached temp path)
        /// </summary>
        public string? ResolvedLocalPath { get; set; }

        public BitmapSource? ThumbnailImage
        {
            get => _thumbnailImage;
            set
            {
                _thumbnailImage = value;
                OnPropertyChanged();
            }
        }

        public bool IsLoading
        {
            get => _isLoading;
            set
            {
                _isLoading = value;
                OnPropertyChanged();
            }
        }

        public bool HasError
        {
            get => _hasError;
            set
            {
                _hasError = value;
                OnPropertyChanged();
            }
        }

        public string ErrorMessage
        {
            get => _errorMessage;
            set
            {
                _errorMessage = value;
                OnPropertyChanged();
            }
        }

        public bool IsSelected
        {
            get => _isSelected;
            set
            {
                _isSelected = value;
                OnPropertyChanged();
            }
        }

        public bool IsChecked
        {
            get => _isChecked;
            set
            {
                _isChecked = value;
                OnPropertyChanged();
            }
        }

        public string FileSizeFormatted
        {
            get
            {
                if (FileSize < 1024)
                    return $"{FileSize} B";
                if (FileSize < 1024 * 1024)
                    return $"{FileSize / 1024.0:F1} KB";
                return $"{FileSize / (1024.0 * 1024.0):F1} MB";
            }
        }

        /// <summary>
        /// Gets a color brush based on the file extension
        /// </summary>
        public SolidColorBrush ExtensionColor
        {
            get
            {
                var ext = FileExtension.ToLowerInvariant();
                var color = ExtensionColors.TryGetValue(ext, out var c) ? c : DefaultColor;
                return new SolidColorBrush(color);
            }
        }

        /// <summary>
        /// Gets a contrasting text color (dark for light backgrounds)
        /// </summary>
        public SolidColorBrush ExtensionTextColor
        {
            get
            {
                var ext = FileExtension.ToLowerInvariant();
                var bgColor = ExtensionColors.TryGetValue(ext, out var c) ? c : DefaultColor;
                // Calculate relative luminance
                double luminance = (0.299 * bgColor.R + 0.587 * bgColor.G + 0.114 * bgColor.B) / 255;
                // Use dark text for light backgrounds
                return new SolidColorBrush(luminance > 0.5 ? Color.FromRgb(40, 40, 40) : Color.FromRgb(255, 255, 255));
            }
        }

        /// <summary>
        /// Combined info string for overlay: "filename.ext | 2.1 MB"
        /// </summary>
        public string OverlayText => $"{FileName} | {FileSizeFormatted}";

        public ThumbnailItem(string filePath)
        {
            FilePath = filePath;
            _fileName = Path.GetFileName(filePath);
            IsLoading = true;

            try
            {
                var fileInfo = new FileInfo(filePath);
                _fileSize = fileInfo.Length;
                LastModified = fileInfo.LastWriteTime;
            }
            catch
            {
                _fileSize = 0;
                LastModified = DateTime.MinValue;
            }
        }

        /// <summary>
        /// Get the cache file path for this thumbnail
        /// </summary>
        public string GetCachePath(string cacheDirectory)
        {
            // Use hash of full path + modification time for cache key
            string cacheKey = $"{FilePath}_{LastModified.Ticks}".GetHashCode().ToString("X8");
            return Path.Combine(cacheDirectory, $"{cacheKey}.jpg");
        }

        public void Dispose()
        {
            if (!_disposed)
            {
                _thumbnailImage = null;
                _disposed = true;
            }
            GC.SuppressFinalize(this);
        }

        ~ThumbnailItem()
        {
            Dispose();
        }

        #region INotifyPropertyChanged

        public event PropertyChangedEventHandler? PropertyChanged;

        protected virtual void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }

        #endregion
    }
}
