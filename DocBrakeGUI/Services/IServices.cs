using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using DocBrake.Models;
using DocBrake.NativeInterop;

namespace DocBrake.Services
{
    public interface IDocumentProcessingService
    {
        event EventHandler<DocumentProcessingProgress>? ProgressUpdated;
        event EventHandler<DocumentProcessingResult>? ProcessingCompleted;
        event EventHandler<string>? ProcessingError;

        Task<bool> ValidateDocumentAsync(string filePath, CancellationToken cancellationToken = default);
        Task<DocumentProcessingResult> ProcessDocumentAsync(string inputPath, ProcessingOptions options, IProgress<DocumentProcessingProgress>? progress = null, CancellationToken cancellationToken = default);
        Task<List<DocumentProcessingResult>> ProcessDocumentsAsync(IEnumerable<string> inputPaths, ProcessingOptions options, IProgress<DocumentProcessingProgress>? progress = null, CancellationToken cancellationToken = default);
        void CancelProcessing();
        
        // Archive operations
        Task<bool> ExtractArchiveAsync(string archivePath, string outputDirectory, IProgress<DocumentProcessingProgress>? progress = null, CancellationToken cancellationToken = default);
        Task<List<ArchiveFileInfo>> ListArchiveAsync(string archivePath, CancellationToken cancellationToken = default);
        Task<bool> ExtractArchiveEntryAsync(string archivePath, string entryName, string outputPath, CancellationToken cancellationToken = default);

        // Single file encoding (for testing)
        Task<bool> EncodeBpgFileAsync(string inputPath, string outputPath, ProcessingOptions options, CancellationToken cancellationToken = default);
        Task<bool> EncodeVideoFileAsync(string inputPath, string outputPath, ProcessingOptions options, CancellationToken cancellationToken = default);
    }

    public interface ISettingsService
    {
        ProcessingOptions LoadSettings();
        ProcessingOptions GetDefaultSettings();
        void SaveSettings(ProcessingOptions options);
    }

    public interface IFileDialogService
    {
        string? OpenFileDialog(string title, string filter, string? initialDirectory = null);
        string[]? OpenFilesDialog(string title, string filter, string? initialDirectory = null);
        string? OpenFolderDialog(string title, string? initialDirectory = null);
        string? SaveFileDialog(string title, string filter, string? defaultFileName = null, string? initialDirectory = null);
    }
}
