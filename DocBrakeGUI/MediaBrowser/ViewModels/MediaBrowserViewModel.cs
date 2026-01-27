using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Data;
using System.Windows.Input;
using DocBrake.Commands;
using DocBrake.MediaBrowser.Models;
using DocBrake.MediaBrowser.Services;
using DocBrake.Services;

namespace DocBrake.MediaBrowser.ViewModels
{
    /// <summary>
    /// ViewModel for the media browser / thumbnail catalog view
    /// </summary>
    public class MediaBrowserViewModel : INotifyPropertyChanged, IDisposable
    {
        #region Fields

        private readonly ThumbnailCacheService _cacheService;
        private readonly FileThumbnailService _fileThumbnailService;
        private readonly IQueueService _queueService;
        private CancellationTokenSource? _loadingCts;

        private string _currentDirectory = string.Empty;
        private string _statusMessage = "Select a folder to browse media files.";
        private bool _isLoading;
        private int _loadedCount;
        private int _totalCount;
        private ThumbnailItem? _selectedItem;
        private int _thumbnailSize = 200;
        private double _viewportWidth = 1000; // Default width
        private int _columnsPerRow = 4; // Default columns
        private bool _disposed;
        private FolderItem? _selectedFolder;
        private bool _isDetailsView;
        private bool _isQueuePanelVisible = true;

        #endregion

        #region Properties

        public ObservableCollection<ThumbnailItem> Thumbnails { get; } = new();
        public ObservableCollection<FolderItem> RootFolders { get; } = new();

        public ICollectionView PreparedItemsView { get; }

        public bool IsDetailsView
        {
            get => _isDetailsView;
            set
            {
                if (_isDetailsView != value)
                {
                    _isDetailsView = value;
                    OnPropertyChanged();
                }
            }
        }

        public bool IsQueuePanelVisible
        {
            get => _isQueuePanelVisible;
            set
            {
                if (_isQueuePanelVisible != value)
                {
                    _isQueuePanelVisible = value;
                    OnPropertyChanged();
                }
            }
        }

        // Selection management for batch processing
        public int SelectedCount => Thumbnails.Count(x => x.IsChecked);
        public string SelectionSummary => $"{SelectedCount} file{(SelectedCount != 1 ? "s" : "")} selected";

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
        public event Action<string, bool?>? FolderCheckChanged;


        #endregion

        #region Commands

        public ICommand OpenFileCommand { get; }
        public ICommand OpenFolderCommand { get; }
        public ICommand RefreshCommand { get; }
        public ICommand ClearCacheCommand { get; }
        public ICommand OpenSelectedCommand { get; }
        public ICommand IncreaseSizeCommand { get; }
        public ICommand DecreaseSizeCommand { get; }
        public ICommand SelectAllCommand { get; }
        public ICommand ClearSelectionCommand { get; }
        public ICommand ProcessSelectedCommand { get; }
        public ICommand ToggleDetailsViewCommand { get; }
        public ICommand ToggleQueuePanelCommand { get; }
        public ICommand AddPreparedToQueueCommand { get; }

        #endregion

        #region Constructor

        public MediaBrowserViewModel()
            : this(new ThumbnailCacheService(256, 256, 8), new QueueService())
        {
        }

