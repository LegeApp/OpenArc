using System;
using System.Windows;
using System.Windows.Input;
using BpgViewer.ViewModels;

namespace BpgViewer
{
    /// <summary>
    /// Standalone window for viewing single BPG images
    /// </summary>
    public partial class ImageViewerWindow : Window
    {
        public ImageViewerViewModel ViewModel { get; }

        public ImageViewerWindow()
        {
            InitializeComponent();
            ViewModel = (ImageViewerViewModel)DataContext;

            // Keyboard shortcuts
            KeyDown += OnKeyDown;
        }

        public ImageViewerWindow(string filePath) : this()
        {
            // Load the file after window is fully loaded
            Loaded += (s, e) =>
            {
                try
                {
                    if (ViewModel != null)
                    {
                        // Load image
                        ViewModel.LoadImage(filePath);

                        // Auto-fit to window with no leftover space
                        Dispatcher.InvokeAsync(() =>
                        {
                            if (ViewModel.CurrentImage != null)
                            {
                                // Calculate available size (subtract menu and status bar heights)
                                double availableHeight = ActualHeight - 80; // Menu + status bar approx
                                double availableWidth = ActualWidth;
                                ViewModel.FitToWindowCommand.Execute(new Size(availableWidth, availableHeight));
                            }
                        }, System.Windows.Threading.DispatcherPriority.Loaded);
                    }
                }
                catch (Exception ex)
                {
                    MessageBox.Show($"Error loading image: {ex.Message}", "Error",
                        MessageBoxButton.OK, MessageBoxImage.Error);
                }
            };
        }

        private void OnKeyDown(object sender, KeyEventArgs e)
        {
            if (e.Key == Key.Escape)
            {
                Close();
                e.Handled = true;
            }
            else if (e.Key == Key.D0 || e.Key == Key.NumPad0)
            {
                // 0 = Fit to window
                double availableHeight = ActualHeight - 80;
                double availableWidth = ActualWidth;
                ViewModel.FitToWindowCommand.Execute(new Size(availableWidth, availableHeight));
                e.Handled = true;
            }
            else if (e.Key == Key.F)
            {
                // F = Toggle between fit-to-window and actual size
                if (Math.Abs(ViewModel.ZoomLevel - 1.0) < 0.01)
                {
                    // Currently at 100%, switch to fit
                    double availableHeight = ActualHeight - 80;
                    double availableWidth = ActualWidth;
                    ViewModel.FitToWindowCommand.Execute(new Size(availableWidth, availableHeight));
                }
                else
                {
                    // Currently fitted, switch to 100%
                    ViewModel.ActualSizeCommand.Execute(null);
                }
                e.Handled = true;
            }
        }

        private void Close_Click(object sender, RoutedEventArgs e)
        {
            Close();
        }

        private void HoverArea_MouseEnter(object sender, System.Windows.Input.MouseEventArgs e)
        {
            // Show metadata overlay when cursor enters bottom-left area
            if (MetadataOverlay != null)
            {
                MetadataOverlay.Visibility = Visibility.Visible;
            }
        }

        private void HoverArea_MouseLeave(object sender, System.Windows.Input.MouseEventArgs e)
        {
            // Hide metadata overlay when cursor leaves bottom-left area
            if (MetadataOverlay != null)
            {
                MetadataOverlay.Visibility = Visibility.Collapsed;
            }
        }
    }
}
