using System;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Windows.Input;
using BpgViewer.Commands;
using BpgViewer.Converters;

namespace BpgViewer.ViewModels
{
    /// <summary>
    /// Main ViewModel coordinating between single image and catalog views
    /// </summary>
    public class MainViewModel : INotifyPropertyChanged, IDisposable
    {
        #region Fields

        private ViewMode _currentViewMode = ViewMode.SingleImage;
        private bool _showInfoPanel = true;
        private bool _disposed;

        #endregion

        #region Properties

        public ImageViewerViewModel ImageViewer { get; }
        public CatalogViewModel Catalog { get; }

        public ViewMode CurrentViewMode
        {
            get => _currentViewMode;
            set
            {
                if (_currentViewMode != value)
                {
                    _currentViewMode = value;
                    OnPropertyChanged();
                    OnPropertyChanged(nameof(IsSingleImageMode));
                    OnPropertyChanged(nameof(IsCatalogMode));
                }
            }
        }

        public bool IsSingleImageMode => CurrentViewMode == ViewMode.SingleImage;
        public bool IsCatalogMode => CurrentViewMode == ViewMode.Catalog;

        public bool ShowInfoPanel
        {
            get => _showInfoPanel;
            set
            {
                _showInfoPanel = value;
                OnPropertyChanged();
            }
        }

        // Expose ImageViewer properties for status bar binding
        public string StatusMessage => IsCatalogMode ? Catalog.StatusMessage : ImageViewer.StatusMessage;
        public string ZoomPercentage => ImageViewer.ZoomPercentage;
        public bool IsImageLoaded => ImageViewer.IsImageLoaded;

        #endregion

        #region Commands

        public ICommand SwitchToSingleImageCommand { get; }
        public ICommand SwitchToCatalogCommand { get; }
        public ICommand ToggleInfoPanelCommand { get; }

        #endregion

        #region Constructor

        public MainViewModel()
        {
            ImageViewer = new ImageViewerViewModel();
            Catalog = new CatalogViewModel();

            // Wire up catalog selection to open in image viewer
            Catalog.ImageSelected += OnCatalogImageSelected;

            // Forward property changes from child view models
            ImageViewer.PropertyChanged += OnChildPropertyChanged;
            Catalog.PropertyChanged += OnChildPropertyChanged;

            // Commands
            SwitchToSingleImageCommand = new RelayCommand(() => CurrentViewMode = ViewMode.SingleImage);
            SwitchToCatalogCommand = new RelayCommand(() => CurrentViewMode = ViewMode.Catalog);
            ToggleInfoPanelCommand = new RelayCommand(() => ShowInfoPanel = !ShowInfoPanel);
        }

        #endregion

        #region Event Handlers

        private void OnCatalogImageSelected(string filePath)
        {
            // Switch to single image mode and load the selected image
            CurrentViewMode = ViewMode.SingleImage;
            ImageViewer.LoadImage(filePath);
        }

        private void OnChildPropertyChanged(object? sender, PropertyChangedEventArgs e)
        {
            // Forward status message changes
            if (e.PropertyName == nameof(ImageViewerViewModel.StatusMessage) ||
                e.PropertyName == nameof(CatalogViewModel.StatusMessage))
            {
                OnPropertyChanged(nameof(StatusMessage));
            }

            if (e.PropertyName == nameof(ImageViewerViewModel.ZoomPercentage))
            {
                OnPropertyChanged(nameof(ZoomPercentage));
            }

            if (e.PropertyName == nameof(ImageViewerViewModel.IsImageLoaded))
            {
                OnPropertyChanged(nameof(IsImageLoaded));
            }
        }

        #endregion

        #region INotifyPropertyChanged

        public event PropertyChangedEventHandler? PropertyChanged;

        protected virtual void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }

        #endregion

        #region IDisposable

        public void Dispose()
        {
            if (!_disposed)
            {
                Catalog.ImageSelected -= OnCatalogImageSelected;
                ImageViewer.PropertyChanged -= OnChildPropertyChanged;
                Catalog.PropertyChanged -= OnChildPropertyChanged;

                Catalog.Dispose();
                _disposed = true;
            }
            GC.SuppressFinalize(this);
        }

        ~MainViewModel()
        {
            Dispose();
        }

        #endregion
    }
}
