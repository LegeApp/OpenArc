using System;
using System.Windows;
using DocBrake.ViewModels;

namespace DocBrake.Views
{
    public partial class ArchiveContentsWindow : Window
    {
        public ArchiveContentsWindow()
        {
            InitializeComponent();

            Closed += (_, _) =>
            {
                if (DataContext is ArchiveContentsViewModel vm)
                {
                    vm.CleanupTemp();
                }
            };
        }
    }
}
