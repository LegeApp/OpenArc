using System.Windows;
using DocBrake.ViewModels;

namespace DocBrake.Views
{
    /// <summary>
    /// Interaction logic for QueueWindow.xaml
    /// </summary>
    public partial class QueueWindow : Window
    {
        public QueueWindow()
        {
            InitializeComponent();
        }

        public QueueWindow(MainViewModel viewModel) : this()
        {
            DataContext = viewModel;
        }
    }
}
