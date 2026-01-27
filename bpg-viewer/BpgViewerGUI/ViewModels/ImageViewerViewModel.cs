using System;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media.Imaging;
using BpgViewer.Models;
using BpgViewer.Commands;
using Microsoft.Win32;

namespace BpgViewer.ViewModels
{
    /// <summary>
    /// ViewModel for the image viewer with zoom and pan support
    /// Follows OpenARC's MVVM pattern
    /// </summary>
    public class ImageViewerViewModel : INotifyPropertyChanged
    {
        #region Fields

        private BpgImage? _currentImage;
        private WriteableBitmap? _displayBitmap;
        private double _zoomLevel = 1.0;
        private Point _panOffset = new Point(0, 0);
        private string _statusMessage = "No image loaded. Click 'Open' or drag a BPG file here.";
        private string _imageInfo = "";
        private bool _isImageLoaded;
        private Point _lastMousePosition;
        private bool _isPanning;
        private bool _isInfoPanelVisible = true;

        #endregion

        #region Properties

        public BpgImage? CurrentImage
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

        public ImageViewerViewModel()
        {
            OpenFileCommand = new RelayCommand(OpenFile);
            ZoomInCommand = new RelayCommand(ZoomIn);
            ZoomOutCommand = new RelayCommand(ZoomOut);
            FitToWindowCommand = new RelayCommand<Size>(FitToWindow);
            ActualSizeCommand = new RelayCommand(ActualSize);
            ResetViewCommand = new RelayCommand(ResetView);
            ToggleInfoPanelCommand = new RelayCommand(() => IsInfoPanelVisible = !IsInfoPanelVisible);
        }

        #endregion

        #region Public Methods

        /// <summary>
        /// Load a BPG image from file path
        /// </summary>
        public void LoadImage(string filePath)
        {
            try
            {
                StatusMessage = $"Loading: {System.IO.Path.GetFileName(filePath)}...";

                var image = BpgImage.Load(filePath);
                if (image == null)
                {
                    StatusMessage = $"Failed to load image: {filePath}";
                    return;
                }

                CurrentImage = image;
                DisplayBitmap = image.Bitmap;
                IsImageLoaded = true;

                // Reset to actual size (100% zoom) when loading new image
                ZoomLevel = 1.0;

                StatusMessage = $"Loaded: {System.IO.Path.GetFileName(filePath)} ({image.Width}x{image.Height})";
            }
            catch (Exception ex)
            {
                StatusMessage = $"Error loading image: {ex.Message}";
                IsImageLoaded = false;
            }
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
            ZoomLevel *= 1.2;
        }

        private void ZoomOut()
        {
            ZoomLevel /= 1.2;
        }

        private void FitToWindow(Size availableSize)
        {
            if (CurrentImage == null || availableSize.Width <= 0 || availableSize.Height <= 0)
                return;

            // Fit to window with no leftover space
            double scaleX = availableSize.Width / CurrentImage.Width;
            double scaleY = availableSize.Height / CurrentImage.Height;

            ZoomLevel = Math.Min(scaleX, scaleY);
        }

        private void ActualSize()
        {
            ZoomLevel = 1.0;
        }

        private void ResetView()
        {
            ZoomLevel = 1.0;
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
