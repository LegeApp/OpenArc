using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Input;
using BpgViewer.Commands;
using BpgViewer.Models;
using BpgViewer.Services;

namespace BpgViewer.ViewModels
{
    /// <summary>
    /// ViewModel for the thumbnail catalog view
    /// </summary>
    public class CatalogViewModel : INotifyPropertyChanged, IDisposable
    {
        #region Fields

        private readonly ThumbnailCacheService _cacheService;
        private CancellationTokenSource? _loadingCts;

        private string _currentDirectory = string.Empty;
        private string _statusMessage = "Select a folder to browse BPG images.";
        private bool _isLoading;
        private int _loadedCount;
        private int _totalCount;
        private ThumbnailItem? _selectedItem;
        private int _thumbnailSize = 200;
        private double _viewportWidth = 1000; // Default width
        private int _columnsPerRow = 4; // Default columns
        private bool _disposed;
        private FolderItem? _selectedFolder;

        #endregion

        #region Properties

        public ObservableCollection<ThumbnailItem> Thumbnails { get; } = new();
        public ObservableCollection<FolderItem> RootFolders { get; } = new();

        public FolderItem? SelectedFolder
        {
            get => _selectedFolder;
            set
            {
                if (_selectedFolder != value)
                {
                    _selectedFolder = value;
                    OnPropertyChanged();
                    if (_selectedFolder != null)
                    {
                        _ = LoadDirectoryAsync(_selectedFolder.FullPath);
                    }
                }
            }
        }

        public string CurrentDirectory
        {
            get => _currentDirectory;
            set
            {
                _currentDirectory = value;
                OnPropertyChanged();
                OnPropertyChanged(nameof(DirectoryName));
            }
        }

        public string DirectoryName => string.IsNullOrEmpty(_currentDirectory)
            ? "No folder selected"
            : Path.GetFileName(_currentDirectory) ?? _currentDirectory;

