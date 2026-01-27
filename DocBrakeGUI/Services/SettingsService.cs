using Microsoft.Extensions.Logging;
using Newtonsoft.Json;
using System;
using System.IO;
using DocBrake.Models;

namespace DocBrake.Services
{
    public class SettingsService : ISettingsService
    {
        private readonly ILogger<SettingsService> _logger;
        private const string SETTINGS_FILENAME = "OpenArcSettings.json";

        public SettingsService(ILogger<SettingsService> logger)
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
        }

        public ProcessingOptions LoadSettings()
        {
            try
            {
                var settingsPath = GetSettingsPath();
                if (File.Exists(settingsPath))
                {
                    var json = File.ReadAllText(settingsPath);
                    var options = JsonConvert.DeserializeObject<ProcessingOptions>(json);
                    if (options != null)
                    {
                        _logger.LogInformation("Settings loaded from: {SettingsPath}", settingsPath);
                        return options;
                    }
                }
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Failed to load settings, using defaults");
            }

            return GetDefaultSettings();
        }

        public void SaveSettings(ProcessingOptions options)
        {
            try
            {
                var settingsPath = GetSettingsPath();
                var json = JsonConvert.SerializeObject(options, Formatting.Indented);

                Directory.CreateDirectory(Path.GetDirectoryName(settingsPath)!);
                File.WriteAllText(settingsPath, json);

                _logger.LogInformation("Settings saved to: {SettingsPath}", settingsPath);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to save settings");
            }
        }

        private ProcessingOptions GetDefaultSettings()
        {
            // Match CLI defaults from openarc-core OrchestratorSettings
            return new ProcessingOptions
            {
                ArchiveMode = ArchiveMode.Standard,
                OutputArchivePath = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
                    "OpenArc Archives",
                    $"archive_{DateTime.Now:yyyyMMdd}.oarc"),

                // BPG Image Settings (CLI default: quality=25)
                BpgQuality = 25,
                BpgLossless = false,

                // Video Settings (CLI default: H264/Medium, crf=23)
                VideoCodec = VideoCodec.H264,
                VideoSpeed = VideoSpeed.Medium,
                VideoCrf = 23,

                // BPG advanced settings
                BpgBitDepth = 8,
                BpgChromaFormat = 1,
                BpgEncoderType = 0,
                BpgCompressionLevel = 8,

                // Archive Settings (ArcMax level only)
                CompressionLevel = 3,

                // Catalog/Dedup Settings (CLI defaults)
                EnableCatalog = true,
                EnableDedup = true,
                SkipAlreadyCompressedVideos = true,

                // Phone Mode
                PhoneSourcePath = string.Empty,
                AutoDetectPhone = true
            };
        }

        private string GetSettingsPath()
        {
            var appDataPath = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
            return Path.Combine(appDataPath, "OpenArc", SETTINGS_FILENAME);
        }
    }
}
