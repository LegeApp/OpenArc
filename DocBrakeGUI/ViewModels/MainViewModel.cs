using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Input;
using Microsoft.Extensions.Logging;
using DocBrake.Commands;
using DocBrake.Models;
using DocBrake.Services;
using DocBrake.NativeInterop;
using DocBrake.MediaBrowser.ViewModels;
using DocBrake.MediaBrowser.Views;
using DocBrake.Views;
using Microsoft.Extensions.DependencyInjection;

namespace DocBrake.ViewModels
{
    public class MainViewModel : INotifyPropertyChanged
    {
        private readonly IDocumentProcessingService _processingService;
        private readonly IQueueService _queueService;
        private readonly ISettingsService _settingsService;
        private readonly IFileDialogService _fileDialogService;
        private readonly ILogger<MainViewModel> _logger;
        private readonly IPhoneDetectionService? _phoneDetectionService;
        private readonly IStagingService? _stagingService;
        private CancellationTokenSource? _cancellationTokenSource;

        public QueueViewModel QueueViewModel { get; }
        private ProcessingOptions _processingOptions;
        private bool _isProcessing;
        private double _overallProgress;
        private string _statusMessage = "Ready";
        private bool _showSettings;
        private bool _queueVisible;
        private bool _isErrorState;
        private string _currentFileName = string.Empty;
        private int _currentFileIndex;
        private int _totalFileCount;
        private string _estimatedTimeRemaining = string.Empty;
        private DateTime _processingStartTime;
        private SettingsViewModel? _settingsViewModel;
        private ComputeCapabilityViewModel _computeCapabilityViewModel;
        private ArchiveTrackingViewModel _archiveTrackingViewModel;
        private ArchiveMode _selectedMode;

        private readonly MediaBrowserViewModel _mediaBrowserViewModel;
        private readonly MediaViewerViewModel _mediaViewerViewModel;

        private readonly HashSet<string> _phoneArchivePromptedThisSession = new(StringComparer.OrdinalIgnoreCase);

        public event PropertyChangedEventHandler? PropertyChanged;

        public MainViewModel(
            IDocumentProcessingService processingService,
            IQueueService queueService,
            QueueViewModel queueViewModel,
            ISettingsService settingsService,
            IFileDialogService fileDialogService,
            ILogger<MainViewModel> logger,
            MediaBrowserViewModel mediaBrowserViewModel,
            MediaViewerViewModel mediaViewerViewModel,
            IPhoneDetectionService? phoneDetectionService = null,
            IStagingService? stagingService = null)
        {
            _processingService = processingService ?? throw new ArgumentNullException(nameof(processingService));
            _queueService = queueService ?? throw new ArgumentNullException(nameof(queueService));
            QueueViewModel = queueViewModel ?? throw new ArgumentNullException(nameof(queueViewModel));
            _settingsService = settingsService ?? throw new ArgumentNullException(nameof(settingsService));
            _fileDialogService = fileDialogService ?? throw new ArgumentNullException(nameof(fileDialogService));
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
            _mediaBrowserViewModel = mediaBrowserViewModel ?? throw new ArgumentNullException(nameof(mediaBrowserViewModel));
            _mediaViewerViewModel = mediaViewerViewModel ?? throw new ArgumentNullException(nameof(mediaViewerViewModel));
            _phoneDetectionService = phoneDetectionService;
            _stagingService = stagingService;

            _processingOptions = _settingsService.LoadSettings();
            _selectedMode = _processingOptions.ArchiveMode;
            if (_selectedMode == ArchiveMode.Standard || _selectedMode == ArchiveMode.Phone)
            {
                _selectedMode = ArchiveMode.MediaBrowser;
                _processingOptions.ArchiveMode = _selectedMode;
            }
            _computeCapabilityViewModel = new ComputeCapabilityViewModel();

            // Create SettingsViewModel with shared ProcessingOptions
            _settingsViewModel = new SettingsViewModel(_fileDialogService, _settingsService);
            // Sync the options - SettingsViewModel should use the same instance
            _settingsViewModel.SyncOptions(_processingOptions);

            // Create ArchiveTrackingViewModel
            var archiveTrackingService = new ArchiveTrackingService();
            _archiveTrackingViewModel = new ArchiveTrackingViewModel(archiveTrackingService, _fileDialogService, GetCatalogDbPath());

            // Initialize phone detection if available
            if (_phoneDetectionService != null)
            {
                _phoneDetectionService.PhoneConnected += OnPhoneConnected;
                _phoneDetectionService.PhoneDisconnected += OnPhoneDisconnected;
                if (_processingOptions.AutoDetectPhone)
                {
                    _phoneDetectionService.StartDetection();
                }

                try
                {
                    _mediaBrowserViewModel.SyncPhoneRoots(_phoneDetectionService.GetConnectedPhones());
                }
                catch
                {
                }
            }

            _mediaBrowserViewModel.ImageSelected += OpenImageViewerWindow;
            _mediaBrowserViewModel.FolderCheckChanged += OnFolderCheckChanged;

            InitializeCommands();
            SubscribeToEvents();
        }

        public MediaBrowserViewModel MediaBrowserViewModel => _mediaBrowserViewModel;
        public MediaViewerViewModel MediaViewerViewModel => _mediaViewerViewModel;

        private void OpenImageViewerWindow(string filePath)
        {
            if (string.IsNullOrWhiteSpace(filePath))
                return;

            Application.Current.Dispatcher.Invoke(() =>
            {
                var window = new MediaImageViewerWindow(filePath)
                {
                    Owner = Application.Current.MainWindow
                };
                window.Show();
            });
        }

        // Properties
        public ObservableCollection<DocumentItem> QueueFiles => _queueService.Items;
        
