using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Windows.Input;
using DocBrake.Commands;
using DocBrake.Models;
using DocBrake.Services;

namespace DocBrake.ViewModels
{
    public class SettingsViewModel : INotifyPropertyChanged
    {
        private readonly IFileDialogService _fileDialogService;
        private readonly ISettingsService _settingsService;
        private ProcessingOptions _options;

        private const int BpgBitDepthMin = 8;
        private const int BpgBitDepthMaxX265 = 12;
        private const int BpgBitDepthMaxJctvc = 14;

        public event PropertyChangedEventHandler? PropertyChanged;

        public SettingsViewModel(IFileDialogService fileDialogService, ISettingsService settingsService)
        {
            _fileDialogService = fileDialogService ?? throw new ArgumentNullException(nameof(fileDialogService));
            _settingsService = settingsService ?? throw new ArgumentNullException(nameof(settingsService));
            _options = _settingsService.LoadSettings();

            InitializeCommands();
        }

        public ProcessingOptions Options => _options;

        // Enum options for ComboBox bindings
        public IEnumerable<VideoCodec> VideoCodecOptions => Enum.GetValues(typeof(VideoCodec)).Cast<VideoCodec>();
        public IEnumerable<VideoSpeed> VideoSpeedOptions => Enum.GetValues(typeof(VideoSpeed)).Cast<VideoSpeed>();
        public IEnumerable<ArchiveMode> ArchiveModeOptions => Enum.GetValues(typeof(ArchiveMode)).Cast<ArchiveMode>();

        /// <summary>
        /// Sync with an external ProcessingOptions instance (used by MainViewModel)
        /// </summary>
        public void SyncOptions(ProcessingOptions options)
        {
            if (options == null) throw new ArgumentNullException(nameof(options));
            _options = options;
            
            // Notify all properties to update UI
            OnPropertyChanged(nameof(BpgQuality));
            OnPropertyChanged(nameof(BpgBitDepth));
            OnPropertyChanged(nameof(BpgEncoderType));
            OnPropertyChanged(nameof(BpgBitDepthMax));
            OnPropertyChanged(nameof(BpgBitDepthToolTip));
            OnPropertyChanged(nameof(BpgCompressionLevel));
            OnPropertyChanged(nameof(VideoCodec));
            OnPropertyChanged(nameof(VideoSpeed));
            OnPropertyChanged(nameof(VideoCrf));
            OnPropertyChanged(nameof(CompressionLevel));
            OnPropertyChanged(nameof(OutputArchivePath));
            OnPropertyChanged(nameof(PhoneSourcePath));
            OnPropertyChanged(nameof(AutoDetectPhone));
            OnPropertyChanged(nameof(EnableCatalog));
            OnPropertyChanged(nameof(EnableDedup));
            OnPropertyChanged(nameof(SkipAlreadyCompressedVideos));
        }

        // BPG Image Settings
        public int BpgQuality
        {
            get => _options.BpgQuality;
            set
            {
                _options.BpgQuality = value;
                OnPropertyChanged();
            }
        }

        public int BpgBitDepth
        {
            get => _options.BpgBitDepth;
            set
            {
                _options.BpgBitDepth = Math.Clamp(value, BpgBitDepthMin, BpgBitDepthMax);
                OnPropertyChanged();
            }
        }

        public int BpgBitDepthMax => BpgEncoderType == 1 ? BpgBitDepthMaxJctvc : BpgBitDepthMaxX265;

        public string BpgBitDepthToolTip =>
            "Preferred bit depth for BPG encoding when the source has >8-bit channels. " +
            "JPEG is always 8-bit. HEIC/HEIF is derived from the input. Most relevant for RAW/PNG/TIFF sources.";

        public int BpgEncoderType
        {
            get => _options.BpgEncoderType;
            set
            {
                _options.BpgEncoderType = value;
                OnPropertyChanged();

                // Encoder type changes the supported bit depth range.
                OnPropertyChanged(nameof(BpgBitDepthMax));
                OnPropertyChanged(nameof(BpgBitDepthToolTip));

                if (_options.BpgBitDepth > BpgBitDepthMax)
                {
                    _options.BpgBitDepth = BpgBitDepthMax;
                    OnPropertyChanged(nameof(BpgBitDepth));
                }
            }
        }

        public int BpgCompressionLevel
        {
            get => _options.BpgCompressionLevel;
            set
            {
                _options.BpgCompressionLevel = value;
                OnPropertyChanged();
            }
        }

        // FFmpeg Video Settings
        public VideoCodec VideoCodec
        {
            get => _options.VideoCodec;
            set
            {
                _options.VideoCodec = value;
                OnPropertyChanged();
            }
        }

