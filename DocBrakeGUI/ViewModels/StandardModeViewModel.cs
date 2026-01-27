using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Threading.Tasks;
using System.Windows.Input;
using DocBrake.Commands;
using DocBrake.Models;
using DocBrake.Services;

namespace DocBrake.ViewModels
{
    public class StandardModeViewModel : INotifyPropertyChanged
    {
        private readonly IFileDialogService _fileDialogService = null!;
        private readonly ISettingsService _settingsService = null!;
        private ObservableCollection<DocumentItem> _selectedFiles = new();
        private bool _isArchiving;

        public event PropertyChangedEventHandler? PropertyChanged;

        public StandardModeViewModel(IFileDialogService fileDialogService, ISettingsService settingsService)
        {
            _fileDialogService = fileDialogService ?? throw new ArgumentNullException(nameof(fileDialogService));
            _settingsService = settingsService ?? throw new ArgumentNullException(nameof(settingsService));

            InitializeCommands();
        }

        public StandardModeViewModel()
        {
            // Parameterless constructor for XAML
            InitializeCommands();
        }

        private void InitializeCommands()
        {
            AddFilesCommand = new RelayCommand(_ => AddFiles());
            AddFolderCommand = new RelayCommand(_ => AddFolder());
            ClearCommand = new RelayCommand(_ => ClearFiles(), _ => SelectedFiles.Count > 0);
            ArchiveCommand = new RelayCommand(async _ => await ArchiveFiles(), _ => CanArchive());
            ShowSettingsCommand = new RelayCommand(_ => ShowSettings());
        }

        // Properties
        public ObservableCollection<DocumentItem> SelectedFiles
        {
            get => _selectedFiles;
            set => SetProperty(ref _selectedFiles, value);
        }

        public bool IsArchiving
        {
            get => _isArchiving;
            set => SetProperty(ref _isArchiving, value);
        }

        public long TotalFileSize => SelectedFiles.Sum(f => f.FileSize);

        // Commands
        public ICommand AddFilesCommand { get; private set; } = null!;
        public ICommand AddFolderCommand { get; private set; } = null!;
        public ICommand ClearCommand { get; private set; } = null!;
        public ICommand ArchiveCommand { get; private set; } = null!;
        public ICommand ShowSettingsCommand { get; private set; } = null!;

        // Command implementations
        private void AddFiles()
        {
            var files = _fileDialogService.OpenFilesDialog(
                "Select media files to archive",
                "Media Files|*.bpg;*.jpg;*.jpeg;*.png;*.bmp;*.tiff;*.tif;*.webp;*.gif;*.heic;*.heif;*.dng;*.raw;*.cr2;*.nef;*.arw;*.orf;*.rw2;*.raf;*.3fr;*.fff;*.dcr;*.kdc;*.srf;*.sr2;*.erf;*.mef;*.mrw;*.nrw;*.pef;*.iiq;*.x3f;*.jp2;*.j2k;*.j2c;*.jpc;*.jpt;*.jph;*.jhc;*.mp4;*.mov;*.avi;*.mkv;*.webm|Images|*.bpg;*.jpg;*.jpeg;*.png;*.bmp;*.tiff;*.tif;*.webp;*.gif;*.heic;*.heif;*.dng;*.raw;*.cr2;*.nef;*.arw;*.orf;*.rw2;*.raf;*.3fr;*.fff;*.dcr;*.kdc;*.srf;*.sr2;*.erf;*.mef;*.mrw;*.nrw;*.pef;*.iiq;*.x3f;*.jp2;*.j2k;*.j2c;*.jpc;*.jpt;*.jph;*.jhc|Videos|*.mp4;*.mov;*.avi;*.mkv;*.webm|All Files|*.*");

            if (files != null)
            {
                foreach (var file in files)
                {
                    AddFileToQueue(file);
                }
            }
        }

        private void AddFolder()
        {
            var folder = _fileDialogService.OpenFolderDialog("Select folder containing media files");
            if (!string.IsNullOrEmpty(folder))
            {
                var mediaExtensions = new[] { ".bpg", ".jpg", ".jpeg", ".png", ".bmp", ".tiff", ".tif", ".webp", ".gif", ".heic", ".heif", ".dng", ".raw", ".cr2", ".nef", ".arw", ".orf", ".rw2", ".raf", ".3fr", ".fff", ".dcr", ".kdc", ".srf", ".sr2", ".erf", ".mef", ".mrw", ".nrw", ".pef", ".iiq", ".x3f", ".jp2", ".j2k", ".j2c", ".jpc", ".jpt", ".jph", ".jhc", ".mp4", ".mov", ".avi", ".mkv", ".webm" };
                var allFiles = Directory.GetFiles(folder, "*.*", SearchOption.AllDirectories)
                    .Where(f => mediaExtensions.Contains(Path.GetExtension(f).ToLower()))
                    .ToArray();

                foreach (var file in allFiles)
                {
                    AddFileToQueue(file);
                }
            }
        }

        private void ClearFiles()
        {
            SelectedFiles.Clear();
        }

        private bool CanArchive()
        {
            return !IsArchiving && SelectedFiles.Count > 0;
        }

        private async Task ArchiveFiles()
        {
            if (SelectedFiles.Count == 0)
            {
                return;
            }

            IsArchiving = true;

            try
            {
                // Get output archive path
                var outputPath = _fileDialogService.SaveFileDialog(
                    "Save Archive",
                    "OpenArc Archive (*.oarc)|*.oarc",
                    $"archive_{DateTime.Now:yyyyMMdd_HHmmss}.oarc");

                if (string.IsNullOrEmpty(outputPath))
                {
                    return;
                }

                // TODO: Implement actual archiving via FFI
                foreach (var file in SelectedFiles)
                {
                    file.Status = DocumentStatus.Processing;
                    await Task.Delay(100); // Simulate processing
                    file.Status = DocumentStatus.Completed;
                }

                // Clear the queue after successful archiving
                SelectedFiles.Clear();
            }
            catch (Exception ex)
            {
                // TODO: Handle error
                System.Diagnostics.Debug.WriteLine($"Archive error: {ex.Message}");
            }
            finally
            {
                IsArchiving = false;
            }
        }

        private void ShowSettings()
        {
            // TODO: Show standard mode settings
        }

        private void AddFileToQueue(string filePath)
        {
            if (SelectedFiles.Any(f => f.FilePath.Equals(filePath, StringComparison.OrdinalIgnoreCase)))
                return;

            var fileInfo = new FileInfo(filePath);
            var ext = fileInfo.Extension.ToLower();
            var fileType = FileType.Unknown;

            if (new[] { ".jpg", ".jpeg", ".png", ".bmp", ".tiff", ".raw", ".cr2", ".nef", ".arw" }.Contains(ext))
                fileType = FileType.Image;
            else if (new[] { ".mp4", ".mov", ".avi", ".mkv", ".webm" }.Contains(ext))
                fileType = FileType.Video;
            else if (new[] { ".pdf", ".doc", ".docx", ".txt" }.Contains(ext))
                fileType = FileType.Document;
            else if (new[] { ".zip", ".rar", ".7z" }.Contains(ext))
                fileType = FileType.Archive;

            var documentItem = new DocumentItem
            {
                FilePath = filePath,
                FileName = fileInfo.Name,
                FileSize = fileInfo.Length,
                FileType = fileType,
                Status = DocumentStatus.Pending
            };

            SelectedFiles.Add(documentItem);
        }

        protected virtual void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
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