        public DocumentItem? SelectedFile
        {
            get => QueueViewModel.SelectedItem;
            set
            {
                QueueViewModel.SelectedItem = value;
                OnPropertyChanged();
            }
        }

        public ProcessingOptions ProcessingOptions
        {
            get => _processingOptions;
            set => SetProperty(ref _processingOptions, value);
        }

        public bool IsProcessing
        {
            get => _isProcessing;
            private set => SetProperty(ref _isProcessing, value);
        }

        public double OverallProgress
        {
            get => _overallProgress;
            private set => SetProperty(ref _overallProgress, value);
        }

        public string StatusMessage
        {
            get => _statusMessage;
            private set => SetProperty(ref _statusMessage, value);
        }

        public bool ShowSettings
        {
            get => _showSettings;
            set => SetProperty(ref _showSettings, value);
        }
        
        public bool QueueVisible
        {
            get => _queueVisible;
            set => SetProperty(ref _queueVisible, value);
        }
        
        public bool IsErrorState
        {
            get => _isErrorState;
            private set => SetProperty(ref _isErrorState, value);
        }

        public string CurrentFileName
        {
            get => _currentFileName;
            private set => SetProperty(ref _currentFileName, value);
        }

        public int CurrentFileIndex
        {
            get => _currentFileIndex;
            private set => SetProperty(ref _currentFileIndex, value);
        }

        public int TotalFileCount
        {
            get => _totalFileCount;
            private set => SetProperty(ref _totalFileCount, value);
        }

        public string ProgressText => TotalFileCount > 0
            ? $"{CurrentFileIndex}/{TotalFileCount}"
            : string.Empty;

        public string EstimatedTimeRemaining
        {
            get => _estimatedTimeRemaining;
            private set => SetProperty(ref _estimatedTimeRemaining, value);
        }

        public SettingsViewModel? SettingsViewModel
        {
            get => _settingsViewModel;
            private set => SetProperty(ref _settingsViewModel, value);
        }
        
        public ComputeCapabilityViewModel ComputeCapabilityViewModel
        {
            get => _computeCapabilityViewModel;
            private set => SetProperty(ref _computeCapabilityViewModel, value);
        }

        public ArchiveTrackingViewModel ArchiveTrackingViewModel
        {
            get => _archiveTrackingViewModel;
            private set => SetProperty(ref _archiveTrackingViewModel, value);
        }

        public ArchiveMode SelectedMode
        {
            get => _selectedMode;
            set
            {
                if (SetProperty(ref _selectedMode, value))
                {
                    _processingOptions.ArchiveMode = value;
                    OnPropertyChanged(nameof(IsPhoneMode));
                    OnPropertyChanged(nameof(IsStandardMode));
                    OnPropertyChanged(nameof(IsMediaBrowserMode));
                }
            }
        }

        public bool IsPhoneMode => SelectedMode == ArchiveMode.Phone;
        public bool IsStandardMode => SelectedMode == ArchiveMode.Standard;
        public bool IsMediaBrowserMode => SelectedMode == ArchiveMode.MediaBrowser || SelectedMode == ArchiveMode.Standard;
        
        // Commands
        public ICommand AddFileCommand { get; private set; } = null!;
        public ICommand AddFolderCommand { get; private set; } = null!;
        public ICommand RemoveFileCommand { get; private set; } = null!;
        public ICommand ClearQueueCommand { get; private set; } = null!;
        public ICommand StartProcessingCommand { get; private set; } = null!;
        public ICommand CancelProcessingCommand { get; private set; } = null!;
        public ICommand ShowQueueCommand { get; private set; } = null!;
        public ICommand ShowSettingsCommand { get; private set; } = null!;
        public ICommand SaveSettingsCommand { get; private set; } = null!;
        public ICommand SwitchToPhoneModeCommand { get; private set; } = null!;
        public ICommand SwitchToStandardModeCommand { get; private set; } = null!;
        public ICommand SwitchToMediaBrowserModeCommand { get; private set; } = null!;
        public ICommand ExtractArchiveCommand { get; private set; } = null!;
        public ICommand ListArchiveCommand { get; private set; } = null!;
        public ICommand TestEncodeCommand { get; private set; } = null!;

        private void InitializeCommands()
        {
            AddFileCommand = new RelayCommand(_ => AddFile());
            AddFolderCommand = new RelayCommand(_ => AddFolder());
            RemoveFileCommand = new RelayCommand(_ => RemoveFile(), _ => CanRemoveFile());
            ClearQueueCommand = new RelayCommand(_ => ClearQueue(), _ => CanClearQueue());
            StartProcessingCommand = new RelayCommand(async _ => await StartProcessingAsync(), _ => CanStartProcessing());
            CancelProcessingCommand = new RelayCommand(_ => CancelProcessing(), _ => CanCancelProcessing());
            ShowQueueCommand = new RelayCommand(_ => ShowQueue());
            ShowSettingsCommand = new RelayCommand(_ => ShowSettingsDialog(), _ => !IsProcessing);
            SaveSettingsCommand = new RelayCommand(_ => SaveSettings());
            SwitchToPhoneModeCommand = new RelayCommand(_ => SelectedMode = ArchiveMode.Phone, _ => !IsProcessing);
            SwitchToStandardModeCommand = new RelayCommand(_ => SelectedMode = ArchiveMode.Standard, _ => !IsProcessing);
            SwitchToMediaBrowserModeCommand = new RelayCommand(_ => SelectedMode = ArchiveMode.MediaBrowser, _ => !IsProcessing);
            ExtractArchiveCommand = new RelayCommand(async _ => await ExtractArchiveAsync(), _ => !IsProcessing);
            ListArchiveCommand = new RelayCommand(async _ => await ListArchiveAsync(), _ => !IsProcessing);
            TestEncodeCommand = new RelayCommand(async _ => await TestEncodeAsync(), _ => !IsProcessing);
        }

