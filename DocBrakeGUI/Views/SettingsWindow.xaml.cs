using System;
using System.Windows;
using DocBrake.ViewModels;

namespace DocBrake.Views
{
    /// <summary>
    /// Interaction logic for SettingsWindow.xaml
    /// </summary>
    public partial class SettingsWindow : Window
    {
        public SettingsWindow()
        {
            InitializeComponent();
        }

        private void SettingsWindow_Loaded(object sender, RoutedEventArgs e)
        {
            var owner = Owner;

            if (owner != null)
            {
                var targetWidth = owner.ActualWidth * 0.75;
                var targetHeight = owner.ActualHeight * 0.75;

                if (!double.IsNaN(targetWidth) && targetWidth > 0)
                    Width = Math.Max(MinWidth, targetWidth);

                if (!double.IsNaN(targetHeight) && targetHeight > 0)
                    Height = Math.Max(MinHeight, targetHeight);

                Left = owner.Left + (owner.ActualWidth - Width) / 2.0;
                Top = owner.Top + (owner.ActualHeight - Height) / 2.0;
            }
            else
            {
                var area = SystemParameters.WorkArea;
                Width = Math.Max(MinWidth, area.Width * 0.75);
                Height = Math.Max(MinHeight, area.Height * 0.75);

                Left = area.Left + (area.Width - Width) / 2.0;
                Top = area.Top + (area.Height - Height) / 2.0;
            }
        }

        public SettingsWindow(SettingsViewModel viewModel) : this()
        {
            DataContext = viewModel;
        }

        private void OkButton_Click(object sender, RoutedEventArgs e)
        {
            DialogResult = true;
            Close();
        }

        private void CancelButton_Click(object sender, RoutedEventArgs e)
        {
            DialogResult = false;
            Close();
        }
    }
}
