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
    public class QueueViewModel : INotifyPropertyChanged
    {
        private readonly IQueueService _queueService;
        private DocumentItem? _selectedItem;

        public event PropertyChangedEventHandler? PropertyChanged;

        public QueueViewModel(IQueueService queueService)
        {
            _queueService = queueService;

            RemoveCommand = new RelayCommand(_ => RemoveSelected(), _ => SelectedItem != null);
            ClearCommand = new RelayCommand(_ => _queueService.Clear(), _ => Items.Count > 0);

            Items.CollectionChanged += (_, __) =>
            {
                OnPropertyChanged(nameof(SummaryText));
            };
        }

        public ObservableCollection<DocumentItem> Items => _queueService.Items;

        public DocumentItem? SelectedItem
        {
            get => _selectedItem;
            set
            {
                if (_selectedItem != value)
                {
                    _selectedItem = value;
                    OnPropertyChanged();
                }
            }
        }

        public string SummaryText => $"{Items.Count} items - {FormatSize(Items.Sum(i => i.FileSize))}";

        public ICommand RemoveCommand { get; }
        public ICommand ClearCommand { get; }

        private void RemoveSelected()
        {
            if (SelectedItem != null)
            {
                _queueService.RemoveItem(SelectedItem);
            }
        }

        private static string FormatSize(long bytes)
        {
            if (bytes < 1024) return $"{bytes} B";
            if (bytes < 1024 * 1024) return $"{bytes / 1024.0:F1} KB";
            if (bytes < 1024L * 1024L * 1024L) return $"{bytes / (1024.0 * 1024.0):F1} MB";
            return $"{bytes / (1024.0 * 1024.0 * 1024.0):F1} GB";
        }

        protected virtual void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }
    }
}
