using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media.Imaging;
using DocBrake.MediaBrowser.Models;
using DocBrake.Commands;
using Microsoft.Win32;

namespace DocBrake.MediaBrowser.ViewModels
{
    /// <summary>
    /// ViewModel for the media viewer with zoom and pan support
    /// </summary>
    public class MediaViewerViewModel : INotifyPropertyChanged
    {
        #region Fields

        private MediaItem? _currentImage;
        private WriteableBitmap? _displayBitmap;
        private Uri? _videoSource;
        private bool _isVideo;
        private bool _isFitToWindow = true;
        private double _zoomLevel = 1.0;
        private Point _panOffset = new Point(0, 0);
        private string _statusMessage = "No image loaded.";
        private string _imageInfo = "";
        private bool _isImageLoaded;
        private Point _lastMousePosition;
        private bool _isPanning;
        private bool _isInfoPanelVisible = true;
        private string _shortcutsLegend = "";

        #endregion

        #region Properties

        public MediaItem? CurrentImage
        {
            get => _currentImage;
            private set
            {
                if (_currentImage != value)
                {
                    _currentImage?.Dispose();
                    _currentImage = value;
                    OnPropertyChanged();
                    UpdateImageInfo();
                }
            }
        }

        public WriteableBitmap? DisplayBitmap
        {
            get => _displayBitmap;
            private set
            {
                _displayBitmap = value;
                OnPropertyChanged();
            }
        }

        public Uri? VideoSource
        {
            get => _videoSource;
            private set
            {
                _videoSource = value;
                OnPropertyChanged();
            }
        }

        public bool IsVideo
        {
            get => _isVideo;
            private set
            {
                _isVideo = value;
                OnPropertyChanged();
            }
        }

        public bool IsFitToWindow
        {
            get => _isFitToWindow;
            private set
            {
                _isFitToWindow = value;
                OnPropertyChanged();
            }
        }

        public double ZoomLevel
        {
            get => _zoomLevel;
            set
            {
                if (Math.Abs(_zoomLevel - value) > 0.001)
                {
                    _zoomLevel = Math.Max(0.1, Math.Min(10.0, value));
                    OnPropertyChanged();
                    OnPropertyChanged(nameof(ZoomPercentage));
                }
            }
        }

        public string ZoomPercentage => $"{ZoomLevel * 100:F0}%";

        public Point PanOffset
        {
            get => _panOffset;
            set
            {
                _panOffset = value;
                OnPropertyChanged();
            }
        }

        public string StatusMessage
        {
            get => _statusMessage;
            set
            {
                _statusMessage = value;
                OnPropertyChanged();
            }
        }

        public string ImageInfo
        {
            get => _imageInfo;
            private set
            {
                _imageInfo = value;
                OnPropertyChanged();
            }
        }

        public bool IsImageLoaded
        {
            get => _isImageLoaded;
            private set
            {
                _isImageLoaded = value;
                OnPropertyChanged();
            }
        }

        public bool IsPanning
        {
            get => _isPanning;
            private set
            {
                _isPanning = value;
                OnPropertyChanged();
            }
        }

        public bool IsInfoPanelVisible
        {
            get => _isInfoPanelVisible;
            set
            {
                if (_isInfoPanelVisible != value)
                {
                    _isInfoPanelVisible = value;
                    OnPropertyChanged();
                }
            }
        }

        public string ShortcutsLegend
        {
            get => _shortcutsLegend;
            private set
            {
                _shortcutsLegend = value;
                OnPropertyChanged();
            }
        }

        #endregion

        #region Commands

        public ICommand OpenFileCommand { get; }
        public ICommand ZoomInCommand { get; }
        public ICommand ZoomOutCommand { get; }
        public ICommand FitToWindowCommand { get; }
        public ICommand ActualSizeCommand { get; }
        public ICommand ResetViewCommand { get; }
        public ICommand ToggleInfoPanelCommand { get; }

        #endregion

        #region Constructor

        public MediaViewerViewModel()
        {
            OpenFileCommand = new RelayCommand(OpenFile);
            ZoomInCommand = new RelayCommand(ZoomIn);
            ZoomOutCommand = new RelayCommand(ZoomOut);
            FitToWindowCommand = new RelayCommand<Size?>(FitToWindow);
            ActualSizeCommand = new RelayCommand(ActualSize);
            ResetViewCommand = new RelayCommand(ResetView);
            ToggleInfoPanelCommand = new RelayCommand(() => IsInfoPanelVisible = !IsInfoPanelVisible);

            ShortcutsLegend = "Keyboard Shortcuts\n" +
                              "Esc: Close\n" +
                              "Right Click: Close\n" +
                              "Drag: Move Window\n" +
                              "F: Fullscreen Toggle\n" +
                              "Arrow Keys: Prev/Next Image\n" +
                              "0: Fit to Window\n" +
                              "1: Actual Size (100%)\n" +
                              "Mouse Wheel: Zoom In/Out";
        }

        #endregion

        #region Public Methods

