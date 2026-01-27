using System;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Windows.Input;
using BpgViewer.Commands;

namespace BpgViewer.Models
{
    public class FolderItem : System.ComponentModel.INotifyPropertyChanged
    {
        private bool _isExpanded;
        private bool _isSelected;
        private bool _hasDummyChild;

        public string Name { get; set; } = string.Empty;
        public string FullPath { get; set; } = string.Empty;
        public string Icon { get; set; } = "📁"; // Simple icon for now

        public ObservableCollection<FolderItem> SubFolders { get; } = new();

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
                    SubFolders.Add(new FolderItem { Name = "Dummy" }); // Dummy item to show expander
                    _hasDummyChild = true;
                }
            }
            catch { }
        }

        // Private constructor for dummy
        private FolderItem() { }

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
                                SubFolders.Add(new FolderItem(dir));
                            }
                        }
                        catch { }
                    }
                }
                catch { }
            }
        }

        public event System.ComponentModel.PropertyChangedEventHandler? PropertyChanged;
        protected void OnPropertyChanged([System.Runtime.CompilerServices.CallerMemberName] string? name = null)
        {
            PropertyChanged?.Invoke(this, new System.ComponentModel.PropertyChangedEventArgs(name));
        }
    }
}
