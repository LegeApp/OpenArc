using System;
using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace DocBrake.Models
{
    public enum ArchiveMode
    {
        Phone,
        Standard,
        MediaBrowser
    }

    public enum VideoCodec
    {
        H264 = 0,
        H265 = 1
    }

    public enum VideoSpeed
    {
        Fast = 0,
        Medium = 1,
        Slow = 2
    }

    /// <summary>
    /// Processing options matching CLI settings from openarc-core OrchestratorSettings.
    /// </summary>
    public class ProcessingOptions : INotifyPropertyChanged
    {
        private ArchiveMode _archiveMode = ArchiveMode.Phone;
        private string _outputArchivePath = string.Empty;

        // BPG Image Settings (CLI: --bpg-quality, --bpg-lossless)
        private int _bpgQuality = 25;
        private bool _bpgLossless = false;
        private int _bpgBitDepth = 8;
        private int _bpgChromaFormat = 2; // Default to RGB (high chroma)
        private int _bpgEncoderType = 0;
        private int _bpgCompressionLevel = 8;

        // FFmpeg Video Settings (CLI: --video-preset, --video-crf)
        private VideoCodec _videoCodec = VideoCodec.H264;
        private VideoSpeed _videoSpeed = VideoSpeed.Medium;
        private int _videoCrf = 23;

        // Archive Settings (ArcMax only) - default to maximum compression
        private int _compressionLevel = 22;

        // Catalog & Dedup Settings (CLI: --no-catalog, --no-dedup, --no-skip-compressed)
        private bool _enableCatalog = true;
        private bool _enableDedup = true;
        private bool _skipAlreadyCompressedVideos = true;

        // Phone Mode Settings
        private string _phoneSourcePath = string.Empty;
        private bool _autoDetectPhone = true;

        public event PropertyChangedEventHandler? PropertyChanged;

        public ArchiveMode ArchiveMode
        {
            get => _archiveMode;
            set => SetProperty(ref _archiveMode, value);
        }

        public string OutputArchivePath
        {
            get => _outputArchivePath;
            set => SetProperty(ref _outputArchivePath, value);
        }

        // BPG Image Settings
        public int BpgQuality
        {
            get => _bpgQuality;
            set => SetProperty(ref _bpgQuality, Math.Clamp(value, 0, 51));
        }

        public bool BpgLossless
        {
            get => _bpgLossless;
            set => SetProperty(ref _bpgLossless, value);
        }

        public int BpgBitDepth
        {
            get => _bpgBitDepth;
            set => SetProperty(ref _bpgBitDepth, Math.Clamp(value, 8, 12));
        }

        public int BpgChromaFormat
        {
            get => _bpgChromaFormat;
            set => SetProperty(ref _bpgChromaFormat, Math.Clamp(value, 0, 2));
        }

        public int BpgEncoderType
        {
            get => _bpgEncoderType;
            set => SetProperty(ref _bpgEncoderType, Math.Clamp(value, 0, 1));
        }

        public int BpgCompressionLevel
        {
            get => _bpgCompressionLevel;
            set => SetProperty(ref _bpgCompressionLevel, Math.Clamp(value, 1, 9));
        }

        // FFmpeg Video Settings
        public VideoCodec VideoCodec
        {
            get => _videoCodec;
            set => SetProperty(ref _videoCodec, value);
        }

        public VideoSpeed VideoSpeed
        {
            get => _videoSpeed;
            set => SetProperty(ref _videoSpeed, value);
        }

        public int VideoCrf
        {
            get => _videoCrf;
            set => SetProperty(ref _videoCrf, Math.Clamp(value, 0, 51));
        }

        public int CompressionLevel
        {
            get => _compressionLevel;
            set => SetProperty(ref _compressionLevel, Math.Clamp(value, 1, 22));
        }

        // Catalog & Dedup Settings (matching CLI flags)
        public bool EnableCatalog
        {
            get => _enableCatalog;
            set => SetProperty(ref _enableCatalog, value);
        }

        public bool EnableDedup
        {
            get => _enableDedup;
            set => SetProperty(ref _enableDedup, value);
        }

        public bool SkipAlreadyCompressedVideos
        {
            get => _skipAlreadyCompressedVideos;
            set => SetProperty(ref _skipAlreadyCompressedVideos, value);
        }

        // Phone Mode Settings
        public string PhoneSourcePath
        {
            get => _phoneSourcePath;
            set => SetProperty(ref _phoneSourcePath, value);
        }

        public bool AutoDetectPhone
        {
            get => _autoDetectPhone;
            set => SetProperty(ref _autoDetectPhone, value);
        }

        public ProcessingOptions Clone()
        {
            return new ProcessingOptions
            {
                ArchiveMode = ArchiveMode,
                OutputArchivePath = OutputArchivePath,

                BpgQuality = BpgQuality,
                BpgLossless = BpgLossless,
                BpgBitDepth = BpgBitDepth,
                BpgChromaFormat = BpgChromaFormat,
                BpgEncoderType = BpgEncoderType,
                BpgCompressionLevel = BpgCompressionLevel,

                VideoCodec = VideoCodec,
                VideoSpeed = VideoSpeed,
                VideoCrf = VideoCrf,

                CompressionLevel = CompressionLevel,
                EnableCatalog = EnableCatalog,
                EnableDedup = EnableDedup,
                SkipAlreadyCompressedVideos = SkipAlreadyCompressedVideos,

                PhoneSourcePath = PhoneSourcePath,
                AutoDetectPhone = AutoDetectPhone
            };
        }

        public void CopyFrom(ProcessingOptions other)
        {
            if (other == null) throw new ArgumentNullException(nameof(other));

            ArchiveMode = other.ArchiveMode;
            OutputArchivePath = other.OutputArchivePath;

            BpgQuality = other.BpgQuality;
            BpgLossless = other.BpgLossless;
            BpgBitDepth = other.BpgBitDepth;
            BpgChromaFormat = other.BpgChromaFormat;
            BpgEncoderType = other.BpgEncoderType;
            BpgCompressionLevel = other.BpgCompressionLevel;

            VideoCodec = other.VideoCodec;
            VideoSpeed = other.VideoSpeed;
            VideoCrf = other.VideoCrf;

            CompressionLevel = other.CompressionLevel;
            EnableCatalog = other.EnableCatalog;
            EnableDedup = other.EnableDedup;
            SkipAlreadyCompressedVideos = other.SkipAlreadyCompressedVideos;

            PhoneSourcePath = other.PhoneSourcePath;
            AutoDetectPhone = other.AutoDetectPhone;
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
