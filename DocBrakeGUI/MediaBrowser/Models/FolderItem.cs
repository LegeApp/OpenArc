using System;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows.Input;
using DocBrake.Commands;
using DocBrake.Services;

namespace DocBrake.MediaBrowser.Models
{
    public class FolderItem : System.ComponentModel.INotifyPropertyChanged
    {
        private bool _isExpanded;
        private bool _isSelected;
        private bool _hasDummyChild;
        private bool? _isChecked = false;
        private bool _hasExplicitCheckState;
        private bool _isPlaceholder;

        public string Name { get; set; } = string.Empty;
        public string FullPath { get; set; } = string.Empty;
        public string Icon { get; set; } = "📁"; // Simple icon for now

        public FolderItem? Parent { get; private set; }

        public bool IsPhoneRoot { get; set; }
        public bool IsDcimFolder { get; set; }
        
        /// <summary>
        /// True if this folder is on an MTP device
        /// </summary>
        public bool IsMtpPath { get; set; }
        
        /// <summary>
        /// MTP device ID (only set for MTP items)
        /// </summary>
        public string? MtpDeviceId { get; set; }
        
        /// <summary>
        /// MTP object ID (only set for MTP items, null for device root)
        /// </summary>
        public string? MtpObjectId { get; set; }

        public bool HasExplicitCheckState
        {
            get => _hasExplicitCheckState;
            private set
            {
                if (_hasExplicitCheckState != value)
                {
                    _hasExplicitCheckState = value;
                    OnPropertyChanged();
                }
            }
        }

        public ObservableCollection<FolderItem> SubFolders { get; } = new();

        public bool? IsChecked
        {
            get => _isChecked;
            set => SetIsChecked(value, updateChildren: true, updateParent: true, isExplicit: true);
        }

        public bool IsExpanded
        {
            get => _isExpanded;
            set
            {
                if (_isExpanded != value)
                {
                    _isExpanded = value;
                    OnPropertyChanged();
                    if (_isExpanded)
                    {
                        LoadSubFolders();
                    }
                }
            }
        }

        public bool IsSelected
        {
            get => _isSelected;
            set
            {
                if (_isSelected != value)
                {
                    _isSelected = value;
                    OnPropertyChanged();
                }
            }
        }

        public FolderItem(string path, bool isDrive = false)
        {
            FullPath = path;
            Name = isDrive ? path : Path.GetFileName(path);
            
            // Check if this is an MTP shell path and parse MTP properties
            IsMtpPath = IsMtpShellPath(path);
            if (IsMtpPath)
            {
                ParseMtpPath(path);
                Console.WriteLine($"[MTP] Created FolderItem: Name={Name} IsMtp={IsMtpPath} Device={MtpDeviceId} Obj={MtpObjectId}");
            }

            // Check if has subfolders (to show expander)
            try
            {
                bool hasSubfolders = false;
                
                if (IsMtpPath && !string.IsNullOrEmpty(MtpDeviceId))
                {
                    // Use Rust backend for MTP paths - always assume has children for device root
                    hasSubfolders = MtpObjectId == null || CheckMtpHasSubfolders();
                }
                else if (!IsMtpPath)
                {
                    hasSubfolders = Directory.EnumerateDirectories(path).Any();
                }
                
                if (hasSubfolders)
                {
                    var dummy = new FolderItem(true) { Name = string.Empty, Icon = string.Empty };
                    dummy.Parent = this;
                    SubFolders.Add(dummy);
                    _hasDummyChild = true;
                }
            }
            catch { }
        }
        
        /// <summary>
        /// Check if a path is an MTP shell namespace path
        /// </summary>
        private static bool IsMtpShellPath(string path)
        {
            return MtpFileService.IsMtpPath(path);
        }
        
        /// <summary>
        /// Parse mtp:// path to extract device ID and object ID
        /// Format: mtp://deviceId or mtp://deviceId/objectId
        /// </summary>
        private void ParseMtpPath(string path)
        {
            if (!path.StartsWith("mtp://", StringComparison.OrdinalIgnoreCase))
                return;
                
            var rest = path.Substring(6); // Remove "mtp://"
            var slashIndex = rest.IndexOf('/');
            
            if (slashIndex < 0)
            {
                // Just device ID, no object ID (root of device)
                MtpDeviceId = rest;
                MtpObjectId = null;
            }
            else
            {
                MtpDeviceId = rest.Substring(0, slashIndex);
                MtpObjectId = rest.Substring(slashIndex + 1);
            }
        }
        
        /// <summary>
        /// Check if an MTP folder has subfolders
        /// </summary>
        private bool CheckMtpHasSubfolders()
        {
            if (string.IsNullOrEmpty(MtpDeviceId)) return false;
            return MtpFileService.HasSubfolders(MtpDeviceId, MtpObjectId);
        }

        private FolderItem(bool isPlaceholder)
        {
            _isPlaceholder = isPlaceholder;
        }

