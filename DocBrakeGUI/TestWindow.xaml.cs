using System;
using System.IO;
using System.Windows;

namespace DocBrake
{
    public partial class TestWindow : Window
    {
        public TestWindow()
        {
            var logPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "test_window.log");
            
            try
            {
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] TestWindow constructor started\n");
                
                InitializeComponent();
                
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] TestWindow InitializeComponent completed\n");
            }
            catch (Exception ex)
            {
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] ERROR in TestWindow constructor: {ex}\n");
                throw;
            }
        }
    }
}
