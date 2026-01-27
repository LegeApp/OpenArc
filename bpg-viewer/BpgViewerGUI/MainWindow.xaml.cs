using System;
using System.Windows;
using Microsoft.Win32;
using BpgViewer.ViewModels;

namespace BpgViewer
{
    /// <summary>
    /// Main window - BPG thumbnail catalog browser
    /// </summary>
    public partial class MainWindow : Window
    {
        private CatalogViewModel ViewModel => (CatalogViewModel)DataContext;

        public MainWindow()
        {
            InitializeComponent();

            // Subscribe to image opening from catalog
            ViewModel.ImageSelected += OnImageSelected;
        }

        private void OnImageSelected(string filePath)
        {
            // Open image in new window
            var imageWindow = new ImageViewerWindow(filePath);
            imageWindow.Show();
        }

        private void MenuOpenImage_Click(object sender, RoutedEventArgs e)
        {
            var dialog = new OpenFileDialog
            {
                Filter = "BPG Images|*.bpg|All Files|*.*",
                Title = "Open BPG Image"
            };

            if (dialog.ShowDialog() == true)
            {
                // Open single image in new window
                var imageWindow = new ImageViewerWindow(dialog.FileName);
                imageWindow.Show();
            }
        }

        private async void MenuOpenFolder_Click(object sender, RoutedEventArgs e)
        {
            var dialog = new System.Windows.Forms.FolderBrowserDialog
            {
                Description = "Select folder containing BPG images"
            };

            if (dialog.ShowDialog() == System.Windows.Forms.DialogResult.OK)
            {
                await ViewModel.LoadDirectoryAsync(dialog.SelectedPath);
            }
        }

        private void MenuExit_Click(object sender, RoutedEventArgs e)
        {
            Application.Current.Shutdown();
        }

        private void MenuShortcuts_Click(object sender, RoutedEventArgs e)
        {
            MessageBox.Show(
                "Keyboard Shortcuts:\n\n" +
                "Ctrl+O - Open single image\n" +
                "Ctrl+Shift+O - Open folder\n" +
                "Ctrl+ - Increase thumbnail size\n" +
                "Ctrl- - Decrease thumbnail size\n" +
                "F5 - Refresh current folder\n" +
                "Single-click - Open image in new viewer window\n\n" +
                "In Image Viewer:\n" +
                "1 - Actual size (100%)\n" +
                "0 - Fit to window\n" +
                "+ - Zoom in\n" +
                "- - Zoom out\n" +
                "Esc - Close viewer",
                "Keyboard Shortcuts",
                MessageBoxButton.OK,
                MessageBoxImage.Information);
        }

        private void MenuAbout_Click(object sender, RoutedEventArgs e)
        {
            MessageBox.Show(
                "BPG Image Browser\n\n" +
                "Version 0.1.0\n\n" +
                "A fast and efficient browser for BPG images.\n" +
                "Built with WPF and Rust.",
                "About BPG Viewer",
                MessageBoxButton.OK,
                MessageBoxImage.Information);
        }

        protected override void OnClosed(EventArgs e)
        {
            base.OnClosed(e);
            ViewModel.Dispose();
        }
    }
}