        private void SubscribeToEvents()
        {
            _processingService.ProgressUpdated += OnProgressUpdated;
            _processingService.ProcessingCompleted += OnProcessingCompleted;
            _processingService.ProcessingError += OnProcessingError;
        }

        // Command implementations
        private void AddFile()
        {
            var files = _fileDialogService.OpenFilesDialog(
                "Select media files to archive",
                "All Files (*.*)|*.*|Images (*.jpg;*.png;*.bmp;*.tiff;*.raw)|*.jpg;*.png;*.bmp;*.tiff;*.raw|Videos (*.mp4;*.mov;*.avi;*.mkv)|*.mp4;*.mov;*.avi;*.mkv");

            if (files != null)
            {
                var before = _queueService.Count;
                foreach (var file in files)
                {
                    _queueService.AddFile(file);
                }
                var added = _queueService.Count - before;
                StatusMessage = $"Added {added} file(s) to queue";
            }
        }

        private void AddFolder()
        {
            var folder = _fileDialogService.OpenFolderDialog("Select folder containing media files");
            if (!string.IsNullOrEmpty(folder))
            {
                var before = _queueService.Count;
                _queueService.AddFolder(folder);
                var added = _queueService.Count - before;
                StatusMessage = $"Added {added} media file(s) from folder";
            }
        }

        private void RemoveFile()
        {
            if (SelectedFile != null)
            {
                _queueService.RemoveItem(SelectedFile);
                StatusMessage = "File removed from queue";
            }
        }

        private bool CanRemoveFile() => SelectedFile != null && !IsProcessing;

        private void ClearQueue()
        {
            _queueService.Clear();
            StatusMessage = "Queue cleared";
        }

        private bool CanClearQueue() => _queueService.Count > 0 && !IsProcessing;

