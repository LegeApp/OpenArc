using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Windows;
using System.Windows.Input;
using DocBrake.Commands;
using DocBrake.Services;

namespace DocBrake.ViewModels
{
    public class ArchiveTrackingViewModel : INotifyPropertyChanged
    {
        private readonly ArchiveTrackingService _archiveTrackingService;
        private readonly IFileDialogService _fileDialogService;
        private readonly string _catalogDbPath;

        private ObservableCollection<ArchiveTrackingService.ArchiveRecord> _archives = new();
        private ArchiveTrackingService.ArchiveRecord _selectedArchive = new();
        private string _statusMessage = "Ready";
        private bool _isLoading;

        public event PropertyChangedEventHandler? PropertyChanged;

        public ArchiveTrackingViewModel(ArchiveTrackingService archiveTrackingService, IFileDialogService fileDialogService, string catalogDbPath)
        {
            _archiveTrackingService = archiveTrackingService ?? throw new ArgumentNullException(nameof(archiveTrackingService));
            _fileDialogService = fileDialogService ?? throw new ArgumentNullException(nameof(fileDialogService));
            _catalogDbPath = catalogDbPath ?? throw new ArgumentNullException(nameof(catalogDbPath));

            Archives = new ObservableCollection<ArchiveTrackingService.ArchiveRecord>();
            LoadArchivesCommand = new RelayCommand(_ => LoadArchives());
            MoveArchiveCommand = new RelayCommand(_ => MoveArchive(), _ => SelectedArchive != null);
            ArchiveEntryCommand = new RelayCommand(param => ArchiveEntry(param as ArchiveTrackingService.ArchiveRecord));
            DeleteEntryCommand = new RelayCommand(param => DeleteEntry(param as ArchiveTrackingService.ArchiveRecord));

            // Load archives initially
            LoadArchives();
        }

        public ObservableCollection<ArchiveTrackingService.ArchiveRecord> Archives
        {
            get => _archives;
            private set
            {
                if (SetProperty(ref _archives, value))
                {
                    OnPropertyChanged(nameof(HasArchives));
                }
            }
        }

        public ArchiveTrackingService.ArchiveRecord SelectedArchive
        {
            get => _selectedArchive;
            set => SetProperty(ref _selectedArchive, value);
        }

        public string StatusMessage
        {
            get => _statusMessage;
            private set => SetProperty(ref _statusMessage, value);
        }

        public bool IsLoading
        {
            get => _isLoading;
            private set => SetProperty(ref _isLoading, value);
        }

        public ICommand LoadArchivesCommand { get; }
        public ICommand MoveArchiveCommand { get; }
        public ICommand ArchiveEntryCommand { get; }
        public ICommand DeleteEntryCommand { get; }

        public bool HasArchives => Archives != null && Archives.Count > 0;

        public void LoadArchives()
        {
            if (IsLoading) return;

            IsLoading = true;
            StatusMessage = "Loading archives...";

            try
            {
                var archives = _archiveTrackingService.GetAllArchives(_catalogDbPath);
                Archives.Clear();
                foreach (var archive in archives)
                {
                    Archives.Add(archive);
                }

                StatusMessage = Archives.Count > 0 ? $"Loaded {Archives.Count} archives" : "No archives found";
                OnPropertyChanged(nameof(HasArchives));
            }
            catch (Exception ex)
            {
                StatusMessage = $"Error loading archives: {ex.Message}";
                OnPropertyChanged(nameof(HasArchives));
            }
            finally
            {
                IsLoading = false;
            }
        }

        public void ArchiveEntry(ArchiveTrackingService.ArchiveRecord? archive)
        {
            if (archive == null) return;

            var result = MessageBox.Show(
                $"Archive this entry? It will be hidden from the list but the information will be preserved.\n\n{archive.ArchivePath}",
                "Archive Entry",
                MessageBoxButton.YesNo,
                MessageBoxImage.Question);

            if (result != MessageBoxResult.Yes) return;

            try
            {
                bool success = _archiveTrackingService.ArchiveEntry(_catalogDbPath, archive.Id);
                if (success)
                {
                    LoadArchives();
                    StatusMessage = $"Archive entry hidden: {Path.GetFileName(archive.ArchivePath)}";
                }
                else
                {
                    StatusMessage = "Failed to archive entry";
                }
            }
            catch (Exception ex)
            {
                StatusMessage = $"Error archiving entry: {ex.Message}";
            }
        }

        public void DeleteEntry(ArchiveTrackingService.ArchiveRecord? archive)
        {
            if (archive == null) return;

            var result = MessageBox.Show(
                $"Permanently delete this archive entry from the database?\n\nThis cannot be undone.\n\n{archive.ArchivePath}",
                "Delete Entry",
                MessageBoxButton.YesNo,
                MessageBoxImage.Warning);

            if (result != MessageBoxResult.Yes) return;

            try
            {
                bool success = _archiveTrackingService.DeleteEntry(_catalogDbPath, archive.Id);
                if (success)
                {
                    LoadArchives();
                    StatusMessage = $"Archive entry deleted: {Path.GetFileName(archive.ArchivePath)}";
                }
                else
                {
                    StatusMessage = "Failed to delete entry";
                }
            }
            catch (Exception ex)
            {
                StatusMessage = $"Error deleting entry: {ex.Message}";
            }
        }

        public void MoveArchive()
        {
            if (SelectedArchive == null) return;

            var destinationPath = _fileDialogService.OpenFolderDialog("Select destination for archive");
            if (string.IsNullOrEmpty(destinationPath)) return;

            try
            {
                bool success = _archiveTrackingService.UpdateArchiveDestination(
                    _catalogDbPath, 
                    SelectedArchive.ArchivePath, 
                    destinationPath);

                if (success)
                {
                    // Refresh the archive list to show the updated destination
                    LoadArchives();
                    StatusMessage = $"Archive moved to: {destinationPath}";
                }
                else
                {
                    StatusMessage = "Failed to update archive destination";
                }
            }
            catch (Exception ex)
            {
                StatusMessage = $"Error moving archive: {ex.Message}";
            }
        }

        public string FormatFileSize(ulong size)
        {
            return _archiveTrackingService.FormatFileSize(size);
        }

        public string FormatDate(ulong timestamp)
        {
            var dateTime = _archiveTrackingService.UnixTimeStampToDateTime(timestamp);
            return dateTime.ToString("yyyy-MM-dd HH:mm:ss");
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