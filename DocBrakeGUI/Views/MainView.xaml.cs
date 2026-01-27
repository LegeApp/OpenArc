using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;

namespace DocBrake.Views
{
    public partial class MainView : UserControl
    {
        public MainView()
        {
            InitializeComponent();
        }

        private void MainView_PreviewDragOver(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                e.Effects = DragDropEffects.Copy;
                e.Handled = true;
            }
        }

        private void MainView_DragEnter(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                DragDropOverlay.Visibility = Visibility.Visible;
            }
        }

        private void MainView_DragLeave(object sender, DragEventArgs e)
        {
            // Check if we're actually leaving the control (not just entering a child element)
            var position = e.GetPosition(this);
            var bounds = new Rect(0, 0, ActualWidth, ActualHeight);

            if (!bounds.Contains(position))
            {
                DragDropOverlay.Visibility = Visibility.Collapsed;
            }
        }

        private void MainView_Drop(object sender, DragEventArgs e)
        {
            DragDropOverlay.Visibility = Visibility.Collapsed;

            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                var files = (string[])e.Data.GetData(DataFormats.FileDrop);
                if (DataContext is ViewModels.MainViewModel viewModel)
                {
                    viewModel.HandleDroppedFiles(files);
                }
            }
        }
    }
}
