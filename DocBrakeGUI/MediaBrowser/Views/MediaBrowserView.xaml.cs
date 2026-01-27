using System;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Data;
using System.Windows.Media;
using DocBrake.MediaBrowser.Models;
using DocBrake.MediaBrowser.ViewModels;

namespace DocBrake.MediaBrowser.Views
{
    /// <summary>
    /// Media browser view for browsing BPG thumbnails
    /// </summary>
    public partial class MediaBrowserView : UserControl
    {
        private MediaBrowserViewModel? ViewModel => DataContext as MediaBrowserViewModel;

        private Point _dragStartPoint;

        private string? _lastSortProperty;
        private ListSortDirection _lastSortDirection = ListSortDirection.Ascending;

        public MediaBrowserView()
        {
            InitializeComponent();

            AddHandler(GridViewColumnHeader.ClickEvent, new RoutedEventHandler(DetailsHeader_Click));

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

        private void DetailsHeader_Click(object sender, RoutedEventArgs e)
        {
            if (ViewModel == null)
                return;

            if (!ViewModel.IsDetailsView)
                return;

            if (e.OriginalSource is not GridViewColumnHeader header)
                return;

            if (header.Tag is not string property || string.IsNullOrWhiteSpace(property))
                return;

            var view = CollectionViewSource.GetDefaultView(ViewModel.Thumbnails);
            if (view == null)
                return;

            var direction = ListSortDirection.Ascending;
            if (string.Equals(_lastSortProperty, property, StringComparison.OrdinalIgnoreCase))
            {
                direction = _lastSortDirection == ListSortDirection.Ascending
                    ? ListSortDirection.Descending
                    : ListSortDirection.Ascending;
            }

            view.SortDescriptions.Clear();
            view.SortDescriptions.Add(new SortDescription(property, direction));
            view.Refresh();

            _lastSortProperty = property;
            _lastSortDirection = direction;
        }

        private void DetailsList_MouseDoubleClick(object sender, MouseButtonEventArgs e)
        {
            if (ViewModel == null)
                return;

            if (sender is ListView lv && lv.SelectedItem is ThumbnailItem item)
            {
                ViewModel.SelectedItem = item;
                ViewModel.OnThumbnailDoubleClick(item);
                e.Handled = true;
            }
        }

        private void ItemsList_PreviewMouseMove(object sender, MouseEventArgs e)
        {
            if (e.LeftButton != MouseButtonState.Pressed)
            {
                _dragStartPoint = e.GetPosition(this);
                return;
            }

            if (sender is not ListView lv)
                return;

            var currentPos = e.GetPosition(this);
            if (Math.Abs(currentPos.X - _dragStartPoint.X) < SystemParameters.MinimumHorizontalDragDistance &&
                Math.Abs(currentPos.Y - _dragStartPoint.Y) < SystemParameters.MinimumVerticalDragDistance)
            {
                return;
            }

            var selected = lv.SelectedItems.OfType<ThumbnailItem>().ToList();
            if (selected.Count == 0)
                return;

            var files = selected
                .Select(t => t.FilePath)
                .Where(p => !string.IsNullOrWhiteSpace(p))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .ToArray();

            if (files.Length == 0)
                return;

            var data = new DataObject(DataFormats.FileDrop, files);
            DragDrop.DoDragDrop(lv, data, DragDropEffects.Copy);
        }

        private void QueuePanel_DragOver(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                e.Effects = DragDropEffects.Copy;
            }
            else
            {
                e.Effects = DragDropEffects.None;
            }
            e.Handled = true;
        }

        private void QueuePanel_Drop(object sender, DragEventArgs e)
        {
            if (ViewModel == null)
                return;

            if (!e.Data.GetDataPresent(DataFormats.FileDrop))
                return;

            var files = (string[])e.Data.GetData(DataFormats.FileDrop);
            if (files == null || files.Length == 0)
                return;

            ViewModel.HandleDroppedFiles(files);

            foreach (var file in files)
            {
                var existing = ViewModel.Thumbnails.FirstOrDefault(t =>
                    t.FilePath.Equals(file, StringComparison.OrdinalIgnoreCase));
                if (existing != null)
                {
                    existing.IsChecked = true;
                }
            }

            ViewModel.NotifySelectionChanged();
        }

