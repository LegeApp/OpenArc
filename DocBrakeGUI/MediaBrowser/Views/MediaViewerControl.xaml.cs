using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using DocBrake.MediaBrowser.ViewModels;

namespace DocBrake.MediaBrowser.Views
{
    /// <summary>
    /// Media viewer control with zoom and pan functionality
    /// Displays RGB data from BPG images via WriteableBitmap
    /// </summary>
    public partial class MediaViewerControl : UserControl
    {
        private MediaViewerViewModel? ViewModel => DataContext as MediaViewerViewModel;

        public MediaViewerControl()
        {
            InitializeComponent();

            // Add keyboard handler for "1" key (actual size)
            KeyDown += OnKeyDown;
            Focusable = true;
        }

        private void OnKeyDown(object sender, KeyEventArgs e)
        {
            if (ViewModel == null)
                return;

            // Press "1" for actual size (100%)
            if (e.Key == Key.D1 || e.Key == Key.NumPad1)
            {
                ViewModel.ActualSizeCommand.Execute(null);
                e.Handled = true;
            }
        }

        private void UserControl_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            if (ViewModel == null || ViewModel.CurrentImage == null)
                return;

            // Simple zoom in/out based on wheel direction
            if (e.Delta > 0)
                ViewModel.ZoomInCommand.Execute(null);
            else
                ViewModel.ZoomOutCommand.Execute(null);

            e.Handled = true;
        }

        private void UserControl_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            // Panning is now handled by ScrollViewer - no custom handling needed
        }

        private void UserControl_MouseMove(object sender, MouseEventArgs e)
        {
            // Panning is now handled by ScrollViewer - no custom handling needed
        }

        private void UserControl_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
        {
            // Panning is now handled by ScrollViewer - no custom handling needed
        }

        private void MetadataHoverArea_MouseEnter(object sender, MouseEventArgs e)
        {
            if (MetadataOverlay != null)
            {
                MetadataOverlay.Visibility = Visibility.Visible;
            }
        }

        private void MetadataHoverArea_MouseLeave(object sender, MouseEventArgs e)
        {
            if (MetadataOverlay != null)
            {
                MetadataOverlay.Visibility = Visibility.Collapsed;
            }
        }

        private void ShortcutsHoverArea_MouseEnter(object sender, MouseEventArgs e)
        {
            if (ShortcutsOverlay != null)
            {
                ShortcutsOverlay.Visibility = Visibility.Visible;
            }
        }

        private void ShortcutsHoverArea_MouseLeave(object sender, MouseEventArgs e)
        {
            if (ShortcutsOverlay != null)
            {
                ShortcutsOverlay.Visibility = Visibility.Collapsed;
            }
        }
    }
}
