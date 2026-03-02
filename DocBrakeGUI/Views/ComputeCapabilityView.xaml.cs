using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using DocBrake.ViewModels;

namespace DocBrake.Views
{
    public partial class ComputeCapabilityView : UserControl
    {
        public ComputeCapabilityView()
        {
            InitializeComponent();
        }

        private void Border_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (DataContext is ComputeCapabilityViewModel viewModel)
            {
                viewModel.ShowAllOptions = !viewModel.ShowAllOptions;
            }
        }
    }
}
