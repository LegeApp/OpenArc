using System;
using System.Collections.Generic;
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
        private readonly MediaStatsService _mediaStatsService;
        private CancellationTokenSource? _loadingCts;
        private DocBrake.Models.ProcessingOptions? _processingOptions;

        private string _currentDirectory = string.Empty;
        private string _statusMessage = "Select a folder to browse media files.";
        private bool _isLoading;
        private int _loadedCount;
        private int _totalCount;
        private ThumbnailItem? _selectedItem;
        private int _thumbnailSize = 200;
        private int _thumbnailPadding = 0;
        private double _viewportWidth = 1000; // Default width
        private int _columnsPerRow = 4; // Default columns
        private bool _disposed;
        private FolderItem? _selectedFolder;
        private bool _isDetailsView;
        private bool _isQueuePanelVisible = true;
        private bool _isPhoneModeActive;
        private PhoneDevice? _activePhoneDevice;
        private string _stagingDirectory = string.Empty;
        private bool _showThumbnailLabelsByDefault = true;

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
        
        public System.Collections.ObjectModel.ObservableCollection<DocBrake.Models.DocumentItem> QueueItems => _queueService.Items;
        
        public int QueueItemCount => _queueService.Count;
        
        public bool ShowThumbnailLabelsByDefault
        {
            get => _showThumbnailLabelsByDefault;
            set
            {
                if (_showThumbnailLabelsByDefault != value)
                {
                    _showThumbnailLabelsByDefault = value;
                    OnPropertyChanged();
                }
            }
        }

        public bool IsPhoneModeActive
        {
            get => _isPhoneModeActive;
            private set
            {
                if (_isPhoneModeActive != value)
                {
                    _isPhoneModeActive = value;
                    OnPropertyChanged();
                    OnPropertyChanged(nameof(PhoneModeStatusText));
                }
            }
        }

        public PhoneDevice? ActivePhoneDevice
        {
            get => _activePhoneDevice;
            private set
            {
                _activePhoneDevice = value;
                OnPropertyChanged();
                OnPropertyChanged(nameof(PhoneModeStatusText));
            }
        }

        public string StagingDirectory
        {
            get => _stagingDirectory;
            set
            {
                if (_stagingDirectory != value)
                {
                    _stagingDirectory = value;
                    OnPropertyChanged();
                }
            }
        }

        public string PhoneModeStatusText
        {
            get
            {
                if (!IsPhoneModeActive || ActivePhoneDevice == null)
                    return string.Empty;

                var icon = ActivePhoneDevice.DeviceCategory switch
                {
                    MobileDeviceType.Phone => "📱",
                    MobileDeviceType.SDCard => "💾",
                    MobileDeviceType.Camera => "📷",
                    _ => "📁"
                };

                return $"{icon} {ActivePhoneDevice.DeviceType}: {ActivePhoneDevice.Name}";
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

        public int ThumbnailPadding
        {
            get => _thumbnailPadding;
            set
            {
                _thumbnailPadding = Math.Max(0, Math.Min(20, value));
                OnPropertyChanged();
                OnPropertyChanged(nameof(ThumbnailMargin));
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

        // Padding for thumbnails (0 = wall-to-wall)
        public Thickness ThumbnailMargin => new Thickness(_thumbnailPadding);

        // Height for thumbnail image area - now uses full size since text is overlaid
        public int ThumbnailImageHeight => ThumbnailSize;

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

        /// <summary>
        /// Set the processing options to access settings like EnableThumbnails
        /// </summary>
        public void SetProcessingOptions(DocBrake.Models.ProcessingOptions options)
        {
            _processingOptions = options;
        }

        /// <summary>
        /// Check if thumbnails are enabled in settings
        /// </summary>
        private bool AreThumbnailsEnabled => _processingOptions?.EnableThumbnails ?? true;

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
        public ICommand ClearQueueCommand { get; }

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
            _mediaStatsService = new MediaStatsService();
            _queueService = queueService;

            // Subscribe to queue changes to update QueueItemCount
            _queueService.Items.CollectionChanged += (_, __) =>
            {
                OnPropertyChanged(nameof(QueueItemCount));
            };

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
            ClearQueueCommand = new RelayCommand(ClearQueueAndSelection);

            UpdateResponsiveLayout();
            LoadDrives();

            // Check for missing DLLs and display warning
            if (!string.IsNullOrEmpty(App.MissingDllMessage))
            {
                StatusMessage = App.MissingDllMessage;
            }
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
            // Group by path to handle any duplicates safely (take first of each group)
            var incoming = phones
                .Where(p => !string.IsNullOrWhiteSpace(p.Path))
                .GroupBy(p => p.Path, StringComparer.OrdinalIgnoreCase)
                .ToDictionary(g => g.Key, g => g.First(), StringComparer.OrdinalIgnoreCase);

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
            LoadDrives(hideDesktopDrives: false); // Never hide drives
        }

        private void LoadDrives(bool hideDesktopDrives)
        {
            try
            {
                // Preserve existing phone roots (MTP devices) before clearing
                var existingPhoneRoots = RootFolders
                    .Where(f => f.IsPhoneRoot)
                    .ToList();

                RootFolders.Clear();

                // Re-add phone roots first
                foreach (var phoneRoot in existingPhoneRoots)
                {
                    RootFolders.Add(phoneRoot);
                }

                // Note: Staging area is no longer shown separately - MTP devices browse directly

                var drives = DriveInfo.GetDrives()
                    .Where(d => d.IsReady)
                    .OrderByDescending(d => d.DriveType == DriveType.Removable)
                    .ThenBy(d => d.Name, StringComparer.OrdinalIgnoreCase)
                    .ToList();

                foreach (var drive in drives)
                {
                    // Always show all drives, both fixed and removable
                    // (Removed hideDesktopDrives logic)

                    try
                    {
                        var node = new FolderItem(drive.Name, true) 
                        { 
                            Name = $"{drive.VolumeLabel} ({drive.Name})" 
                        };

                        if (drive.DriveType == DriveType.Removable)
                        {
                            node.IsPhoneRoot = true;
                            // Set icon based on device type detection
                            var icon = GetDriveIcon(drive);
                            node.Icon = icon;
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

        private string GetDriveIcon(DriveInfo drive)
        {
            try
            {
                var rootPath = drive.RootDirectory.FullName;
                var volumeLabel = drive.VolumeLabel ?? string.Empty;

                // Check for SD card indicators
                var sdLabels = new[] { "SDHC", "SDXC", "SD CARD", "SDCARD" };
                if (sdLabels.Any(label => volumeLabel.Contains(label, StringComparison.OrdinalIgnoreCase)))
                    return "💾";

                // Check for camera indicators
                var cameraLabels = new[] { "EOS_DIGITAL", "CANON", "NIKON", "SONY", "LUMIX", "FUJI", "OLYMPUS", "PENTAX" };
                if (cameraLabels.Any(label => volumeLabel.Contains(label, StringComparison.OrdinalIgnoreCase)))
                    return "📷";

                // Check for Android folder = Phone
                if (Directory.Exists(Path.Combine(rootPath, "Android")))
                    return "📱";

                // Has DCIM but no Android = likely camera/SD
                if (Directory.Exists(Path.Combine(rootPath, "DCIM")))
                    return "📷";
            }
            catch { }

            return "📱"; // Default
        }

        /// <summary>
        /// Activate phone mode, hiding desktop drives and showing only phone/SD and staging folder.
        /// </summary>
        public void ActivatePhoneMode(PhoneDevice device, string stagingDirectory)
        {
            ActivePhoneDevice = device;
            StagingDirectory = stagingDirectory;
            IsPhoneModeActive = true;

            StatusMessage = $"Phone mode activated: {device.Name}";
            LoadDrives(hideDesktopDrives: false); // Show all drives
        }

        /// <summary>
        /// Deactivate phone mode and show all drives.
        /// </summary>
        public void DeactivatePhoneMode()
        {
            ActivePhoneDevice = null;
            IsPhoneModeActive = false;

            StatusMessage = "Phone mode deactivated";
            LoadDrives(hideDesktopDrives: false);
        }

        /// <summary>
        /// Refresh drives list maintaining current phone mode state.
        /// </summary>
        public void RefreshDrives()
        {
            LoadDrives(hideDesktopDrives: IsPhoneModeActive);
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
            int addedCount = 0;
            foreach (var item in Thumbnails)
            {
                if (!item.IsChecked)
                {
                    item.IsChecked = true;
                    // Add to queue (don't call OnThumbnailCheckedChanged to avoid multiple status updates)
                    string? pathToAdd = null;
                    
                    if (item.IsMtpFile && !string.IsNullOrEmpty(item.MtpDeviceId) && !string.IsNullOrEmpty(item.MtpObjectId))
                    {
                        var localPath = MtpFileService.GetLocalPath(item.MtpDeviceId, item.MtpObjectId, item.FileName);
                        if (!string.IsNullOrEmpty(localPath))
                        {
                            pathToAdd = localPath;
                            item.ResolvedLocalPath = localPath;
                        }
                        else
                        {
                            item.IsChecked = false; // Can't cache, revert
                            continue;
                        }
                    }
                    else if (!string.IsNullOrWhiteSpace(item.FilePath) && System.IO.File.Exists(item.FilePath))
                    {
                        pathToAdd = item.FilePath;
                        item.ResolvedLocalPath = item.FilePath;
                    }
                    
                    if (pathToAdd != null)
                    {
                        _queueService.AddFile(pathToAdd, item.FileName);
                        addedCount++;
                    }
                }
            }
            OnPropertyChanged(nameof(SelectedCount));
            OnPropertyChanged(nameof(SelectionSummary));
            StatusMessage = $"Selected all - {addedCount} files added to queue ({_queueService.Count} total)";
        }

        private void ClearSelection()
        {
            foreach (var item in Thumbnails)
            {
                item.IsChecked = false;
            }
            // Clear the queue as well since selection = queue
            _queueService.Clear();
            OnPropertyChanged(nameof(SelectedCount));
            OnPropertyChanged(nameof(SelectionSummary));
            StatusMessage = "Selection and queue cleared";
        }
        
        private void ClearQueueAndSelection()
        {
            // Same as ClearSelection - they're now unified
            ClearSelection();
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
        
        /// <summary>
        /// Handle checkbox state change - directly add/remove from queue
        /// </summary>
        public void OnThumbnailCheckedChanged(ThumbnailItem item)
        {
            OnPropertyChanged(nameof(SelectedCount));
            OnPropertyChanged(nameof(SelectionSummary));
            
            if (item.IsChecked)
            {
                // Add to queue
                string? pathToAdd = null;
                
                if (item.IsMtpFile && !string.IsNullOrEmpty(item.MtpDeviceId) && !string.IsNullOrEmpty(item.MtpObjectId))
                {
                    // For MTP files, resolve to local path
                    Console.WriteLine($"[QUEUE-ADD] MTP file: {item.FileName}, caching...");
                    var localPath = MtpFileService.GetLocalPath(item.MtpDeviceId, item.MtpObjectId, item.FileName);
                    if (!string.IsNullOrEmpty(localPath))
                    {
                        pathToAdd = localPath;
                        item.ResolvedLocalPath = localPath;
                        Console.WriteLine($"[QUEUE-ADD] Cached to: {localPath}");
                    }
                    else
                    {
                        Console.WriteLine($"[QUEUE-ADD] Failed to cache MTP file: {item.FileName}");
                        StatusMessage = $"Failed to cache {item.FileName}";
                        item.IsChecked = false; // Revert checkbox
                        return;
                    }
                }
                else if (!string.IsNullOrWhiteSpace(item.FilePath) && System.IO.File.Exists(item.FilePath))
                {
                    pathToAdd = item.FilePath;
                    item.ResolvedLocalPath = item.FilePath;
                }
                else
                {
                    Console.WriteLine($"[QUEUE-ADD] File doesn't exist: {item.FilePath}");
                    item.IsChecked = false;
                    return;
                }
                
                if (pathToAdd != null)
                {
                    // Add with original display name
                    _queueService.AddFile(pathToAdd, item.FileName);
                    StatusMessage = $"Added {item.FileName} to queue ({_queueService.Count} total)";
                }
            }
            else
            {
                // Remove from queue
                var pathToRemove = item.ResolvedLocalPath ?? item.FilePath;
                if (!string.IsNullOrEmpty(pathToRemove))
                {
                    _queueService.RemoveByPath(pathToRemove);
                    StatusMessage = $"Removed {item.FileName} from queue ({_queueService.Count} total)";
                }
            }
        }

        public void NotifyFolderCheckChanged(FolderItem folder)
        {
            FolderCheckChanged?.Invoke(folder.FullPath, folder.IsChecked);
        }

        private void AddPreparedToQueue()
        {
            var selected = Thumbnails.Where(x => x.IsChecked).ToList();
            Console.WriteLine($"[MTP-QUEUE] Adding {selected.Count} selected items to queue");
            
            int addedCount = 0;
            foreach (var item in selected)
            {
                string pathToAdd = item.FilePath;
                
                // For MTP files, resolve to local path first
                if (item.IsMtpFile && !string.IsNullOrEmpty(item.MtpDeviceId) && !string.IsNullOrEmpty(item.MtpObjectId))
                {
                    Console.WriteLine($"[MTP-QUEUE] MTP file detected: {item.FileName}, DeviceId={item.MtpDeviceId}, ObjectId={item.MtpObjectId}");
                    var localPath = MtpFileService.GetLocalPath(item.MtpDeviceId, item.MtpObjectId, item.FileName);
                    if (!string.IsNullOrEmpty(localPath))
                    {
                        pathToAdd = localPath;
                        Console.WriteLine($"[MTP-QUEUE] Resolved {item.FileName} to local: {pathToAdd}, exists: {System.IO.File.Exists(pathToAdd)}");
                    }
                    else
                    {
                        StatusMessage = $"Failed to download {item.FileName} from device";
                        Console.WriteLine($"[MTP-QUEUE] FAILED to cache {item.FileName}");
                        continue;
                    }
                }
                else
                {
                    Console.WriteLine($"[MTP-QUEUE] Regular file: {item.FileName}, path: {pathToAdd}, exists: {System.IO.File.Exists(pathToAdd)}");
                }
                
                int beforeCount = _queueService.Count;
                _queueService.AddFile(pathToAdd);
                int afterCount = _queueService.Count;
                
                if (afterCount > beforeCount)
                {
                    addedCount++;
                    Console.WriteLine($"[MTP-QUEUE] Successfully added to queue: {pathToAdd}");
                }
                else
                {
                    Console.WriteLine($"[MTP-QUEUE] File was NOT added (rejected or duplicate): {pathToAdd}");
                }
            }
            
            Console.WriteLine($"[MTP-QUEUE] Queue now has {_queueService.Count} items (added {addedCount})");
            StatusMessage = $"Added {addedCount} file(s) to queue";
            
            // Clear selection after adding to queue
            foreach (var item in selected)
            {
                item.IsChecked = false;
            }
            OnPropertyChanged(nameof(SelectedCount));
            OnPropertyChanged(nameof(SelectionSummary));
            PreparedItemsView.Refresh();
        }



        #endregion

        #region Public Methods

        /// <summary>
        /// Load BPG files from a directory
        /// </summary>
        public async Task LoadDirectoryAsync(string directoryPath)
        {
            bool isMtpPath = MtpFileService.IsMtpPath(directoryPath);
            
            if (!isMtpPath && !Directory.Exists(directoryPath))
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
            StatusMessage = isMtpPath ? "Scanning MTP device..." : "Scanning for files...";

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
                        IEnumerable<(string filePath, string? mtpDeviceId, string? mtpObjectId, string name, long size)> files;
                        
                        if (isMtpPath)
                        {
                            // Parse MTP path: mtp://deviceId[/objectId]
                            files = GetMtpFiles(directoryPath);
                        }
                        else
                        {
                            // Regular filesystem
                            files = Directory.EnumerateFiles(directoryPath, "*.*", SearchOption.TopDirectoryOnly)
                                             .OrderBy(f => f, StringComparer.OrdinalIgnoreCase)
                                             .Select(f => (f, (string?)null, (string?)null, Path.GetFileName(f), 0L));
                        }

                        foreach (var (filePath, mtpDeviceId, mtpObjectId, name, size) in files)
                        {
                            if (token.IsCancellationRequested) break;

                            var item = new ThumbnailItem(filePath)
                            {
                                MtpDeviceId = mtpDeviceId,
                                MtpObjectId = mtpObjectId,
                                FileName = name
                            };
                            
                            if (isMtpPath)
                            {
                                item.FileSize = size;
                            }
                            
                            batch.Add(item);
                            count++;

                            // Batch update UI every 50 items
                            if (batch.Count >= 50)
                            {
                                var currentBatch = batch.ToList(); // Copy
                                batch.Clear();
                                
                                Application.Current.Dispatcher.Invoke(() =>
                                {
                                    foreach (var batchItem in currentBatch)
                                    {
                                        Thumbnails.Add(batchItem);
                                    }
                                    TotalCount = Thumbnails.Count;
                                    StatusMessage = isMtpPath 
                                        ? $"Found {TotalCount} files on device..." 
                                        : $"Found {TotalCount} files...";
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

                StatusMessage = isMtpPath 
                    ? $"Loading thumbnails from device ({Thumbnails.Count} files)..." 
                    : $"Loading thumbnails for {Thumbnails.Count} files...";

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
            if (IsImageFile(item.FileName) || IsVideoFile(item.FileName))
            {
                // For MTP files, we need to provide a local path if possible
                string path = item.IsMtpFile && !string.IsNullOrEmpty(item.MtpDeviceId)
                    ? DocBrake.Services.MtpFileService.GetLocalPath(item.MtpDeviceId, item.MtpObjectId, item.FileName) ?? item.FilePath
                    : item.FilePath;
                
                ImageSelected?.Invoke(path);
                return;
            }

            try
            {
                string path = item.FilePath;
                if (item.IsMtpFile && !string.IsNullOrEmpty(item.MtpDeviceId))
                {
                    StatusMessage = $"Downloading {item.FileName}...";
                    path = DocBrake.Services.MtpFileService.GetLocalPath(item.MtpDeviceId, item.MtpObjectId, item.FileName);
                    if (path == null)
                    {
                        StatusMessage = "Failed to download file from device";
                        return;
                    }
                }
                
                Process.Start(new ProcessStartInfo(path) { UseShellExecute = true });
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
            // Use Parallel.ForEachAsync for proper bounded concurrency.
            // This avoids creating 650+ tasks upfront (which caused thread pool starvation
            // and the "stops at ~90" bug with the old Select+WhenAll pattern).
            await Parallel.ForEachAsync(
                Thumbnails,
                new ParallelOptions { MaxDegreeOfParallelism = 8, CancellationToken = cancellationToken },
                async (item, ct) =>
            {
                try
                {
                    if (ct.IsCancellationRequested) return;

                    // Check if thumbnails are disabled - if so, gather stats instead
                    if (!AreThumbnailsEnabled)
                    {
                        item.IsStatsMode = true;
                        item.IsLoading = true;

                        // Gather file stats
                        var stats = await _mediaStatsService.GatherStatsAsync(item.FilePath);

                        await Application.Current.Dispatcher.InvokeAsync(() =>
                        {
                            item.StatsText = stats.DisplayText;
                            item.IsLoading = false;
                            item.HasError = stats.HasError;
                            if (stats.HasError)
                                item.ErrorMessage = stats.ErrorMessage;
                        });
                    }
                    else
                    {
                        item.IsStatsMode = false;

                        // 1. Handle MTP Files specifically
                        if (MtpFileService.IsMtpPath(item.FilePath) && !string.IsNullOrEmpty(item.MtpDeviceId) && !string.IsNullOrEmpty(item.MtpObjectId))
                        {
                            Console.WriteLine($"[MTP-THUMB] Processing: {item.FileName}, DeviceId={item.MtpDeviceId}, ObjectId={item.MtpObjectId}");

                            bool ok = await _cacheService.LoadThumbnailAsync(item, ct);
                            if (!ok)
                            {
                                Console.WriteLine($"[MTP-THUMB] Thumbnail generation failed for {item.FileName}: {item.ErrorMessage}");
                            }
                        }
                        else
                        {
                            // 2. Handle Standard Files (or local files)
                            if (IsImageFile(item.FileName))
                            {
                                await _cacheService.LoadThumbnailAsync(item, ct);
                            }
                            else
                            {
                                await Task.Run(() => _fileThumbnailService.TryLoadThumbnail(item, _cacheServiceThumbnailSize), ct);
                            }
                        }
                    }

                    // Update Progress
                    await Application.Current.Dispatcher.InvokeAsync(() =>
                    {
                        LoadedCount++;
                        if (LoadedCount % 10 == 0 || LoadedCount == TotalCount)
                        {
                            var action = AreThumbnailsEnabled ? "thumbnails" : "stats";
                            StatusMessage = $"Loading {action}... {LoadedCount}/{TotalCount}";
                        }
                    });
                }
                catch (OperationCanceledException)
                {
                    // Expected on navigation/cancel
                }
                catch (Exception ex)
                {
                    Debug.WriteLine($"Error loading thumb: {ex.Message}");
                }
            });
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

        #region Helper Methods

        /// <summary>
        /// Check if a path is an MTP path (uses MtpFileService)
        /// </summary>
        private static bool IsMtpShellPath(string path) => MtpFileService.IsMtpPath(path);

        /// <summary>
        /// Get files from an MTP folder path
        /// Returns tuples of (localPath, deviceId, objectId, fileName, size)
        /// </summary>
        private static IEnumerable<(string filePath, string? mtpDeviceId, string? mtpObjectId, string name, long size)> GetMtpFiles(string mtpPath)
        {
            // Parse: mtp://deviceId[/objectId]
            if (!mtpPath.StartsWith("mtp://", StringComparison.OrdinalIgnoreCase))
                yield break;

            var uri = mtpPath.Substring(6);
            var parts = uri.Split(new[] { '/' }, 2);
            var deviceId = parts[0];
            var objectId = parts.Length > 1 ? parts[1] : null;

            Console.WriteLine($"[MTP] GetMtpFiles: Path={mtpPath} Device={deviceId} Object={objectId ?? "(root)"}");

            var result = DocBrake.NativeInterop.OpenArcFFI.GetMtpFolderContents(deviceId, objectId);
            if (!result.success || result.data == null)
            {
                Console.WriteLine($"[MTP] Failed to get contents: {result.error}");
                yield break;
            }

            var items = result.data.Where(o => !o.is_folder).OrderBy(o => o.name).ToList();
            Console.WriteLine($"[MTP] Found {items.Count} files in {mtpPath}");

            foreach (var item in items)
            {
                // Return MTP URI as path (will be resolved to local temp path when needed)
                var itemPath = $"mtp://{deviceId}/{item.id}";
                yield return (itemPath, deviceId, item.id, item.name, (long)item.size);
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