        /// <summary>
        /// Handle keyboard shortcuts (Ctrl+ / Ctrl-)
        /// </summary>
        private void MediaBrowserView_KeyDown(object sender, KeyEventArgs e)
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
                else if (e.Key == Key.A)
                {
                    // Ctrl+A = Select all
                    ViewModel.SelectAllCommand.Execute(null);
                    e.Handled = true;
                }
            }
        }

        /// <summary>
        /// Handle clicking the thumbnail image to open viewer
        /// </summary>
        private void ThumbnailImage_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (sender is FrameworkElement element && element.DataContext is ThumbnailItem item)
            {
                // Select highlight + open in viewer
                if (ViewModel != null)
                    ViewModel.SelectedItem = item;

                ViewModel?.OnThumbnailDoubleClick(item);
                e.Handled = true;
            }
        }

        /// <summary>
        /// Clicking the filename toggles queue/selection (checkbox semantics)
        /// </summary>
        private void Filename_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (sender is FrameworkElement element && element.DataContext is ThumbnailItem item)
            {
                item.IsChecked = !item.IsChecked;
                ViewModel?.NotifySelectionChanged();
                e.Handled = true;
            }
        }

        private void Checkbox_Click(object sender, RoutedEventArgs e)
        {
            // Notify ViewModel that selection changed
            ViewModel?.NotifySelectionChanged();
        }

        private void TreeView_SelectedItemChanged(object sender, RoutedPropertyChangedEventArgs<object> e)
        {
            if (ViewModel != null && e.NewValue is FolderItem folder)
            {
                ViewModel.SelectedFolder = folder;
            }
        }

        private void FolderIcon_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (sender is FrameworkElement element && element.DataContext is FolderItem folder)
            {
                // Toggle tri-state: null/false -> true, true -> false
                folder.IsChecked = folder.IsChecked == true ? (bool?)false : true;
                e.Handled = true;
            }
        }

        private void DirectoryName_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            // Swap to textbox for manual path entry
            DirectoryNameText.Visibility = Visibility.Collapsed;
            DirectoryPathTextBox.Visibility = Visibility.Visible;
            DirectoryPathTextBox.Focus();
            DirectoryPathTextBox.SelectAll();
            e.Handled = true;
        }

        private void CommitDirectoryPathEdit(bool load)
        {
            var newPath = DirectoryPathTextBox.Text?.Trim() ?? string.Empty;

            DirectoryPathTextBox.Visibility = Visibility.Collapsed;
            DirectoryNameText.Visibility = Visibility.Visible;

            if (!load || ViewModel == null)
                return;

            if (string.IsNullOrWhiteSpace(newPath))
                return;

            // Let ViewModel handle not-found messaging.
            _ = ViewModel.LoadDirectoryAsync(newPath);
        }

        private void DirectoryPathTextBox_KeyDown(object sender, KeyEventArgs e)
        {
            if (e.Key == Key.Enter)
            {
                CommitDirectoryPathEdit(load: true);
                e.Handled = true;
                return;
            }

            if (e.Key == Key.Escape)
            {
                CommitDirectoryPathEdit(load: false);
                e.Handled = true;
            }
        }

        private void DirectoryPathTextBox_LostFocus(object sender, RoutedEventArgs e)
        {
            CommitDirectoryPathEdit(load: true);
        }

        private void ItemsList_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            if (ViewModel == null)
                return;

            if (sender is not ListView lv)
                return;

            // Keep ViewModel.SelectedItem aligned for "open" behavior, without destroying multi-select.
            if (lv.SelectedItem is ThumbnailItem item)
            {
                ViewModel.SelectedItem = item;
            }

            // Keep IsSelected highlights in sync.
            foreach (var t in ViewModel.Thumbnails)
                t.IsSelected = false;

            foreach (var selected in lv.SelectedItems.OfType<ThumbnailItem>())
                selected.IsSelected = true;
        }

        private void ItemsList_KeyDown(object sender, KeyEventArgs e)
        {
            if (ViewModel == null)
                return;

            if (sender is not ListView lv)
                return;

            // Space toggles checkbox state for all selected items.
            if (e.Key == Key.Space)
            {
                var selected = lv.SelectedItems.OfType<ThumbnailItem>().ToList();
                if (selected.Count == 0)
                    return;

                bool anyUnchecked = selected.Any(x => !x.IsChecked);
                foreach (var item in selected)
                    item.IsChecked = anyUnchecked;

                ViewModel.NotifySelectionChanged();
                e.Handled = true;
                return;
            }

            // Ctrl+A convenience (ListView handles select-all, but we keep parity with existing shortcuts)
            bool ctrlPressed = (Keyboard.Modifiers & ModifierKeys.Control) == ModifierKeys.Control;
            if (ctrlPressed && e.Key == Key.A)
            {
                lv.SelectAll();
                e.Handled = true;
            }
        }

        private void ItemsList_PreviewMouseRightButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (sender is not ListView lv)
                return;

            var dep = e.OriginalSource as DependencyObject;
            while (dep != null && dep is not ListViewItem)
            {
                dep = VisualTreeHelper.GetParent(dep);
            }

            if (dep is ListViewItem lvi)
            {
                if (!lvi.IsSelected)
                {
                    lv.SelectedItems.Clear();
                    lvi.IsSelected = true;
                }
            }
        }


        private void ContextMenu_AddToQueue_Click(object sender, RoutedEventArgs e)
        {
            if (ViewModel == null)
                return;

            if (sender is not MenuItem mi)
                return;

            if (mi.Parent is not ContextMenu cm)
                return;

            if (cm.PlacementTarget is not ListView lv)
                return;

            var items = lv.SelectedItems.OfType<ThumbnailItem>().ToList();
            if (items.Count == 0)
                return;

            // Check the items instead of using RequestAddToQueue
            foreach (var item in items)
            {
                item.IsChecked = true;
            }
            ViewModel.NotifySelectionChanged();
        }

        private void UserControl_Drop(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                string[] files = (string[])e.Data.GetData(DataFormats.FileDrop);
                ViewModel?.HandleDroppedFiles(files);
            }
        }

        private void UserControl_DragOver(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                e.Effects = DragDropEffects.Copy;
            }
            else
            {
                e.Effects = DragDropEffects.None;
            }
            e.Handled = true;
        }
    }
}