        /// <summary>
        /// Load any supported image from file path using universal native decoder
        /// Supports BPG, HEIC, RAW, DNG, JPEG2000, and standard formats
        /// </summary>
        public void LoadImage(string filePath)
        {
            try
            {
                StatusMessage = $"Loading: {System.IO.Path.GetFileName(filePath)}...";

                if (IsVideoFile(filePath))
                {
                    CurrentImage = null;
                    DisplayBitmap = null;
                    IsVideo = true;
                    IsFitToWindow = false;
                    VideoSource = new Uri(filePath, UriKind.Absolute);
                    IsImageLoaded = true;

                    ZoomLevel = 1.0;
                    PanOffset = new Point(0, 0);

                    StatusMessage = $"Loaded: {System.IO.Path.GetFileName(filePath)}";
                    return;
                }

                IsVideo = false;
                VideoSource = null;

                // Use universal decoder for all formats (handles HEIC, RAW, etc.)
                var image = MediaItem.LoadUniversal(filePath);
                if (image == null)
                {
                    StatusMessage = $"Failed to load image: {filePath}";
                    return;
                }

                CurrentImage = image;
                DisplayBitmap = image.Bitmap;
                IsImageLoaded = true;
                IsFitToWindow = true;

                // Reset to actual size (100% zoom) when loading new image
                ZoomLevel = 1.0;
                PanOffset = new Point(0, 0);

                StatusMessage = $"Loaded: {System.IO.Path.GetFileName(filePath)} ({image.Width}x{image.Height})";
            }
            catch (Exception ex)
            {
                StatusMessage = $"Error loading image: {ex.Message}";
                IsImageLoaded = false;
            }
        }

        private static bool IsVideoFile(string filePath)
        {
            var ext = Path.GetExtension(filePath).ToLowerInvariant();
            return ext is ".mp4" or ".mov" or ".m4v" or ".avi" or ".mkv" or ".wmv";
        }

        /// <summary>
        /// Handle mouse wheel for zooming
        /// </summary>
        public void HandleMouseWheel(int delta, Point mousePosition)
        {
            if (!IsImageLoaded)
                return;

            // Zoom factor based on wheel delta
            double zoomFactor = delta > 0 ? 1.1 : 0.9;

            // Store old zoom
            double oldZoom = ZoomLevel;

            // Update zoom
            ZoomLevel *= zoomFactor;

            // Adjust pan to keep mouse position steady (zoom to cursor)
            double zoomChange = ZoomLevel / oldZoom;
            PanOffset = new Point(
                mousePosition.X - (mousePosition.X - PanOffset.X) * zoomChange,
                mousePosition.Y - (mousePosition.Y - PanOffset.Y) * zoomChange
            );
        }

        /// <summary>
        /// Start panning
        /// </summary>
        public void StartPan(Point mousePosition)
        {
            IsPanning = true;
            _lastMousePosition = mousePosition;
        }

        /// <summary>
        /// Update pan position
        /// </summary>
        public void UpdatePan(Point mousePosition)
        {
            if (!IsPanning || !IsImageLoaded)
                return;

            double deltaX = mousePosition.X - _lastMousePosition.X;
            double deltaY = mousePosition.Y - _lastMousePosition.Y;

            PanOffset = new Point(
                PanOffset.X + deltaX,
                PanOffset.Y + deltaY
            );

            _lastMousePosition = mousePosition;
        }

        /// <summary>
        /// Stop panning
        /// </summary>
        public void StopPan()
        {
            IsPanning = false;
        }

        #endregion

        #region Private Methods

        private void OpenFile()
        {
            var dialog = new OpenFileDialog
            {
                Filter = "BPG Images|*.bpg|All Files|*.*",
                Title = "Open BPG Image"
            };

            if (dialog.ShowDialog() == true)
            {
                LoadImage(dialog.FileName);
            }
        }

        private void ZoomIn()
        {
            IsFitToWindow = false;
            ZoomLevel *= 1.2;
        }

        private void ZoomOut()
        {
            IsFitToWindow = false;
            ZoomLevel /= 1.2;
        }

        private void FitToWindow(Size? availableSize)
        {
            if (CurrentImage == null || availableSize == null || 
                availableSize.Value.Width <= 0 || availableSize.Value.Height <= 0)
                return;

            IsFitToWindow = true;

            // Fit to window with no leftover space
            double scaleX = availableSize.Value.Width / CurrentImage.Width;
            double scaleY = availableSize.Value.Height / CurrentImage.Height;

            ZoomLevel = Math.Min(scaleX, scaleY);
            PanOffset = new Point(0, 0);
        }

        private void ActualSize()
        {
            IsFitToWindow = false;
            ZoomLevel = 1.0;
            PanOffset = new Point(0, 0);
        }

        private void ResetView()
        {
            IsFitToWindow = false;
            ZoomLevel = 1.0;
            PanOffset = new Point(0, 0);
        }

        private void UpdateImageInfo()
        {
            if (CurrentImage == null)
            {
                ImageInfo = "No image loaded";
                return;
            }

            ImageInfo = $"Size: {CurrentImage.Width}×{CurrentImage.Height}\n" +
                       $"Aspect Ratio: {CurrentImage.AspectRatio:F2}\n" +
                       $"Bitrate: {CurrentImage.BitsPerPixel:F3} bpp\n" +
                       $"File Size: {FormatFileSize(CurrentImage.FileSizeBytes)}\n" +
                       $"Zoom: {ZoomPercentage}\n" +
                       $"Color Space: {CurrentImage.ColorSpace}\n" +
                       $"Date Taken: {CurrentImage.DateTaken}\n" +
                       $"Camera: {CurrentImage.CameraModel}\n" +
                       $"Lens: {CurrentImage.LensModel}";
        }

        private string FormatFileSize(long bytes)
        {
            if (bytes < 1024)
                return $"{bytes} B";
            else if (bytes < 1024 * 1024)
                return $"{bytes / 1024.0:F1} KB";
            else
                return $"{bytes / (1024.0 * 1024.0):F2} MB";
        }

        #endregion

        #region INotifyPropertyChanged

        public event PropertyChangedEventHandler? PropertyChanged;

        protected virtual void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));

            // Update image info when zoom changes
            if (propertyName == nameof(ZoomLevel))
            {
                UpdateImageInfo();
            }
        }

        #endregion
    }
}
