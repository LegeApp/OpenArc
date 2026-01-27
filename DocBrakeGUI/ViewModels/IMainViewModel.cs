using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Windows.Input;
using DocBrake.Models;

namespace DocBrake.ViewModels
{
    public interface IMainViewModel : INotifyPropertyChanged
    {
        ObservableCollection<DocumentItem> QueueFiles { get; }
        DocumentItem? SelectedFile { get; set; }
        ProcessingOptions ProcessingOptions { get; set; }
        bool IsProcessing { get; }
        double OverallProgress { get; }
        string StatusMessage { get; }
        bool ShowSettings { get; set; }

        ICommand AddFileCommand { get; }
        ICommand AddFolderCommand { get; }
        ICommand RemoveFileCommand { get; }
        ICommand ClearQueueCommand { get; }
        ICommand StartProcessingCommand { get; }
        ICommand CancelProcessingCommand { get; }        ICommand ShowSettingsCommand { get; }
        ICommand SaveSettingsCommand { get; }
        ICommand ShowQueueCommand { get; }

        void HandleDroppedFiles(string[] files);
    }
}