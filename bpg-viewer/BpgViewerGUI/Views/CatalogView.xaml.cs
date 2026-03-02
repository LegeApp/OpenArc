using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using BpgViewer.Models;
using BpgViewer.ViewModels;

namespace BpgViewer.Views
{
    /// <summary>
    /// Catalog view for browsing BPG thumbnails
    /// </summary>
    public partial class CatalogView : UserControl
    {
        private CatalogViewModel? ViewModel => DataContext as CatalogViewModel;

        public CatalogView()
        {
            InitializeComponent();

            // Track window size changes for responsive layout
            SizeChanged += (s, e) =>
            {
                if (ViewModel != null && e.NewSize.Width > 0)
                {
                    ViewModel.ViewportWidth = e.NewSize.Width;
                }
            };

            // Set initial focus for keyboard shortcuts
            Loaded += (s, e) => ThumbnailScroller.Focus();
        }

        /// <summary>
        /// Handle keyboard shortcuts (Ctrl+ / Ctrl-)
        /// </summary>
        private void CatalogView_KeyDown(object sender, KeyEventArgs e)
        {
            if (ViewModel == null)
                return;

            bool ctrlPressed = (Keyboard.Modifiers & ModifierKeys.Control) == ModifierKeys.Control;

            if (ctrlPressed)
            {
                if (e.Key == Key.OemPlus || e.Key == Key.Add)
                {
                    // Ctrl+ = Increase size
                    ViewModel.AdjustThumbnailSize(20);
                    e.Handled = true;
                }
                else if (e.Key == Key.OemMinus || e.Key == Key.Subtract)
                {
                    // Ctrl- = Decrease size
                    ViewModel.AdjustThumbnailSize(-20);
                    e.Handled = true;
                }
            }
        }

        /// <summary>
        /// Handle thumbnail click (single click opens image)
        /// </summary>
        private void Thumbnail_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (sender is FrameworkElement element && element.DataContext is ThumbnailItem item)
            {
                // Single click - open the image
                ViewModel?.OnThumbnailDoubleClick(item);
                e.Handled = true;
            }
        }

        private void Thumbnail_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
        {
            // Could add drag selection later
        }

        private void TreeView_SelectedItemChanged(object sender, RoutedPropertyChangedEventArgs<object> e)
        {
            if (ViewModel != null && e.NewValue is FolderItem folder)
            {
                ViewModel.SelectedFolder = folder;
            }
        }
    }
}
