using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Windows.Data;
using DocBrake.Models;

namespace DocBrake.Services
{
    public interface IQueueService
    {
        ObservableCollection<DocumentItem> Items { get; }
        int Count { get; }

        void AddFile(string filePath);
        void AddFolder(string folderPath);
        void RemoveFolder(string folderPath);
        void RemoveItem(DocumentItem item);
        void Clear();
        bool Contains(string filePath);
        void SortPendingByType();
    }

    public class QueueService : IQueueService
    {
        private static readonly HashSet<string> MediaExtensions = new(StringComparer.OrdinalIgnoreCase)
        {
            ".bpg", ".jpg", ".jpeg", ".png", ".bmp", ".tiff", ".tif", ".webp", ".gif", ".heic", ".heif",
            ".dng", ".raw", ".cr2", ".nef", ".arw", ".orf", ".rw2", ".raf", ".3fr", ".fff", ".dcr",
            ".kdc", ".srf", ".sr2", ".erf", ".mef", ".mrw", ".nrw", ".pef", ".iiq", ".x3f", ".jp2",
            ".j2k", ".j2c", ".jpc", ".jpt", ".jph", ".jhc", ".mp4", ".mov", ".avi", ".mkv", ".webm"
        };

        private static readonly HashSet<string> ImageExtensions = new(StringComparer.OrdinalIgnoreCase)
        {
            ".bpg", ".jpg", ".jpeg", ".png", ".bmp", ".tiff", ".tif", ".webp", ".gif", ".heic", ".heif",
            ".dng", ".raw", ".cr2", ".nef", ".arw", ".orf", ".rw2", ".raf", ".3fr", ".fff", ".dcr",
            ".kdc", ".srf", ".sr2", ".erf", ".mef", ".mrw", ".nrw", ".pef", ".iiq", ".x3f", ".jp2",
            ".j2k", ".j2c", ".jpc", ".jpt", ".jph", ".jhc"
        };

        private static readonly HashSet<string> VideoExtensions = new(StringComparer.OrdinalIgnoreCase)
        {
            ".mp4", ".mov", ".avi", ".mkv", ".webm"
        };

        private readonly ObservableCollection<DocumentItem> _items = new();
        private readonly object _lock = new();

        private readonly HashSet<string> _manualFiles = new(StringComparer.OrdinalIgnoreCase);
        private readonly HashSet<string> _trackedFolders = new(StringComparer.OrdinalIgnoreCase);
        private readonly Dictionary<string, HashSet<string>> _folderFiles = new(StringComparer.OrdinalIgnoreCase);

        public ObservableCollection<DocumentItem> Items => _items;

        public int Count
        {
            get
            {
                lock (_lock)
                {
                    return _items.Count;
                }
            }
        }

        public QueueService()
        {
            BindingOperations.EnableCollectionSynchronization(_items, _lock);
        }

        public void AddFile(string filePath)
        {
            AddFileInternal(filePath, originFolder: null);
        }

        public void AddFolder(string folderPath)
        {
            if (string.IsNullOrWhiteSpace(folderPath) || !Directory.Exists(folderPath))
                return;

            lock (_lock)
            {
                if (_trackedFolders.Contains(folderPath))
                    return;

                _trackedFolders.Add(folderPath);
            }

            IEnumerable<string> files;
            try
            {
                files = Directory.EnumerateFiles(folderPath, "*.*", SearchOption.AllDirectories)
                    .Where(f => MediaExtensions.Contains(Path.GetExtension(f)));
            }
            catch
            {
                return;
            }

            var added = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            foreach (var file in files)
            {
                if (AddFileInternal(file, originFolder: folderPath))
                {
                    added.Add(file);
                }
            }

            lock (_lock)
            {
                if (!_folderFiles.TryGetValue(folderPath, out var set))
                {
                    set = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
                    _folderFiles[folderPath] = set;
                }

                foreach (var f in added)
                    set.Add(f);
            }
        }

        public void RemoveFolder(string folderPath)
        {
            if (string.IsNullOrWhiteSpace(folderPath))
                return;

            HashSet<string>? filesToRemove;
            lock (_lock)
            {
                if (!_trackedFolders.Remove(folderPath))
                    return;

                _folderFiles.TryGetValue(folderPath, out filesToRemove);
                _folderFiles.Remove(folderPath);
            }

            if (filesToRemove == null || filesToRemove.Count == 0)
                return;

            lock (_lock)
            {
                foreach (var f in filesToRemove)
                {
                    if (_manualFiles.Contains(f))
                        continue;

                    var stillReferenced = _folderFiles.Values.Any(set => set.Contains(f));
                    if (stillReferenced)
                        continue;

                    var item = _items.FirstOrDefault(i => i.FilePath.Equals(f, StringComparison.OrdinalIgnoreCase));
                    if (item != null)
                    {
                        _items.Remove(item);
                    }
                }
            }
        }

        public void RemoveItem(DocumentItem item)
        {
            if (item == null)
                return;

            lock (_lock)
            {
                _manualFiles.Remove(item.FilePath);
                foreach (var kv in _folderFiles)
                {
                    kv.Value.Remove(item.FilePath);
                }

                _items.Remove(item);
            }
        }

        public void Clear()
        {
            lock (_lock)
            {
                _items.Clear();
                _manualFiles.Clear();
                _trackedFolders.Clear();
                _folderFiles.Clear();
            }
        }

        public bool Contains(string filePath)
        {
            if (string.IsNullOrWhiteSpace(filePath))
                return false;

            lock (_lock)
            {
                return _items.Any(i => i.FilePath.Equals(filePath, StringComparison.OrdinalIgnoreCase));
            }
        }

        public void SortPendingByType()
        {
            lock (_lock)
            {
                if (_items.Count <= 1)
                    return;

                var pending = _items
                    .Where(f => f.Status == DocumentStatus.Pending)
                    .OrderBy(f => f.FileType == FileType.Image ? 0 : (f.FileType == FileType.Video ? 1 : 2))
                    .ThenBy(f => f.FileName, StringComparer.OrdinalIgnoreCase)
                    .ToList();

                var nonPending = _items
                    .Where(f => f.Status != DocumentStatus.Pending)
                    .ToList();

                _items.Clear();
                foreach (var item in pending)
                    _items.Add(item);
                foreach (var item in nonPending)
                    _items.Add(item);
            }
        }

        private bool AddFileInternal(string filePath, string? originFolder)
        {
            if (string.IsNullOrWhiteSpace(filePath) || !File.Exists(filePath))
                return false;

            var ext = Path.GetExtension(filePath);
            if (!MediaExtensions.Contains(ext))
                return false;

            lock (_lock)
            {
                if (_items.Any(i => i.FilePath.Equals(filePath, StringComparison.OrdinalIgnoreCase)))
                    return false;

                var item = CreateDocumentItem(filePath);
                _items.Add(item);

                if (originFolder == null)
                {
                    _manualFiles.Add(filePath);
                }

                return true;
            }
        }

        private static DocumentItem CreateDocumentItem(string filePath)
        {
            var fileInfo = new FileInfo(filePath);
            var ext = fileInfo.Extension;

            var fileType = FileType.Unknown;
            if (ImageExtensions.Contains(ext))
                fileType = FileType.Image;
            else if (VideoExtensions.Contains(ext))
                fileType = FileType.Video;

            return new DocumentItem
            {
                FilePath = filePath,
                FileName = fileInfo.Name,
                FileSize = fileInfo.Length,
                FileType = fileType,
                Status = DocumentStatus.Pending,
                AddedTime = DateTime.Now
            };
        }
    }
}