        private void LoadSubFolders()
        {
            if (_hasDummyChild)
            {
                SubFolders.Clear();
                _hasDummyChild = false;

                try
                {
                    if (IsMtpPath || IsMtpShellPath(FullPath))
                    {
                        // Use Shell COM for MTP paths
                        LoadMtpSubFolders();
                    }
                    else
                    {
                        // Regular filesystem path
                        var dirs = Directory.GetDirectories(FullPath);
                        foreach (var dir in dirs)
                        {
                            try
                            {
                                // Skip hidden folders if desired, or access denied
                                var info = new DirectoryInfo(dir);
                                if (!info.Attributes.HasFlag(FileAttributes.Hidden))
                                {
                                    var child = new FolderItem(dir) { Parent = this };
                                    if (IsPhoneRoot && string.Equals(child.Name, "DCIM", StringComparison.OrdinalIgnoreCase))
                                    {
                                        child.IsDcimFolder = true;
                                        child.Icon = "📸";
                                    }

                                    if (IsChecked.HasValue)
                                    {
                                        child.SetIsChecked(IsChecked, updateChildren: true, updateParent: false, isExplicit: false);
                                    }

                                    SubFolders.Add(child);
                                }
                            }
                            catch { }
                        }
                    }
                }
                catch { }
            }
        }
        
        /// <summary>
        /// Load subfolders from an MTP folder using MtpFileService
        /// </summary>
        private void LoadMtpSubFolders()
        {
            if (string.IsNullOrEmpty(MtpDeviceId)) return;

            try
            {
                var result = DocBrake.NativeInterop.OpenArcFFI.GetMtpFolderContents(MtpDeviceId, MtpObjectId);
                if (!result.success || result.data == null) 
                {
                    Console.WriteLine($"[MTP] Failed to load subfolders for {Name}: {result.error}");
                    return;
                }

                var folders = result.data.Where(i => i.is_folder).ToList();
                Console.WriteLine($"[MTP] Loaded {folders.Count} subfolders for {Name} (ID: {MtpObjectId ?? "ROOT"})");

                foreach (var item in folders)
                {
                    var child = new FolderItem(true) // Use placeholder constructor
                    {
                        Name = item.name,
                        FullPath = $"mtp://{MtpDeviceId}/{item.id}",
                        IsMtpPath = true,
                        MtpDeviceId = MtpDeviceId,
                        MtpObjectId = item.id,
                        Parent = this
                    };
                    
                    // Re-init: check for subfolders (defer for performance)
                    child._isPlaceholder = false;
                    
                    // Assume has subfolders initially, will verify on expand
                    var dummy = new FolderItem(true) { Name = string.Empty, Icon = string.Empty };
                    dummy.Parent = child;
                    child.SubFolders.Add(dummy);
                    child._hasDummyChild = true;
                    
                    // Set appropriate icons based on folder name
                    child.Icon = GetMtpFolderIcon(item.name, out bool isPhoneRoot, out bool isDcim);
                    child.IsPhoneRoot = isPhoneRoot;
                    child.IsDcimFolder = isDcim;
                    
                    if (IsChecked.HasValue)
                    {
                        child.SetIsChecked(IsChecked, updateChildren: true, updateParent: false, isExplicit: false);
                    }
                    
                    SubFolders.Add(child);
                }
            }
            catch { }
        }

        /// <summary>
        /// Get appropriate icon for MTP folder based on name
        /// </summary>
        private static string GetMtpFolderIcon(string name, out bool isPhoneRoot, out bool isDcim)
        {
            isPhoneRoot = false;
            isDcim = false;
            
            var lower = name.ToLowerInvariant();
            
            if (lower == "dcim" || lower == "camera")
            {
                isDcim = lower == "dcim";
                return "📸";
            }
            if (lower.Contains("internal") || lower == "phone")
            {
                isPhoneRoot = true;
                return "💾";
            }
            if (lower.Contains("sd") || lower == "card")
                return "💳";
            if (lower == "pictures" || lower == "photos")
                return "🖼️";
            if (lower == "movies" || lower == "videos")
                return "🎬";
            if (lower == "music")
                return "🎵";
            if (lower == "download" || lower == "downloads")
                return "📥";
            
            return "📁";
        }

        private void SetIsChecked(bool? value, bool updateChildren, bool updateParent, bool isExplicit)
        {
            if (_isPlaceholder)
                return;

            if (_isChecked == value)
                return;

            _isChecked = value;
            OnPropertyChanged(nameof(IsChecked));

            if (isExplicit)
            {
                HasExplicitCheckState = true;
            }

            if (updateChildren && value.HasValue)
            {
                foreach (var child in SubFolders.Where(c => !c._isPlaceholder))
                {
                    child.SetIsChecked(value, updateChildren: true, updateParent: false, isExplicit: false);
                }
            }

            if (updateParent && Parent != null)
            {
                Parent.UpdateCheckStateFromChildren();
            }
        }

        private void UpdateCheckStateFromChildren()
        {
            if (_isPlaceholder)
                return;

            var children = SubFolders.Where(c => !c._isPlaceholder).ToList();
            if (children.Count == 0)
                return;

            bool allTrue = children.All(c => c.IsChecked == true);
            bool allFalse = children.All(c => c.IsChecked == false);
            bool? newValue = allTrue ? true : (allFalse ? false : (bool?)null);

            SetIsChecked(newValue, updateChildren: false, updateParent: true, isExplicit: false);
        }

        public event System.ComponentModel.PropertyChangedEventHandler? PropertyChanged;
        protected void OnPropertyChanged([System.Runtime.CompilerServices.CallerMemberName] string? name = null)
        {
            PropertyChanged?.Invoke(this, new System.ComponentModel.PropertyChangedEventArgs(name));
        }
    }
}
