using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Windows.Media.Imaging;

namespace BpgViewer.Models
{
    /// <summary>
    /// Represents a thumbnail item in the catalog view
    /// </summary>
    public class ThumbnailItem : INotifyPropertyChanged, IDisposable
    {
        private BitmapImage? _thumbnailImage;
        private bool _isLoading;
        private bool _hasError;
        private string _errorMessage = string.Empty;
        private bool _isSelected;
        private bool _disposed;

        public string FilePath { get; }
        public string FileName => Path.GetFileName(FilePath);
        public string FileNameWithoutExtension => Path.GetFileNameWithoutExtension(FilePath);
        public long FileSize { get; }
        public DateTime LastModified { get; }

        public BitmapImage? ThumbnailImage
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

        public ThumbnailItem(string filePath)
        {
            FilePath = filePath;
            IsLoading = true;

            try
            {
                var fileInfo = new FileInfo(filePath);
                FileSize = fileInfo.Length;
                LastModified = fileInfo.LastWriteTime;
            }
            catch
            {
                FileSize = 0;
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
            return Path.Combine(cacheDirectory, $"{cacheKey}.png");
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