        public string StatusMessage
        {
            get => _statusMessage;
            set
            {
                _statusMessage = value;
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

        public int LoadedCount
        {
            get => _loadedCount;
            set
            {
                _loadedCount = value;
                OnPropertyChanged();
                OnPropertyChanged(nameof(LoadingProgress));
            }
        }

        public int TotalCount
        {
            get => _totalCount;
            set
            {
                _totalCount = value;
                OnPropertyChanged();
                OnPropertyChanged(nameof(LoadingProgress));
            }
        }

        public double LoadingProgress => TotalCount > 0 ? (double)LoadedCount / TotalCount * 100 : 0;

        public ThumbnailItem? SelectedItem
        {
            get => _selectedItem;
            set
            {
                if (_selectedItem != null)
                    _selectedItem.IsSelected = false;

                _selectedItem = value;

                if (_selectedItem != null)
                    _selectedItem.IsSelected = true;

                OnPropertyChanged();
            }
        }

        public int ThumbnailSize
        {
            get => _thumbnailSize;
            set
            {
                _thumbnailSize = Math.Max(80, Math.Min(600, value));
                OnPropertyChanged();
                OnPropertyChanged(nameof(ThumbnailMargin));
                OnPropertyChanged(nameof(ThumbnailImageHeight));
            }
        }

        public double ViewportWidth
        {
            get => _viewportWidth;
            set
            {
                _viewportWidth = value;
                OnPropertyChanged();
                UpdateResponsiveLayout();
            }
        }

        public int ColumnsPerRow
        {
            get => _columnsPerRow;
            private set
            {
                _columnsPerRow = value;
                OnPropertyChanged();
            }
        }

        // Minimal spacing for dense aesthetic
        public Thickness ThumbnailMargin => new Thickness(4);

        // Height for thumbnail image area (size minus text area)
        public int ThumbnailImageHeight => Math.Max(60, ThumbnailSize - 35);

        public string CacheSizeFormatted
        {
            get
            {
                long size = _cacheService.GetCacheSize();
                if (size < 1024)
                    return $"{size} B";
                if (size < 1024 * 1024)
                    return $"{size / 1024.0:F1} KB";
                return $"{size / (1024.0 * 1024.0):F1} MB";
            }
        }

        #endregion

        #region Events

        public event Action<string>? ImageSelected;

        #endregion

        #region Commands

        public ICommand OpenFileCommand { get; }
        public ICommand OpenFolderCommand { get; }
        public ICommand RefreshCommand { get; }
        public ICommand ClearCacheCommand { get; }
        public ICommand OpenSelectedCommand { get; }
        public ICommand IncreaseSizeCommand { get; }
        public ICommand DecreaseSizeCommand { get; }

        #endregion

        #region Constructor

        public CatalogViewModel()
        {
            _cacheService = new ThumbnailCacheService(256, 256, 8);

            OpenFileCommand = new RelayCommand(OpenFile);
            OpenFolderCommand = new RelayCommand(OpenFolder);
            RefreshCommand = new RelayCommand(Refresh, () => !string.IsNullOrEmpty(CurrentDirectory));
            ClearCacheCommand = new RelayCommand(ClearCache);
            OpenSelectedCommand = new RelayCommand(OpenSelected, () => SelectedItem != null);
            IncreaseSizeCommand = new RelayCommand(() => AdjustThumbnailSize(20));
            DecreaseSizeCommand = new RelayCommand(() => AdjustThumbnailSize(-20));

            UpdateResponsiveLayout();
            LoadDrives();
        }

        private void LoadDrives()
        {
            try
            {
                RootFolders.Clear();
                foreach (var drive in DriveInfo.GetDrives().Where(d => d.IsReady))
                {
                    try
                    {
                        RootFolders.Add(new FolderItem(drive.Name, true) 
                        { 
                            Name = $"{drive.VolumeLabel} ({drive.Name})" 
                        });
                    }
                    catch
                    {
                        RootFolders.Add(new FolderItem(drive.Name, true));
                    }
                }
            }
            catch (Exception ex)
            {
                StatusMessage = $"Error listing drives: {ex.Message}";
            }
        }

        /// <summary>
        /// Adjust thumbnail size smoothly with scroll wheel or keyboard
        /// </summary>
        public void AdjustThumbnailSize(int delta)
        {
            ThumbnailSize += delta;
        }

        /// <summary>
        /// Handle mouse wheel for thumbnail sizing
        /// </summary>
        public void HandleMouseWheel(int delta)
        {
            // Scroll up = larger thumbnails (decrease columns)
            // Scroll down = smaller thumbnails (increase columns)
            int adjustment = delta > 0 ? 15 : -15;
            AdjustThumbnailSize(adjustment);
        }

        /// <summary>
        /// Update responsive layout based on viewport width
        /// Calculate optimal columns and thumbnail size
        /// </summary>
        private void UpdateResponsiveLayout()
        {
            // Window size categorization
            if (_viewportWidth < 800) // Small
            {
                ColumnsPerRow = 3;
                if (ThumbnailSize > 250) ThumbnailSize = 250;
            }
            else if (_viewportWidth < 1400) // Large
            {
                ColumnsPerRow = 4;
                if (ThumbnailSize > 300) ThumbnailSize = 300;
            }
            else // Full screen
            {
                ColumnsPerRow = 8;
                if (ThumbnailSize == 200) ThumbnailSize = 180; // Default for full screen
            }
        }

        #endregion

        #region Public Methods

        /// <summary>
        /// Load BPG files from a directory
        /// </summary>
        public async Task LoadDirectoryAsync(string directoryPath)
        {
            if (!Directory.Exists(directoryPath))
            {
                StatusMessage = $"Directory not found: {directoryPath}";
                return;
            }

            // Cancel any existing load operation
            CancelLoading();

            CurrentDirectory = directoryPath;
            Thumbnails.Clear();
            LoadedCount = 0;
            TotalCount = 0;

            IsLoading = true;
            StatusMessage = "Scanning for BPG files...";

            _loadingCts = new CancellationTokenSource();
            var token = _loadingCts.Token;

            try
            {
                await Task.Run(() =>
                {
                    var batch = new System.Collections.Generic.List<ThumbnailItem>();
                    int count = 0;

                    try
                    {
                        var files = Directory.EnumerateFiles(directoryPath, "*.bpg", SearchOption.TopDirectoryOnly)
                                             .OrderBy(f => f);

                        foreach (var file in files)
                        {
                            if (token.IsCancellationRequested) break;

                            batch.Add(new ThumbnailItem(file));
                            count++;

                            // Batch update UI every 50 items
                            if (batch.Count >= 50)
                            {
                                var currentBatch = batch.ToList(); // Copy
                                batch.Clear();
                                
                                Application.Current.Dispatcher.Invoke(() =>
                                {
                                    foreach (var item in currentBatch)
                                    {
                                        Thumbnails.Add(item);
                                    }
                                    TotalCount = Thumbnails.Count;
                                    StatusMessage = $"Found {TotalCount} BPG files...";
                                });
                            }
                        }

                        // Add remaining items
                        if (batch.Count > 0)
                        {
                            Application.Current.Dispatcher.Invoke(() =>
                            {
                                foreach (var item in batch)
                                {
                                    Thumbnails.Add(item);
                                }
                                TotalCount = Thumbnails.Count;
                            });
                        }
                    }
                    catch (Exception ex)
                    {
                        Application.Current.Dispatcher.Invoke(() =>
                        {
                            StatusMessage = $"Error scanning files: {ex.Message}";
                        });
                    }
                }, token);

                if (Thumbnails.Count == 0)
                {
                    StatusMessage = "No BPG files found in this folder.";
                    IsLoading = false;
                    return;
                }

                StatusMessage = $"Loading thumbnails for {Thumbnails.Count} files...";

                // Start loading thumbnails
                await LoadThumbnailsAsync(token);

                StatusMessage = $"Loaded {LoadedCount} of {TotalCount} thumbnails.";
            }
            catch (OperationCanceledException)
            {
                StatusMessage = "Loading canceled.";
            }
            catch (Exception ex)
            {
                StatusMessage = $"Error loading directory: {ex.Message}";
            }
            finally
            {
                IsLoading = false;
            }
        }

        /// <summary>
        /// Handle thumbnail double-click
        /// </summary>
        public void OnThumbnailDoubleClick(ThumbnailItem item)
        {
            ImageSelected?.Invoke(item.FilePath);
        }

        #endregion

        #region Private Methods

        private async Task LoadThumbnailsAsync(CancellationToken cancellationToken)
        {
            // Load thumbnails in parallel batches
            var tasks = Thumbnails.Select(async item =>
            {
                if (cancellationToken.IsCancellationRequested)
                    return;

                bool success = await _cacheService.LoadThumbnailAsync(item, cancellationToken);

                await Application.Current.Dispatcher.InvokeAsync(() =>
                {
                    LoadedCount++;
                    if (LoadedCount % 10 == 0 || LoadedCount == TotalCount)
                    {
                        StatusMessage = $"Loading thumbnails... {LoadedCount}/{TotalCount}";
                    }
                });
            });

            await Task.WhenAll(tasks);
        }

        private void OpenFile()
        {
            var dialog = new Microsoft.Win32.OpenFileDialog
            {
                Filter = "BPG Images|*.bpg|All Files|*.*",
                Title = "Open BPG Image"
            };

            if (dialog.ShowDialog() == true)
            {
                // Open single image in new window
                ImageSelected?.Invoke(dialog.FileName);
            }
        }

        private void OpenFolder()
        {
            var dialog = new Ookii.Dialogs.Wpf.VistaFolderBrowserDialog
            {
                Description = "Select a folder containing BPG images",
                UseDescriptionForTitle = true,
                Multiselect = false,
                ShowNewFolderButton = false
            };

            if (dialog.ShowDialog() == true)
            {
                _ = LoadDirectoryAsync(dialog.SelectedPath);
            }
        }

        private void Refresh()
        {
            if (!string.IsNullOrEmpty(CurrentDirectory))
            {
                _ = LoadDirectoryAsync(CurrentDirectory);
            }
        }

        private void ClearCache()
        {
            _cacheService.ClearCache();
            OnPropertyChanged(nameof(CacheSizeFormatted));
            StatusMessage = "Cache cleared.";
        }

        private void OpenSelected()
        {
            if (SelectedItem != null)
            {
                ImageSelected?.Invoke(SelectedItem.FilePath);
            }
        }

        private void CancelLoading()
        {
            _loadingCts?.Cancel();
            _loadingCts?.Dispose();
            _loadingCts = null;
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
                CancelLoading();
                _cacheService.Dispose();

                foreach (var item in Thumbnails)
                {
                    item.Dispose();
                }
                Thumbnails.Clear();

                _disposed = true;
            }
            GC.SuppressFinalize(this);
        }

        ~CatalogViewModel()
        {
            Dispose();
        }

        #endregion
    }
}