        public VideoSpeed VideoSpeed
        {
            get => _options.VideoSpeed;
            set
            {
                _options.VideoSpeed = value;
                OnPropertyChanged();
            }
        }

        public int VideoCrf
        {
            get => _options.VideoCrf;
            set
            {
                _options.VideoCrf = value;
                OnPropertyChanged();
            }
        }

        // Archive Settings
        public int CompressionLevel
        {
            get => _options.CompressionLevel;
            set
            {
                _options.CompressionLevel = value;
                OnPropertyChanged();
            }
        }

        // Output Settings
        public string OutputArchivePath
        {
            get => _options.OutputArchivePath;
            set
            {
                _options.OutputArchivePath = value;
                OnPropertyChanged();
            }
        }

        // Phone Mode Settings
        public string PhoneSourcePath
        {
            get => _options.PhoneSourcePath;
            set
            {
                _options.PhoneSourcePath = value;
                OnPropertyChanged();
            }
        }

        public bool AutoDetectPhone
        {
            get => _options.AutoDetectPhone;
            set
            {
                _options.AutoDetectPhone = value;
                OnPropertyChanged();
            }
        }
        
        // UI Settings
        public bool ShowThumbnailLabelsByDefault
        {
            get => _options.ShowThumbnailLabelsByDefault;
            set
            {
                _options.ShowThumbnailLabelsByDefault = value;
                OnPropertyChanged();
            }
        }

        // Catalog & Dedup Settings
        public bool EnableCatalog
        {
            get => _options.EnableCatalog;
            set
            {
                _options.EnableCatalog = value;
                OnPropertyChanged();
            }
        }

        public bool EnableDedup
        {
            get => _options.EnableDedup;
            set
            {
                _options.EnableDedup = value;
                OnPropertyChanged();
            }
        }

        public bool SkipAlreadyCompressedVideos
        {
            get => _options.SkipAlreadyCompressedVideos;
            set
            {
                _options.SkipAlreadyCompressedVideos = value;
                OnPropertyChanged();
            }
        }

        // Commands
        public ICommand BrowseOutputArchiveCommand { get; private set; } = null!;
        public ICommand BrowsePhoneSourceCommand { get; private set; } = null!;
        public ICommand ResetToDefaultsCommand { get; private set; } = null!;
        public ICommand SaveSettingsCommand { get; private set; } = null!;

        private void InitializeCommands()
        {
            BrowseOutputArchiveCommand = new RelayCommand(_ => BrowseOutputArchive());
            BrowsePhoneSourceCommand = new RelayCommand(_ => BrowsePhoneSource());
            ResetToDefaultsCommand = new RelayCommand(_ => ResetToDefaults());
            SaveSettingsCommand = new RelayCommand(_ => SaveSettings());
        }

        private void BrowseOutputArchive()
        {
            var file = _fileDialogService.SaveFileDialog(
                "Save Archive As",
                "Zstd Archive (*.zstd)|*.zstd|OpenArc Archive (*.oarc)|*.oarc|All Files (*.*)|*.*",
                "archive.zstd");
            
            if (!string.IsNullOrEmpty(file))
            {
                OutputArchivePath = file;
            }
        }

        private void BrowsePhoneSource()
        {
            var directory = _fileDialogService.OpenFolderDialog("Select Phone Storage Location", PhoneSourcePath);
            if (!string.IsNullOrEmpty(directory))
            {
                PhoneSourcePath = directory;
            }
        }

        private void ResetToDefaults()
        {
            var defaultSettings = _settingsService.GetDefaultSettings();

            // Image settings
            BpgQuality = defaultSettings.BpgQuality;
            BpgBitDepth = defaultSettings.BpgBitDepth;
            BpgEncoderType = defaultSettings.BpgEncoderType;
            BpgCompressionLevel = defaultSettings.BpgCompressionLevel;

            // Video settings
            VideoCodec = defaultSettings.VideoCodec;
            VideoSpeed = defaultSettings.VideoSpeed;
            VideoCrf = defaultSettings.VideoCrf;

            // Archive settings
            CompressionLevel = defaultSettings.CompressionLevel;

            // Output settings
            OutputArchivePath = defaultSettings.OutputArchivePath;

            // Catalog & dedup settings
            EnableCatalog = defaultSettings.EnableCatalog;
            EnableDedup = defaultSettings.EnableDedup;
            SkipAlreadyCompressedVideos = defaultSettings.SkipAlreadyCompressedVideos;

            // Phone mode settings
            PhoneSourcePath = defaultSettings.PhoneSourcePath;
            AutoDetectPhone = defaultSettings.AutoDetectPhone;
        }

        private void SaveSettings()
        {
            _settingsService.SaveSettings(_options);
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