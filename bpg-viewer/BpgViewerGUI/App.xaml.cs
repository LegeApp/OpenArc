using System.Windows;

namespace BpgViewer
{
    /// <summary>
    /// Application entry point
    /// Matches OpenARC's DocBrakeGUI structure
    /// </summary>
    public partial class App : Application
    {
        protected override void OnStartup(StartupEventArgs e)
        {
            base.OnStartup(e);

            // Handle command-line arguments
            if (e.Args.Length > 0)
            {
                string filePath = e.Args[0];
                if (System.IO.File.Exists(filePath))
                {
                    // Load the file after the main window is shown
                    var mainWindow = MainWindow as MainWindow;
                    mainWindow?.Loaded += (s, args) =>
                    {
                        var viewModel = mainWindow.DataContext as ViewModels.ImageViewerViewModel;
                        viewModel?.LoadImage(filePath);
                    };
                }
            }
        }
    }
}
