using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.CompilerServices;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Input;
using DocBrake.Commands;
using DocBrake.MediaBrowser.Views;
using DocBrake.Models;
using DocBrake.Services;

namespace DocBrake.ViewModels
{
    public class ArchiveContentsViewModel : INotifyPropertyChanged
    {
        private readonly IDocumentProcessingService _processingService;
        private readonly string _archivePath;
        private readonly string _tempRoot;

        private ObservableCollection<ArchiveFileInfo> _files = new();
        private ArchiveFileInfo? _selectedFile;
        private string _statusMessage = string.Empty;
        private bool _isLoading;

        public event PropertyChangedEventHandler? PropertyChanged;

        public ArchiveContentsViewModel(IDocumentProcessingService processingService, string archivePath)
        {
            _processingService = processingService ?? throw new ArgumentNullException(nameof(processingService));
            _archivePath = archivePath ?? throw new ArgumentNullException(nameof(archivePath));
            _tempRoot = Path.Combine(Path.GetTempPath(), "OpenArcExtract", Guid.NewGuid().ToString("N"));

            RefreshCommand = new RelayCommand(async () => await LoadAsync(), () => !IsLoading);
            OpenSelectedCommand = new RelayCommand(async () => await OpenSelectedAsync(), () => !IsLoading && SelectedFile != null);

            _ = LoadAsync();
        }

        public ObservableCollection<ArchiveFileInfo> Files
        {
            get => _files;
            private set
            {
                if (SetProperty(ref _files, value))
                {
                    OnPropertyChanged(nameof(HasFiles));
                }
            }
        }

        public bool HasFiles => Files.Count > 0;

        public ArchiveFileInfo? SelectedFile
        {
            get => _selectedFile;
            set
            {
                if (SetProperty(ref _selectedFile, value))
                {
                    (OpenSelectedCommand as RelayCommand)?.RaiseCanExecuteChanged();
                }
            }
        }

        public string StatusMessage
        {
            get => _statusMessage;
            private set => SetProperty(ref _statusMessage, value);
        }

        public bool IsLoading
        {
            get => _isLoading;
            private set
            {
                if (SetProperty(ref _isLoading, value))
                {
                    (RefreshCommand as RelayCommand)?.RaiseCanExecuteChanged();
                    (OpenSelectedCommand as RelayCommand)?.RaiseCanExecuteChanged();
                }
            }
        }

        public ICommand RefreshCommand { get; }
        public ICommand OpenSelectedCommand { get; }

        public async Task LoadAsync()
        {
            if (IsLoading)
                return;

            IsLoading = true;
            StatusMessage = "Loading archive contents...";

            try
            {
                var list = await _processingService.ListArchiveAsync(_archivePath);

                Files.Clear();
                foreach (var item in list)
                {
                    Files.Add(item);
                }

                StatusMessage = Files.Count > 0 ? $"{Files.Count} file(s)" : "Archive is empty";
                OnPropertyChanged(nameof(HasFiles));
            }
            catch (Exception ex)
            {
                StatusMessage = $"Error: {ex.Message}";
            }
            finally
            {
                IsLoading = false;
            }
        }

        public async Task OpenSelectedAsync()
        {
            var selected = SelectedFile;
            if (selected == null)
                return;

            var relative = selected.Filename ?? string.Empty;
            if (string.IsNullOrWhiteSpace(relative))
                return;

            try
            {
                IsLoading = true;
                StatusMessage = $"Extracting {Path.GetFileName(relative)}...";

                var outputPath = GetSafeOutputPath(_tempRoot, relative);

                var ok = await _processingService.ExtractArchiveEntryAsync(_archivePath, relative, outputPath);
                if (!ok)
                {
                    StatusMessage = "Failed to extract file";
                    return;
                }

                StatusMessage = "Opening...";

                Application.Current.Dispatcher.Invoke(() =>
                {
                    if (selected.FileType == FileType.Image || selected.FileType == FileType.Video)
                    {
                        var window = new MediaImageViewerWindow(outputPath)
                        {
                            Owner = Application.Current.MainWindow
                        };
                        window.Show();
                    }
                    else
                    {
                        Process.Start(new ProcessStartInfo(outputPath)
                        {
                            UseShellExecute = true
                        });
                    }
                });

                StatusMessage = "Ready";
            }
            catch (Exception ex)
            {
                StatusMessage = $"Error: {ex.Message}";
            }
            finally
            {
                IsLoading = false;
            }
        }

        public void CleanupTemp()
        {
            try
            {
                if (Directory.Exists(_tempRoot))
                {
                    Directory.Delete(_tempRoot, true);
                }
            }
            catch
            {
            }
        }

        private static string GetSafeOutputPath(string rootDir, string archiveRelPath)
        {
            var rel = archiveRelPath.Replace('/', Path.DirectorySeparatorChar).Replace('\\', Path.DirectorySeparatorChar);
            while (rel.StartsWith(Path.DirectorySeparatorChar))
            {
                rel = rel.Substring(1);
            }

            var combined = Path.GetFullPath(Path.Combine(rootDir, rel));
            var rootFull = Path.GetFullPath(rootDir) + Path.DirectorySeparatorChar;

            if (!combined.StartsWith(rootFull, StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException("Invalid archive path");

            var dir = Path.GetDirectoryName(combined);
            if (!string.IsNullOrEmpty(dir))
            {
                Directory.CreateDirectory(dir);
            }

            return combined;
        }

        protected virtual void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName ?? string.Empty));
        }

        protected bool SetProperty<T>(ref T storage, T value, [CallerMemberName] string? propertyName = null)
        {
            if (Equals(storage, value)) return false;
            storage = value;
            OnPropertyChanged(propertyName);
            return true;
        }
    }
}