        // Constructor with injected cache service
        public MediaBrowserViewModel(ThumbnailCacheService cacheService, IQueueService queueService)
        {
            _cacheService = cacheService;
            _fileThumbnailService = new FileThumbnailService();
            _queueService = queueService;

            PreparedItemsView = new ListCollectionView(Thumbnails)
            {
                Filter = o => o is ThumbnailItem t && t.IsChecked
            };

            OpenFileCommand = new RelayCommand(OpenFile);
            OpenFolderCommand = new RelayCommand(OpenFolder);
            RefreshCommand = new RelayCommand(Refresh, () => !string.IsNullOrEmpty(CurrentDirectory));
            ClearCacheCommand = new RelayCommand(ClearCache);
            OpenSelectedCommand = new RelayCommand(OpenSelected, () => SelectedItem != null);
            IncreaseSizeCommand = new RelayCommand(() => AdjustThumbnailSize(20));
            DecreaseSizeCommand = new RelayCommand(() => AdjustThumbnailSize(-20));
            SelectAllCommand = new RelayCommand(SelectAll);
            ClearSelectionCommand = new RelayCommand(ClearSelection);
            ProcessSelectedCommand = new RelayCommand(ProcessSelected, () => SelectedCount > 0);
            ToggleDetailsViewCommand = new RelayCommand(() => IsDetailsView = !IsDetailsView);

            ToggleQueuePanelCommand = new RelayCommand(() => IsQueuePanelVisible = !IsQueuePanelVisible);
            AddPreparedToQueueCommand = new RelayCommand(AddPreparedToQueue, () => SelectedCount > 0);

            UpdateResponsiveLayout();
            LoadDrives();
        }

        public System.Collections.Generic.IReadOnlyList<string> GetCheckedFolderPaths()
        {
            var results = new System.Collections.Generic.List<string>();

            void Walk(FolderItem node)
            {
                if (node.IsChecked == true && !string.IsNullOrWhiteSpace(node.FullPath))
                {
                    results.Add(node.FullPath);
                }

                foreach (var child in node.SubFolders)
                {
                    Walk(child);
                }
            }

            foreach (var root in RootFolders)
            {
                Walk(root);
            }

            return results
                .Where(p => !string.IsNullOrWhiteSpace(p))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .ToList();
        }

        public void SyncPhoneRoots(System.Collections.Generic.IReadOnlyList<PhoneDevice> phones)
        {
            var incoming = phones
                .Where(p => !string.IsNullOrWhiteSpace(p.Path))
                .ToDictionary(p => p.Path, p => p, StringComparer.OrdinalIgnoreCase);

            for (int i = RootFolders.Count - 1; i >= 0; i--)
            {
                var existing = RootFolders[i];
                if (existing.IsPhoneRoot)
                {
                    if (!incoming.ContainsKey(existing.FullPath))
                    {
                        RootFolders.RemoveAt(i);
                    }
                }
            }

            foreach (var kvp in incoming)
            {
                var path = kvp.Key;
                var phone = kvp.Value;

                var existing = RootFolders.FirstOrDefault(r => r.IsPhoneRoot && string.Equals(r.FullPath, path, StringComparison.OrdinalIgnoreCase));
                if (existing != null)
                {
                    existing.Name = phone.Name;
                    existing.Icon = "📱";
                    existing.IsPhoneRoot = true;
                    continue;
                }

                var node = new FolderItem(path, true)
                {
                    Name = phone.Name,
                    Icon = "📱",
                    IsPhoneRoot = true
                };

                RootFolders.Insert(0, node);
            }
        }