        private async Task StartProcessingAsync()
        {
            var selectedFiles = _mediaBrowserViewModel.Thumbnails
                .Where(t => t.IsChecked)
                .Select(t => t.FilePath)
                .Where(p => !string.IsNullOrWhiteSpace(p) && File.Exists(p))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .ToList();

            var selectedFolders = _mediaBrowserViewModel.GetCheckedFolderPaths();

            foreach (var folder in selectedFolders)
            {
                _queueService.AddFolder(folder);
            }

            foreach (var file in selectedFiles)
            {
                _queueService.AddFile(file);
            }

            _queueService.SortPendingByType();

            if (_queueService.Count == 0)
            {
                StatusMessage = "No files to process";
                return;
            }

            if (string.IsNullOrEmpty(_processingOptions.OutputArchivePath))
            {
                // Prompt the user for an output path instead of silently failing
                var outputPath = _fileDialogService.SaveFileDialog(
                    "Save Archive As",
                    "Zstd Archive (*.zstd)|*.zstd|OpenArc Archive (*.oarc)|*.oarc|All Files (*.*)|*.*",
                    "archive.zstd");

                if (string.IsNullOrEmpty(outputPath))
                {
                    StatusMessage = "Processing cancelled - no output path selected";
                    return;
                }

                _processingOptions.OutputArchivePath = outputPath;
                if (_settingsViewModel != null)
                {
                    _settingsViewModel.SyncOptions(_processingOptions);
                }

                StatusMessage = $"Output path set to: {_processingOptions.OutputArchivePath}";
            }

            _cancellationTokenSource = new CancellationTokenSource();
            IsProcessing = true;
            IsErrorState = false; // Reset error state when starting processing
            OverallProgress = 0;
            StatusMessage = "Starting processing...";
            _processingStartTime = DateTime.Now;
            EstimatedTimeRemaining = "Calculating...";

            try
            {
                var pendingItems = _queueService.Items.Where(f => f.Status == DocumentStatus.Pending).ToList();

                var mediaExtensions = new[] { ".bpg", ".jpg", ".jpeg", ".png", ".bmp", ".tiff", ".tif", ".webp", ".gif", ".heic", ".heif", ".dng", ".raw", ".cr2", ".nef", ".arw", ".orf", ".rw2", ".raf", ".3fr", ".fff", ".dcr", ".kdc", ".srf", ".sr2", ".erf", ".mef", ".mrw", ".nrw", ".pef", ".iiq", ".x3f", ".jp2", ".j2k", ".j2c", ".jpc", ".jpt", ".jph", ".jhc", ".mp4", ".mov", ".avi", ".mkv", ".webm" };

                var expanded = BuildProcessingPlan(pendingItems, mediaExtensions);

                if (expanded.Count == 0)
                {
                    StatusMessage = "No valid files to process";
                    return;
                }

                foreach (var item in pendingItems)
                {
                    item.Status = DocumentStatus.Processing;
                    item.Progress = 0;
                }

                var perItemTotals = expanded
                    .GroupBy(e => e.Item)
                    .ToDictionary(g => g.Key, g => g.Count());

                var perItemCompleted = expanded
                    .GroupBy(e => e.Item)
                    .ToDictionary(g => g.Key, g => 0);

                TotalFileCount = expanded.Count;

                var progress = new Progress<DocumentProcessingProgress>(p =>
                {
                    var progressPercentage = p.Total > 0 ? (double)p.Current / p.Total * 100 : 0;
                    OverallProgress = progressPercentage;
                    StatusMessage = string.IsNullOrWhiteSpace(_processingOptions.OutputArchivePath)
                        ? p.Status
                        : $"{p.Status} → {_processingOptions.OutputArchivePath}";
                    CurrentFileIndex = p.Current;
                    TotalFileCount = p.Total > 0 ? p.Total : expanded.Count;
                    OnPropertyChanged(nameof(ProgressText));

                    // Calculate estimated time remaining
                    if (p.Current > 0 && p.Total > 0)
                    {
                        var elapsed = DateTime.Now - _processingStartTime;
                        var averageTimePerFile = elapsed.TotalSeconds / p.Current;
                        var remainingFiles = p.Total - p.Current;
                        var estimatedSeconds = averageTimePerFile * remainingFiles;

                        if (estimatedSeconds < 60)
                        {
                            EstimatedTimeRemaining = $"~{estimatedSeconds:F0}s remaining";
                        }
                        else if (estimatedSeconds < 3600)
                        {
                            EstimatedTimeRemaining = $"~{estimatedSeconds / 60:F0}m remaining";
                        }
                        else
                        {
                            var hours = (int)(estimatedSeconds / 3600);
                            var minutes = (int)((estimatedSeconds % 3600) / 60);
                            EstimatedTimeRemaining = $"~{hours}h {minutes}m remaining";
                        }
                    }

                    // Extract current file name from status if present
                    if (!string.IsNullOrEmpty(p.Status) && p.Status.Contains(":"))
                    {
                        var parts = p.Status.Split(':');
                        if (parts.Length > 1)
                        {
                            CurrentFileName = parts[^1].Trim();
                        }
                    }

                    if (p.Current > 0 && p.Current <= expanded.Count)
                    {
                        var (item, _) = expanded[p.Current - 1];
                        perItemCompleted[item] = Math.Min(perItemCompleted[item] + 1, perItemTotals[item]);
                        var perItemPercent = perItemTotals[item] > 0
                            ? (double)perItemCompleted[item] / perItemTotals[item] * 100
                            : progressPercentage;
                        item.Progress = (int)perItemPercent;
                        item.Status = DocumentStatus.Processing;
                    }
                });

                var results = await _processingService.ProcessDocumentsAsync(
                    expanded.Select(e => e.Path),
                    _processingOptions,
                    progress,
                    _cancellationTokenSource.Token);

                int successCount = 0;
                string? firstError = null;

                for (int i = 0; i < expanded.Count; i++)
                {
                    var (item, _) = expanded[i];
                    if (i < results.Count && results[i].Success)
                    {
                        perItemCompleted[item] = Math.Min(perItemCompleted[item] + 1, perItemTotals[item]);
                        if (perItemCompleted[item] >= perItemTotals[item])
                        {
                            item.Status = DocumentStatus.Completed;
                            item.Progress = 100;
                            item.OutputPath = results[i].OutputPath;
                            successCount++;
                        }
                        else
                        {
                            var perItemPercent = (double)perItemCompleted[item] / perItemTotals[item] * 100;
                            item.Progress = (int)perItemPercent;
                        }
                    }
                    else
                    {
                        item.Status = DocumentStatus.Error;
                        var error = i < results.Count ? results[i].Error : "Processing failed";
                        item.ErrorMessage = error;
                        firstError ??= error;
                    }
                }

                if (successCount == perItemTotals.Count)
                {
                    StatusMessage = string.IsNullOrWhiteSpace(_processingOptions.OutputArchivePath)
                        ? $"Processing completed - {successCount} item(s) archived"
                        : $"Saved archive to {_processingOptions.OutputArchivePath}";
                }
                else if (successCount == 0)
                {
                    IsErrorState = true;
                    StatusMessage = $"Processing failed: {firstError ?? "Unknown error"}";
                }
                else
                {
                    IsErrorState = true;
                    StatusMessage = $"{successCount}/{perItemTotals.Count} succeeded. Error: {firstError}";
                }
            }
            catch (OperationCanceledException)
            {
                StatusMessage = "Processing cancelled";
                foreach (var file in _queueService.Items.Where(f => f.Status == DocumentStatus.Processing))
                {
                    file.Status = DocumentStatus.Cancelled;
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error during processing");
                StatusMessage = $"Error: {ex.Message}";
            }
            finally
            {
                IsProcessing = false;
                OverallProgress = 0;
                CurrentFileName = string.Empty;
                CurrentFileIndex = 0;
                TotalFileCount = 0;
                EstimatedTimeRemaining = string.Empty;
                OnPropertyChanged(nameof(ProgressText));
                _cancellationTokenSource?.Dispose();
                _cancellationTokenSource = null;
            }
        }

        private bool CanStartProcessing() =>
            !IsProcessing &&
            (_queueService.Count > 0 || _mediaBrowserViewModel.Thumbnails.Any(t => t.IsChecked));

        private void CancelProcessing()
        {
            _cancellationTokenSource?.Cancel();
            _processingService.CancelProcessing();
            StatusMessage = "Cancelling processing...";
        }

        private bool CanCancelProcessing() => IsProcessing;

        private void ShowSettingsDialog()
        {
            var workingOptions = _processingOptions.Clone();
            var workingViewModel = new SettingsViewModel(_fileDialogService, _settingsService);
            workingViewModel.SyncOptions(workingOptions);

            var window = new SettingsWindow
            {
                Owner = Application.Current.MainWindow,
                DataContext = workingViewModel
            };

            var result = window.ShowDialog();
            if (result == true)
            {
                _processingOptions.CopyFrom(workingOptions);
                _settingsService.SaveSettings(_processingOptions);
                StatusMessage = "Settings saved";

                if (_phoneDetectionService != null)
                {
                    if (_processingOptions.AutoDetectPhone)
                        _phoneDetectionService.StartDetection();
                    else
                        _phoneDetectionService.StopDetection();
                }
            }
        }

        private void ShowQueue()
        {
            QueueVisible = !QueueVisible;
        }

        private void SaveSettings()
        {
            _settingsService.SaveSettings(_processingOptions);
            StatusMessage = "Settings saved";
        }

        public void HandleDroppedFiles(string[] files)
        {
            var before = _queueService.Count;
            foreach (var file in files)
            {
                if (File.Exists(file))
                {
                    _queueService.AddFile(file);
                }
                else if (Directory.Exists(file))
                {
                    _queueService.AddFolder(file);
                }
            }
            var added = _queueService.Count - before;
            StatusMessage = $"Added {added} file(s) from drop";
        }

        private void OnProgressUpdated(object? sender, DocumentProcessingProgress e)
        {
            Application.Current.Dispatcher.Invoke(() =>
            {
                StatusMessage = e.Status;
            });
        }

        private void OnProcessingCompleted(object? sender, DocumentProcessingResult e)
        {
            Application.Current.Dispatcher.Invoke(() =>
            {
                StatusMessage = e.Success ? "Processing completed successfully" : $"Processing failed: {e.Error}";

                if (!e.Success)
                    return;

                var outputPath = e.OutputPath;
                if (string.IsNullOrWhiteSpace(outputPath))
                    return;

                var ext = Path.GetExtension(outputPath);
                if (!string.Equals(ext, ".oarc", StringComparison.OrdinalIgnoreCase) &&
                    !string.Equals(ext, ".zstd", StringComparison.OrdinalIgnoreCase) &&
                    !string.Equals(ext, ".zst", StringComparison.OrdinalIgnoreCase))
                {
                    return;
                }

                if (!File.Exists(outputPath))
                    return;

                var result = MessageBox.Show(
                    "Archiving finished - test the output?",
                    "Archive complete",
                    MessageBoxButton.YesNo,
                    MessageBoxImage.Question);

                if (result != MessageBoxResult.Yes)
                    return;

                var window = new ArchiveContentsWindow
                {
                    Owner = Application.Current.MainWindow,
                    DataContext = new ArchiveContentsViewModel(_processingService, outputPath)
                };
                window.Show();
            });
        }

        private void OnProcessingError(object? sender, string e)
        {
            Application.Current.Dispatcher.Invoke(() =>
            {
                StatusMessage = $"Error: {e}";
                IsErrorState = true;
            });
        }

        private void OnPhoneConnected(object? sender, string phoneName)
        {
            Application.Current.Dispatcher.Invoke(() =>
            {
                StatusMessage = $"Phone connected: {phoneName}";
                if (_phoneDetectionService != null)
                {
                    try
                    {
                        var phones = _phoneDetectionService.GetConnectedPhones();
                        _mediaBrowserViewModel.SyncPhoneRoots(phones);
                        TryPromptPhoneArchiveAsync(phones);
                    }
                    catch
                    {
                    }
                }
            });
        }

        private List<(DocumentItem Item, string Path)> BuildProcessingPlan(List<DocumentItem> pendingItems, string[] mediaExtensions)
        {
            // QueueService already expands folders into individual files. Keep the per-item mapping intact.
            return pendingItems
                .Where(i => !string.IsNullOrWhiteSpace(i.FilePath) && File.Exists(i.FilePath))
                .Select(i => (i, i.FilePath))
                .ToList();
        }

        private void OnFolderCheckChanged(string folderPath, bool? isChecked)
        {
            if (isChecked == true)
            {
                _queueService.AddFolder(folderPath);
            }
            else
            {
                _queueService.RemoveFolder(folderPath);
            }
        }

        private void OnPhoneDisconnected(object? sender, string phonePath)
        {
            Application.Current.Dispatcher.Invoke(() =>
            {
                StatusMessage = "Phone disconnected";
                if (_phoneDetectionService != null)
                {
                    try
                    {
                        _mediaBrowserViewModel.SyncPhoneRoots(_phoneDetectionService.GetConnectedPhones());
                    }
                    catch
                    {
                    }
                }
            });
        }

        private sealed class PhoneStatusDto
        {
            public string? PhoneRoot { get; set; }
            public string? DbPath { get; set; }
            public bool FirstTime { get; set; }
            public long LastBackupAt { get; set; }
            public long TotalFiles { get; set; }
            public long ArchivedFiles { get; set; }
            public long UnarchivedFiles { get; set; }
        }

        private void TryPromptPhoneArchiveAsync(IEnumerable<PhoneDevice> phones)
        {
            foreach (var phone in phones)
            {
                if (phone == null || string.IsNullOrWhiteSpace(phone.Path))
                    continue;

                if (_phoneArchivePromptedThisSession.Contains(phone.Path))
                    continue;

                _phoneArchivePromptedThisSession.Add(phone.Path);

                // Activate phone mode in media browser
                if (_stagingService != null)
                {
                    _mediaBrowserViewModel.ActivatePhoneMode(phone, _stagingService.StagingDirectory);
                }

                Task.Run(async () =>
                {
                    try
                    {
                        // First, get new files that haven't been backed up
                        List<string> newFiles = new();
                        if (_stagingService != null)
                        {
                            newFiles = await _stagingService.GetNewFilesAsync(phone.Path, recursive: true);
                        }

                        // Also get status from FFI
                        string json = string.Empty;
                        try
                        {
                            json = OpenArcFFI.GetPhoneStatusJson(phone.Path);
                        }
                        catch { }

                        return (newFiles, json);
                    }
                    catch
                    {
                        return (new List<string>(), string.Empty);
                    }
                }).ContinueWith(t =>
                {
                    var (newFiles, json) = t.Result;
                    
                    PhoneStatusDto? status = null;
                    if (!string.IsNullOrWhiteSpace(json))
                    {
                        try
                        {
                            status = JsonSerializer.Deserialize<PhoneStatusDto>(json, new JsonSerializerOptions
                            {
                                PropertyNameCaseInsensitive = true
                            });
                        }
                        catch { }
                    }

                    // Use new files count from staging service, or fall back to FFI status
                    var hasNewFiles = newFiles.Count > 0 || (status != null && (status.FirstTime || status.UnarchivedFiles > 0));

                    if (!hasNewFiles)
                        return;

                    Application.Current.Dispatcher.Invoke(() =>
                    {
                        if (IsProcessing)
                            return;

                        // Prompt to stage files first
                        _ = PromptAndStageFilesAsync(phone, newFiles);
                    });
                });
            }
        }

        private async Task PromptAndStageFilesAsync(PhoneDevice phone, List<string> newFiles)
        {
            if (_stagingService == null)
            {
                // No staging service, fall back to direct archive
                var fallbackResult = MessageBox.Show(
                    "Device detected - some user files are not archived per database - would you like to compress and archive them now?",
                    "Device detected",
                    MessageBoxButton.YesNo,
                    MessageBoxImage.Question);

                if (fallbackResult == MessageBoxResult.Yes)
                {
                    await ArchivePhoneAsync(phone);
                }
                return;
            }

            var fileCount = newFiles.Count;
            var totalSize = newFiles.Sum(f => { try { return new FileInfo(f).Length; } catch { return 0L; } });
            var totalSizeMB = totalSize / (1024.0 * 1024.0);

            var deviceIcon = phone.DeviceCategory switch
            {
                MobileDeviceType.Phone => "📱",
                MobileDeviceType.SDCard => "💾",
                MobileDeviceType.Camera => "📷",
                _ => "📁"
            };

            var message = $"{deviceIcon} {phone.DeviceType} detected: {phone.Name}\n\n" +
                          $"Found {fileCount:N0} new files ({totalSizeMB:N1} MB) to archive.\n\n" +
                          $"Would you like to copy these files to the local staging area first?\n" +
                          $"(Recommended: Avoids slow USB transfer during compression)\n\n" +
                          $"• Yes - Copy to staging, then configure encoding settings\n" +
                          $"• No - Archive directly from device (slower)\n" +
                          $"• Cancel - Skip for now";

            var stageResult = MessageBox.Show(
                message,
                $"{phone.DeviceType} Detected - Stage Files?",
                MessageBoxButton.YesNoCancel,
                MessageBoxImage.Question);

            if (stageResult == MessageBoxResult.Cancel)
                return;

            if (stageResult == MessageBoxResult.Yes)
            {
                // Stage files first
                await StageFilesFromDeviceAsync(phone, newFiles);
            }
            else
            {
                // Archive directly without staging
                await ArchivePhoneAsync(phone);
            }
        }

        private async Task StageFilesFromDeviceAsync(PhoneDevice phone, List<string> files)
        {
            if (_stagingService == null)
                return;

            IsProcessing = true;
            OverallProgress = 0;
            StatusMessage = $"Staging files from {phone.Name}...";

            try
            {
                var progress = new Progress<StagingProgress>(p =>
                {
                    OverallProgress = p.ProgressPercent;
                    CurrentFileIndex = p.ProcessedFiles;
                    TotalFileCount = p.TotalFiles;
                    CurrentFileName = p.CurrentFile;
                    StatusMessage = $"Staging {p.ProcessedFiles}/{p.TotalFiles}: {p.CurrentFile}";
                    OnPropertyChanged(nameof(ProgressText));
                });

                var result = await _stagingService.StageFilesAsync(files, progress, _cancellationTokenSource?.Token ?? CancellationToken.None);

                if (result.Success)
                {
                    StatusMessage = $"✅ Staged {result.FilesStaged} files ({result.BytesStaged / (1024.0 * 1024.0):N1} MB)";

                    // Add staged files to queue for processing
                    foreach (var stagedFile in result.StagedFiles)
                    {
                        _queueService.AddFile(stagedFile);
                    }

                    // Refresh drives to show staging folder
                    _mediaBrowserViewModel.RefreshDrives();

                    // Ask if user wants to proceed with encoding settings
                    var proceedResult = MessageBox.Show(
                        $"Successfully staged {result.FilesStaged} files to local storage.\n\n" +
                        $"The files have been added to the processing queue.\n" +
                        $"Would you like to configure encoding settings and start processing?",
                        "Files Staged - Configure Settings?",
                        MessageBoxButton.YesNo,
                        MessageBoxImage.Question);

                    if (proceedResult == MessageBoxResult.Yes)
                    {
                        ShowSettingsDialog();
                    }
                }
                else
                {
                    StatusMessage = $"⚠️ Staging completed with errors: {result.FailedFiles.Count} failed";
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to stage files from device");
                StatusMessage = $"❌ Staging failed: {ex.Message}";
            }
            finally
            {
                IsProcessing = false;
                OverallProgress = 0;
                OnPropertyChanged(nameof(ProgressText));
            }
        }

        private async Task ArchivePhoneAsync(PhoneDevice phone)
        {
            if (phone == null || string.IsNullOrWhiteSpace(phone.Path))
                return;

            if (IsProcessing)
                return;

            var defaultName = $"phone_archive_{DateTime.Now:yyyyMMdd_HHmmss}.oarc";

            var outputPath = _fileDialogService.SaveFileDialog(
                "Save Phone Archive",
                "OpenArc Archive (*.oarc)|*.oarc|All Files (*.*)|*.*",
                defaultName);

            if (string.IsNullOrWhiteSpace(outputPath))
                return;

            IsProcessing = true;
            OverallProgress = 0;
            StatusMessage = $"Archiving phone: {phone.Name}";

            try
            {
                await Task.Run(() =>
                {
                    var settings = new OpenArcFFI.CompressionSettings
                    {
                        BpgQuality = _processingOptions.BpgQuality,
                        BpgLossless = _processingOptions.BpgLossless,
                        BpgBitDepth = _processingOptions.BpgBitDepth,
                        BpgChromaFormat = _processingOptions.BpgChromaFormat,
                        BpgEncoderType = _processingOptions.BpgEncoderType,
                        BpgCompressionLevel = _processingOptions.BpgCompressionLevel,
                        VideoCodec = (int)_processingOptions.VideoCodec,
                        VideoSpeed = (int)_processingOptions.VideoSpeed,
                        VideoCrf = _processingOptions.VideoCrf,
                        CompressionLevel = _processingOptions.CompressionLevel,
                        EnableCatalog = false,
                        EnableDedup = _processingOptions.EnableDedup,
                        SkipAlreadyCompressedVideos = _processingOptions.SkipAlreadyCompressedVideos
                    };

                    OpenArcFFI.ProgressCallback cb = p =>
                    {
                        Application.Current.Dispatcher.Invoke(() =>
                        {
                            var current = Math.Max(0, p.CurrentFile);
                            var total = Math.Max(0, p.TotalFiles);
                            OverallProgress = total > 0 ? (double)current / total * 100.0 : 0;
                            StatusMessage = string.IsNullOrWhiteSpace(p.CurrentFileName)
                                ? $"Archiving {current}/{total}"
                                : $"Archiving {current}/{total}: {p.CurrentFileName}";
                            OnPropertyChanged(nameof(ProgressText));
                        });
                    };

                    var rc = OpenArcFFI.PhoneArchivePendingFiles(phone.Path, outputPath, ref settings, cb);
                    GC.KeepAlive(cb);

                    if (rc < 0)
                    {
                        var err = OpenArcFFI.GetLastErrorMessage();
                        throw new InvalidOperationException(err);
                    }

                    return rc;
                });

                StatusMessage = $"✅ Archived phone files to {Path.GetFileName(outputPath)}";
            }
            catch (Exception ex)
            {
                StatusMessage = $"❌ Phone archive failed: {ex.Message}";
            }
            finally
            {
                IsProcessing = false;
            }
        }

        private string GetCatalogDbPath()
        {
            // Use the default catalog path based on the output archive path or a default location
            if (!string.IsNullOrEmpty(_processingOptions.OutputArchivePath))
            {
                var archivePath = new System.IO.FileInfo(_processingOptions.OutputArchivePath);
                return System.IO.Path.Combine(archivePath.Directory?.FullName ?? archivePath.DirectoryName!, $"{System.IO.Path.GetFileNameWithoutExtension(archivePath.Name)}.catalog.sqlite");
            }
            else
            {
                // Default to app data directory if no archive path is set
                var appDataPath = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
                var openArcPath = System.IO.Path.Combine(appDataPath, "OpenArc");
                System.IO.Directory.CreateDirectory(openArcPath);
                return System.IO.Path.Combine(openArcPath, "default.catalog.sqlite");
            }
        }

        private async Task ExtractArchiveAsync()
        {
            var archiveFile = _fileDialogService.OpenFileDialog(
                "Select archive to extract",
                "Zstd Archive (*.zstd)|*.zstd|OpenArc Archive (*.oarc)|*.oarc|All Files (*.*)|*.*",
                _processingOptions.OutputArchivePath);

            if (string.IsNullOrEmpty(archiveFile))
                return;

            var outputDir = _fileDialogService.OpenFolderDialog("Select extraction directory");
            if (string.IsNullOrEmpty(outputDir))
                return;

            IsProcessing = true;
            StatusMessage = "Extracting archive...";

            try
            {
                var progress = new Progress<DocumentProcessingProgress>(p =>
                {
                    var progressPercentage = p.Total > 0 ? (double)p.Current / p.Total * 100 : 0;
                    OverallProgress = progressPercentage;
                    StatusMessage = p.Status;
                    OnPropertyChanged(nameof(ProgressText));
                });

                var success = await _processingService.ExtractArchiveAsync(archiveFile, outputDir, progress, _cancellationTokenSource?.Token ?? CancellationToken.None);

                StatusMessage = success ? "Extraction completed" : "Extraction failed";
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Extraction failed");
                StatusMessage = $"Error: {ex.Message}";
            }
            finally
            {
                IsProcessing = false;
                OverallProgress = 0;
                OnPropertyChanged(nameof(ProgressText));
            }
        }

        private async Task ListArchiveAsync()
        {
            var archiveFile = _fileDialogService.OpenFileDialog(
                "Select archive to list",
                "Zstd Archive (*.zstd)|*.zstd|OpenArc Archive (*.oarc)|*.oarc|All Files (*.*)|*.*",
                _processingOptions.OutputArchivePath);

            if (string.IsNullOrEmpty(archiveFile))
                return;

            StatusMessage = "Listing archive contents...";

            try
            {
                var files = await _processingService.ListArchiveAsync(archiveFile, CancellationToken.None);

                if (files.Count == 0)
                {
                    StatusMessage = "Archive is empty or could not be read";
                    return;
                }

                // Create a simple message box with file list
                var message = $"Archive contains {files.Count} files:\n\n";
                foreach (var file in files.Take(20)) // Limit to first 20 files
                {
                    message += $"{file.Filename} ({file.FormattedOriginalSize} → {file.FormattedCompressedSize})\n";
                }

                if (files.Count > 20)
                {
                    message += $"\n... and {files.Count - 20} more files";
                }

                MessageBox.Show(message, "Archive Contents", MessageBoxButton.OK, MessageBoxImage.Information);
                StatusMessage = $"Listed {files.Count} files";
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Archive listing failed");
                StatusMessage = $"Error: {ex.Message}";
            }
        }

        private async Task TestEncodeAsync()
        {
            var inputFile = _fileDialogService.OpenFileDialog(
                "Select a file to test encode",
                "Images (*.jpg;*.jpeg;*.png;*.bmp;*.tiff)|*.jpg;*.jpeg;*.png;*.bmp;*.tiff|Videos (*.mp4;*.mov;*.avi;*.mkv)|*.mp4;*.mov;*.avi;*.mkv|All Files (*.*)|*.*");

            if (string.IsNullOrEmpty(inputFile))
                return;

            var fileInfo = new FileInfo(inputFile);
            var ext = fileInfo.Extension.ToLower();

            // Determine if this is an image or video
            bool isImage = new[] { ".jpg", ".jpeg", ".png", ".bmp", ".tiff", ".raw", ".cr2", ".nef", ".arw" }.Contains(ext);
            bool isVideo = new[] { ".mp4", ".mov", ".avi", ".mkv", ".webm" }.Contains(ext);

            if (!isImage && !isVideo)
            {
                MessageBox.Show("Please select an image or video file.", "Invalid File Type", MessageBoxButton.OK, MessageBoxImage.Warning);
                return;
            }

            // Create output path in same directory as input
            var outputFileName = Path.GetFileNameWithoutExtension(inputFile) + "_encoded";
            var outputExtension = isImage ? ".bpg" : ".mp4";
            var outputPath = Path.Combine(fileInfo.DirectoryName ?? Path.GetTempPath(), outputFileName + outputExtension);

            // If file exists, add timestamp
            if (File.Exists(outputPath))
            {
                outputPath = Path.Combine(fileInfo.DirectoryName ?? Path.GetTempPath(),
                    $"{outputFileName}_{DateTime.Now:yyyyMMdd_HHmmss}{outputExtension}");
            }

            IsProcessing = true;
            StatusMessage = $"Test encoding {fileInfo.Name}...";
            var startTime = DateTime.Now;

            try
            {
                _cancellationTokenSource = new CancellationTokenSource();

                bool success;
                if (isImage)
                {
                    success = await _processingService.EncodeBpgFileAsync(inputFile, outputPath, _processingOptions, _cancellationTokenSource.Token);
                }
                else
                {
                    success = await _processingService.EncodeVideoFileAsync(inputFile, outputPath, _processingOptions, _cancellationTokenSource.Token);
                }

                if (success)
                {
                    var outputFileInfo = new FileInfo(outputPath);
                    var encodingTime = DateTime.Now - startTime;

                    if (outputFileInfo.Exists)
                    {
                        var compressionRatio = ((1.0 - ((double)outputFileInfo.Length / fileInfo.Length)) * 100);

                        var message = $"Test encoding completed!\n\n" +
                                      $"File Type: {(isImage ? "Image (BPG)" : "Video (MP4)")}\n" +
                                      $"Original: {FormatFileSize(fileInfo.Length)}\n" +
                                      $"Encoded: {FormatFileSize(outputFileInfo.Length)}\n" +
                                      $"Compression: {compressionRatio:F1}%\n" +
                                      $"Time: {encodingTime.TotalSeconds:F1}s\n\n" +
                                      $"Settings used:\n" +
                                      (isImage
                                        ? $"- BPG Quality: {_processingOptions.BpgQuality}\n" +
                                          $"- Bit Depth: {_processingOptions.BpgBitDepth}\n" +
                                          $"- Chroma: {GetChromaFormatName(_processingOptions.BpgChromaFormat)}\n" +
                                          $"- Encoder: {(_processingOptions.BpgEncoderType == 0 ? "Default" : "Slow")}"
                                        : $"- Video Codec: {_processingOptions.VideoCodec}\n" +
                                          $"- Video Speed: {_processingOptions.VideoSpeed}\n" +
                                          $"- Video CRF: {_processingOptions.VideoCrf}") +
                                      $"\n\nOutput saved to:\n{outputPath}";

                        var result = MessageBox.Show(message, "Test Encode Results", MessageBoxButton.YesNo, MessageBoxImage.Information);

                        // Open file location if user clicks Yes
                        if (result == MessageBoxResult.Yes)
                        {
                            System.Diagnostics.Process.Start("explorer.exe", $"/select,\"{outputPath}\"");
                        }

                        StatusMessage = $"Test encoding completed - saved to {Path.GetFileName(outputPath)}";
                    }
                    else
                    {
                        MessageBox.Show("Encoding completed but output file not found.", "Warning", MessageBoxButton.OK, MessageBoxImage.Warning);
                        StatusMessage = "Test encoding completed but output file missing";
                    }
                }
                else
                {
                    MessageBox.Show($"Test encoding failed. Check the error log for details.", "Test Encode Error", MessageBoxButton.OK, MessageBoxImage.Error);
                    StatusMessage = "Test encoding failed";
                }
            }
            catch (OperationCanceledException)
            {
                StatusMessage = "Test encoding cancelled";
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Test encoding failed");
                MessageBox.Show($"Test encoding error:\n{ex.Message}", "Test Encode Error", MessageBoxButton.OK, MessageBoxImage.Error);
                StatusMessage = $"Error: {ex.Message}";
            }
            finally
            {
                IsProcessing = false;
                OverallProgress = 0;
                CurrentFileIndex = 0;
                TotalFileCount = 0;
                OnPropertyChanged(nameof(ProgressText));
                _cancellationTokenSource?.Dispose();
                _cancellationTokenSource = null;
            }
        }

        private string FormatFileSize(long bytes)
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

        private string GetChromaFormatName(int chromaFormat)
        {
            return chromaFormat switch
            {
                0 => "4:2:0",
                1 => "4:4:4",
                2 => "RGB",
                _ => "Unknown"
            };
        }

        protected virtual void OnPropertyChanged([System.Runtime.CompilerServices.CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }

        protected bool SetProperty<T>(ref T storage, T value, [System.Runtime.CompilerServices.CallerMemberName] string? propertyName = null)
        {
            if (Equals(storage, value)) return false;
            storage = value;
            OnPropertyChanged(propertyName);
            return true;
        }
    }
}