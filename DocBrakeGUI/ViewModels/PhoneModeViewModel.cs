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
    public class PhoneModeViewModel : INotifyPropertyChanged
    {
        private readonly IFileDialogService _fileDialogService = null!;
        private readonly ISettingsService _settingsService = null!;
        private string _phoneStatus = "No phone connected";
        private string _phoneModel = "";
        private string _storagePath = "";
        private ObservableCollection<DocumentItem> _phoneFiles = new();
        private bool _isArchiving;

        public event PropertyChangedEventHandler? PropertyChanged;

        public PhoneModeViewModel(IFileDialogService fileDialogService, ISettingsService settingsService)
        {
            _fileDialogService = fileDialogService ?? throw new ArgumentNullException(nameof(fileDialogService));
            _settingsService = settingsService ?? throw new ArgumentNullException(nameof(settingsService));

            InitializeCommands();
            InitializeSampleData();
        }

        public PhoneModeViewModel()
        {
            // Parameterless constructor for XAML
            InitializeCommands();
            InitializeSampleData();
        }

        private void InitializeCommands()
        {
            RefreshCommand = new RelayCommand(_ => RefreshPhoneDetection());
            ArchiveCommand = new RelayCommand(async _ => await ArchivePhoneMedia(), _ => !IsArchiving);
            ShowSettingsCommand = new RelayCommand(_ => ShowSettings());
        }

        private void InitializeSampleData()
        {
            // TODO: Replace with actual phone detection
            PhoneStatus = "Phone not detected - Connect phone via USB";
            StoragePath = "Waiting for phone connection...";
        }

        // Properties
        public string PhoneStatus
        {
            get => _phoneStatus;
            set => SetProperty(ref _phoneStatus, value);
        }

        public string PhoneModel
        {
            get => _phoneModel;
            set => SetProperty(ref _phoneModel, value);
        }

        public string StoragePath
        {
            get => _storagePath;
            set => SetProperty(ref _storagePath, value);
        }

        public ObservableCollection<DocumentItem> PhoneFiles
        {
            get => _phoneFiles;
            set => SetProperty(ref _phoneFiles, value);
        }

        public bool IsArchiving
        {
            get => _isArchiving;
            set => SetProperty(ref _isArchiving, value);
        }

        // Commands
        public ICommand RefreshCommand { get; private set; } = null!;
        public ICommand ArchiveCommand { get; private set; } = null!;
        public ICommand ShowSettingsCommand { get; private set; } = null!;

        // Command implementations
        private void RefreshPhoneDetection()
        {
            // TODO: Implement actual phone detection
            // For now, simulate phone detection
            PhoneStatus = "📱 Phone detected - Samsung Galaxy S23";
            PhoneModel = "Samsung Galaxy S23 (SM-S911B)";
            StoragePath = "Computer\\Galaxy S23\\Phone\\DCIM\\Camera";

            // Load sample files
            LoadSamplePhoneFiles();
        }

        private void LoadSamplePhoneFiles()
        {
            var sampleFiles = new[]
            {
                new DocumentItem
                {
                    FileName = "IMG_20240120_143022.jpg",
                    FilePath = "Computer\\Galaxy S23\\Phone\\DCIM\\Camera\\IMG_20240120_143022.jpg",
                    FileType = FileType.Image,
                    FileSize = 4_194_304, // ~4MB
                    AddedTime = DateTime.Now.AddHours(-2),
                    Status = DocumentStatus.Pending
                },
                new DocumentItem
                {
                    FileName = "VID_20240120_150015.mp4",
                    FilePath = "Computer\\Galaxy S23\\Phone\\DCIM\\Camera\\VID_20240120_150015.mp4",
                    FileType = FileType.Video,
                    FileSize = 125_829_120, // ~120MB
                    AddedTime = DateTime.Now.AddHours(-1),
                    Status = DocumentStatus.Pending
                },
                new DocumentItem
                {
                    FileName = "IMG_20240120_152233.jpg",
                    FilePath = "Computer\\Galaxy S23\\Phone\\DCIM\\Camera\\IMG_20240120_152233.jpg",
                    FileType = FileType.Image,
                    FileSize = 3_145_728, // ~3MB
                    AddedTime = DateTime.Now.AddMinutes(-30),
                    Status = DocumentStatus.Pending
                }
            };

            PhoneFiles.Clear();
            foreach (var file in sampleFiles)
            {
                PhoneFiles.Add(file);
            }
        }

        private async Task ArchivePhoneMedia()
        {
            if (PhoneFiles.Count == 0)
            {
                PhoneStatus = "No files to archive";
                return;
            }

            IsArchiving = true;
            PhoneStatus = "Archiving phone media...";

            try
            {
                // Get output archive path
                var outputPath = _fileDialogService.SaveFileDialog(
                    "Save Phone Archive",
                    "OpenArc Archive (*.oarc)|*.oarc",
                    $"phone_archive_{DateTime.Now:yyyyMMdd_HHmmss}.oarc");

                if (string.IsNullOrEmpty(outputPath))
                {
                    PhoneStatus = "Archive cancelled";
                    return;
                }

                // TODO: Implement actual archiving via FFI
                await Task.Delay(2000); // Simulate processing

                PhoneStatus = $"✅ Successfully archived {PhoneFiles.Count} files to {Path.GetFileName(outputPath)}";
            }
            catch (Exception ex)
            {
                PhoneStatus = $"❌ Archive failed: {ex.Message}";
            }
            finally
            {
                IsArchiving = false;
            }
        }

        private void ShowSettings()
        {
            // TODO: Show phone mode specific settings
            PhoneStatus = "Settings - Configure phone detection and compression";
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
