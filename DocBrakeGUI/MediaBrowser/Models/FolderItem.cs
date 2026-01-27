using System;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Windows.Input;
using DocBrake.Commands;

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

            // Check if has subfolders (to show expander)
            try
            {
                if (Directory.EnumerateDirectories(path).Any())
                {
                    var dummy = new FolderItem(true) { Name = string.Empty, Icon = string.Empty };
                    dummy.Parent = this;
                    SubFolders.Add(dummy);
                    _hasDummyChild = true;
                }
            }
            catch { }
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
                catch { }
            }
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