        private void LoadDrives()
        {
            try
            {
                RootFolders.Clear();

                var drives = DriveInfo.GetDrives()
                    .Where(d => d.IsReady)
                    .OrderByDescending(d => d.DriveType == DriveType.Removable)
                    .ThenBy(d => d.Name, StringComparer.OrdinalIgnoreCase)
                    .ToList();

                foreach (var drive in drives)
                {
                    try
                    {
                        var node = new FolderItem(drive.Name, true) 
                        { 
                            Name = $"{drive.VolumeLabel} ({drive.Name})" 

                        };

                        if (drive.DriveType == DriveType.Removable)
                        {
                            node.IsPhoneRoot = true;
                            node.Icon = "📱";
                        }

                        RootFolders.Add(node);
                    }
                    catch
                    {
                        var node = new FolderItem(drive.Name, true);
                        if (drive.DriveType == DriveType.Removable)
                        {
                            node.IsPhoneRoot = true;
                            node.Icon = "📱";
                        }
                        RootFolders.Add(node);
                    }
                }

                var defaultRoot = RootFolders
                    .OrderByDescending(r => r.IsPhoneRoot)
                    .ThenBy(r => r.FullPath, StringComparer.OrdinalIgnoreCase)
                    .FirstOrDefault();

                if (defaultRoot != null)
                {
                    defaultRoot.IsExpanded = true;
                    defaultRoot.IsSelected = true;

                    var dcim = defaultRoot.SubFolders.FirstOrDefault(f =>
                        string.Equals(f.Name, "DCIM", StringComparison.OrdinalIgnoreCase));
                    if (dcim != null)
                    {
                        dcim.IsSelected = true;
                        SelectedFolder = dcim;
                    }
                    else
                    {
                        SelectedFolder = defaultRoot;
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

        #region Selection Methods

        private void SelectAll()
        {
            foreach (var item in Thumbnails)
            {
                item.IsChecked = true;
            }
            OnPropertyChanged(nameof(SelectedCount));
            OnPropertyChanged(nameof(SelectionSummary));
        }

        private void ClearSelection()
        {
            foreach (var item in Thumbnails)
            {
                item.IsChecked = false;
            }
            OnPropertyChanged(nameof(SelectedCount));
            OnPropertyChanged(nameof(SelectionSummary));
        }

        private void ProcessSelected()
        {
            var selectedFiles = Thumbnails.Where(x => x.IsChecked).Select(x => x.FilePath).ToList();
            if (selectedFiles.Count > 0)
            {
                StatusMessage = $"Processing {selectedFiles.Count} files...";
                // TODO: Integrate with OpenArcProcessingService for actual processing
            }
        }

        /// <summary>
        /// Update selection count when item selection changes
        /// </summary>
        public void NotifySelectionChanged()
        {
            OnPropertyChanged(nameof(SelectedCount));
            OnPropertyChanged(nameof(SelectionSummary));
        }

        public void NotifyFolderCheckChanged(FolderItem folder)
        {
            FolderCheckChanged?.Invoke(folder.FullPath, folder.IsChecked);
        }

        private void AddPreparedToQueue()
        {
            var selected = Thumbnails.Where(x => x.IsChecked).ToList();
            foreach (var item in selected)
            {
                _queueService.AddFile(item.FilePath);
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
            StatusMessage = "Scanning for files...";

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
                        var files = Directory.EnumerateFiles(directoryPath, "*.*", SearchOption.TopDirectoryOnly)
                                             .OrderBy(f => f, StringComparer.OrdinalIgnoreCase);

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
                                    StatusMessage = $"Found {TotalCount} files...";
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
                    StatusMessage = "No files found in this folder.";
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
        /// Handle dropped files
        /// </summary>
        public void HandleDroppedFiles(string[] files)
        {
            if (files == null || files.Length == 0)
                return;

            var dirs = files.Where(Directory.Exists).ToList();
            var directFiles = files.Where(File.Exists).ToList();

            if (dirs.Count == 1 && directFiles.Count == 0)
            {
                _ = LoadDirectoryAsync(dirs[0]);
                return;
            }

            var allFiles = new System.Collections.Generic.List<string>();
            allFiles.AddRange(directFiles);

            foreach (var dir in dirs)
            {
                try
                {
                    allFiles.AddRange(Directory.EnumerateFiles(dir, "*.*", SearchOption.AllDirectories));
                }
                catch
                {
                }
            }

            if (allFiles.Count == 0)
            {
                StatusMessage = "No files found in drop.";
                return;
            }

            foreach (var file in allFiles.Distinct(StringComparer.OrdinalIgnoreCase))
            {
                if (Thumbnails.Any(t => t.FilePath.Equals(file, StringComparison.OrdinalIgnoreCase)))
                    continue;

                var item = new ThumbnailItem(file);
                Thumbnails.Add(item);

                if (IsImageFile(file))
                {
                    _ = _cacheService.LoadThumbnailAsync(item);
                }
                else
                {
                    _ = Task.Run(() => _fileThumbnailService.TryLoadThumbnail(item, _cacheServiceThumbnailSize));
                }
            }

            TotalCount = Thumbnails.Count;
            StatusMessage = $"Added {allFiles.Count} files. Total: {TotalCount}";
        }

        /// <summary>
        /// Handle thumbnail double-click
        /// </summary>
        public void OnThumbnailDoubleClick(ThumbnailItem item)
        {
            if (IsImageFile(item.FilePath) || IsVideoFile(item.FilePath))
            {
                ImageSelected?.Invoke(item.FilePath);
                return;
            }

            try
            {
                Process.Start(new ProcessStartInfo(item.FilePath) { UseShellExecute = true });
            }
            catch (Exception ex)
            {
                StatusMessage = $"Unable to open file: {ex.Message}";
            }
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

                bool success;
                if (IsImageFile(item.FilePath))
                {
                    success = await _cacheService.LoadThumbnailAsync(item, cancellationToken);
                }
                else
                {
                    success = await Task.Run(() => _fileThumbnailService.TryLoadThumbnail(item, _cacheServiceThumbnailSize), cancellationToken);
                }

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

        private int _cacheServiceThumbnailSize => 256;

        private static bool IsBpgFile(string filePath) =>
            filePath.EndsWith(".bpg", StringComparison.OrdinalIgnoreCase);

        private static bool IsImageFile(string filePath)
        {
            var ext = Path.GetExtension(filePath).ToLowerInvariant();
            return ext switch
            {
                ".bpg" or ".jpg" or ".jpeg" or ".png" or ".tiff" or ".tif" or ".bmp" or ".webp" or 
                ".gif" or ".ico" or ".pnm" or ".pbm" or ".pgm" or ".ppm" or ".pam" or ".ff" or 
                ".farbfeld" or ".dds" or ".tga" or ".hdr" or ".exr" or ".heic" or ".heif" or 
                ".cr2" or ".nef" or ".arw" or ".orf" or ".rw2" or ".raf" or ".3fr" or ".fff" or 
                ".dcr" or ".kdc" or ".srf" or ".sr2" or ".erf" or ".mef" or ".mrw" or ".nrw" or 
                ".pef" or ".iiq" or ".x3f" or ".dng" or 
                ".jp2" or ".j2k" or ".j2c" or ".jpc" or ".jpt" or ".jph" or ".jhc" => true,
                _ => false
            };
        }

        private static bool IsVideoFile(string filePath)
        {
            var ext = Path.GetExtension(filePath).ToLowerInvariant();
            return ext is ".mp4" or ".mov" or ".m4v" or ".avi" or ".mkv" or ".wmv";
        }

        private void OpenFile()
        {
            var dialog = new Microsoft.Win32.OpenFileDialog
            {
                Filter = "All Files|*.*",
                Title = "Open File"
            };

            if (dialog.ShowDialog() == true)
            {
                if (IsImageFile(dialog.FileName) || IsVideoFile(dialog.FileName))
                {
                    ImageSelected?.Invoke(dialog.FileName);
                    return;
                }

                try
                {
                    Process.Start(new ProcessStartInfo(dialog.FileName) { UseShellExecute = true });
                }
                catch (Exception ex)
                {
                    StatusMessage = $"Unable to open file: {ex.Message}";
                }
            }
        }

        private void OpenFolder()
        {
            var dialog = new Microsoft.Win32.OpenFolderDialog
            {
                Title = "Select a folder",
                Multiselect = false
            };

            if (dialog.ShowDialog() == true)
            {
                _ = LoadDirectoryAsync(dialog.FolderName);
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

        ~MediaBrowserViewModel()
        {
            Dispose();
        }

        #endregion
    }
}
