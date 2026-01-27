using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using System;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Media;
using DocBrake.Services;
using DocBrake.ViewModels;
using DocBrake.Views;
using DocBrake.MediaBrowser.Services;
using DocBrake.MediaBrowser.ViewModels;
using DocBrake.MediaBrowser.Views;

namespace DocBrake
{
    public partial class App : Application
    {
        private IHost? _host;

        private static int _isHandlingDispatcherException;
        private static int _isHandlingDomainException;
        private static int _hasShownCriticalError;

        public IHost? Host => _host;

        public App()
        {
            // Immediate debug output
            Console.WriteLine("DocBrake App constructor called");
            
            try
            {
                var logPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "app_constructor.log");
                File.WriteAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] App constructor called\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Base Directory: {AppDomain.CurrentDomain.BaseDirectory}\n");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Error in App constructor: {ex.Message}");
            }
            
            // Add global exception handlers
            AppDomain.CurrentDomain.UnhandledException += OnUnhandledException;
            DispatcherUnhandledException += OnDispatcherUnhandledException;
        }

        protected override void OnStartup(StartupEventArgs e)
        {
            var logPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "startup.log");
            
            try
            {
                // Clear previous log and start fresh
                File.WriteAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] === DocBrake GUI Startup Begin ===\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Base Directory: {AppDomain.CurrentDomain.BaseDirectory}\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Working Directory: {Environment.CurrentDirectory}\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Command Line Args: {string.Join(" ", e.Args)}\n");

                // Check for CLI mode
                if (e.Args.Length > 0)
                {
                    File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] CLI mode detected, args: {string.Join(" ", e.Args)}\n");
                    RunCliMode(e.Args).GetAwaiter().GetResult();
                    Environment.Exit(0);
                    return;
                }
                
                // Check for required OpenArc FFI DLL
                var openArcDllPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "openarc_ffi.dll");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Checking for OpenArc FFI DLL at: {openArcDllPath}\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] DLL exists: {File.Exists(openArcDllPath)}\n");
                if (!File.Exists(openArcDllPath))
                {
                    File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] WARNING: openarc_ffi.dll not found - archiving functionality will not work\n");
                }
                
                // Set application theme colors
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Setting theme colors...\n");
                var app = Current;
                app.Resources["AccentColor"] = new SolidColorBrush(Colors.DodgerBlue);
                app.Resources["AccentColorBrush"] = new SolidColorBrush(Colors.DodgerBlue);
                
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Theme colors initialized\n");
                
                _host = Microsoft.Extensions.Hosting.Host.CreateDefaultBuilder()
                    .ConfigureServices((context, services) =>
                    {
                        // Register services
                        services.AddSingleton<IDocumentProcessingService, OpenArcProcessingService>();
                        services.AddSingleton<ISettingsService, SettingsService>();
                        services.AddSingleton<IFileDialogService, FileDialogService>();
                        services.AddSingleton<IPhoneDetectionService, PhoneDetectionService>();
                        services.AddSingleton<ICLIInterface, CLIInterface>();

                        // Queue service (shared state)
                        services.AddSingleton<IQueueService, QueueService>();
                        services.AddTransient<QueueViewModel>();
                        
                        // MediaBrowser services
                        services.AddSingleton<ThumbnailCacheService>(sp =>
                            new ThumbnailCacheService(thumbnailWidth: 256, thumbnailHeight: 256, maxConcurrency: 12));
                        services.AddSingleton<MediaBrowserViewModel>(sp =>
                            new MediaBrowserViewModel(
                                sp.GetRequiredService<ThumbnailCacheService>(),
                                sp.GetRequiredService<IQueueService>()));
                        services.AddSingleton<MediaViewerViewModel>();
                        
                        // Register ViewModels
                        services.AddTransient<MainViewModel>();
                        services.AddTransient<SettingsViewModel>();
                        
                        // Register Views
                        services.AddTransient<MainWindow>();
                        services.AddTransient<SettingsWindow>();
                    })
                    .ConfigureLogging(logging =>
                    {
                        logging.AddConsole();
                        logging.SetMinimumLevel(LogLevel.Information);
                    })
                    .Build();
                
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Dependency injection container built\n");

                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Creating MainWindow...\n");
                var mainWindow = _host.Services.GetRequiredService<MainWindow>();
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] MainWindow created successfully\n");
                
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Setting MainWindow properties...\n");
                mainWindow.WindowStartupLocation = System.Windows.WindowStartupLocation.CenterScreen;
                
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Calling mainWindow.Show()...\n");
                mainWindow.Show();
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] MainWindow.Show() completed\n");
                
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] MainWindow visibility: {mainWindow.Visibility}\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] MainWindow state: {mainWindow.WindowState}\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] MainWindow IsVisible: {mainWindow.IsVisible}\n");

                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Calling base.OnStartup()...\n");
                base.OnStartup(e);
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] base.OnStartup() completed\n");
            }
            catch (Exception ex)
            {
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] FATAL ERROR in OnStartup: {ex}\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Stack Trace: {ex.StackTrace}\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Inner Exception: {ex.InnerException}\n");
                
                MessageBox.Show($"Failed to start application: {ex.Message}\n\nCheck startup.log for details.", "Startup Error", MessageBoxButton.OK, MessageBoxImage.Error);
                Environment.Exit(1);
            }
            finally
            {
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] === OnStartup method completed ===\n");
            }
        }

        private void OnUnhandledException(object sender, UnhandledExceptionEventArgs e)
        {
            var logPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "startup.log");
            try
            {
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] UNHANDLED EXCEPTION: {e.ExceptionObject}\n");
            }
            catch
            {
            }

            if (Interlocked.Exchange(ref _isHandlingDomainException, 1) == 1)
                return;

            try
            {
                if (e.ExceptionObject is Exception ex)
                {
                    try
                    {
                        File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] UNHANDLED EXCEPTION DETAILS: {ex}\n");
                    }
                    catch
                    {
                    }

                    if (Interlocked.Exchange(ref _hasShownCriticalError, 1) == 0)
                    {
                        MessageBox.Show(
                            $"A critical error occurred:\n\n{ex.Message}\n\nSee startup.log for details.",
                            "Critical Error",
                            MessageBoxButton.OK,
                            MessageBoxImage.Error);
                    }
                }
            }
            catch
            {
            }
            finally
            {
                Interlocked.Exchange(ref _isHandlingDomainException, 0);
            }
        }

        private void OnDispatcherUnhandledException(object sender, System.Windows.Threading.DispatcherUnhandledExceptionEventArgs e)
        {
            var logPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "startup.log");
            try
            {
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] DISPATCHER EXCEPTION: {e.Exception}\n");
            }
            catch
            {
            }

            // Prevent recursive MessageBox.Show / layout exceptions from cascading into a stack overflow.
            if (Interlocked.Exchange(ref _isHandlingDispatcherException, 1) == 1)
            {
                e.Handled = true;
                return;
            }

            try
            {
                if (Interlocked.Exchange(ref _hasShownCriticalError, 1) == 0)
                {
                    MessageBox.Show(
                        $"An unexpected UI error occurred:\n\n{e.Exception.Message}\n\nSee startup.log for details.",
                        "Error",
                        MessageBoxButton.OK,
                        MessageBoxImage.Error);
                }
            }
            catch
            {
            }
            finally
            {
                e.Handled = true;
                Interlocked.Exchange(ref _isHandlingDispatcherException, 0);
            }
        }

        protected override void OnExit(ExitEventArgs e)
        {
            _host?.Dispose();
            base.OnExit(e);
        }

        private async Task RunCliMode(string[] args)
        {
            try
            {
                var logPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "startup.log");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Building host for CLI mode...\n");

                _host = Microsoft.Extensions.Hosting.Host.CreateDefaultBuilder()
                    .ConfigureServices((context, services) =>
                    {
                        // Register services
                        services.AddSingleton<IDocumentProcessingService, OpenArcProcessingService>();
                        services.AddSingleton<ISettingsService, SettingsService>();
                        services.AddSingleton<IFileDialogService, FileDialogService>();
                        services.AddSingleton<IPhoneDetectionService, PhoneDetectionService>();
                        services.AddSingleton<ICLIInterface, CLIInterface>();

                        // Queue service (shared state)
                        services.AddSingleton<IQueueService, QueueService>();
                        services.AddTransient<QueueViewModel>();
                        
                        // MediaBrowser services
                        services.AddSingleton<ThumbnailCacheService>(sp =>
                            new ThumbnailCacheService(thumbnailWidth: 256, thumbnailHeight: 256, maxConcurrency: 12));
                        services.AddSingleton<MediaBrowserViewModel>(sp =>
                            new MediaBrowserViewModel(
                                sp.GetRequiredService<ThumbnailCacheService>(),
                                sp.GetRequiredService<IQueueService>()));
                        services.AddSingleton<MediaViewerViewModel>();
                        
                        // Register ViewModels (needed for dependencies)
                        services.AddTransient<MainViewModel>();
                        services.AddTransient<SettingsViewModel>();
                    })
                    .ConfigureLogging(logging =>
                    {
                        logging.AddConsole();
                        logging.SetMinimumLevel(LogLevel.Information);
                    })
                    .Build();

                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Host built for CLI mode\n");

                var cli = _host.Services.GetRequiredService<ICLIInterface>();
                var exitCode = await cli.ProcessCommandLineAsync(args);
                
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] CLI completed with exit code: {exitCode}\n");
                Environment.Exit(exitCode);
            }
            catch (Exception ex)
            {
                var logPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "startup.log");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] CLI MODE ERROR: {ex}\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Stack Trace: {ex.StackTrace}\n");
                
                Console.WriteLine($"CLI Error: {ex.Message}");
                Console.WriteLine($"Stack Trace: {ex.StackTrace}");
                Environment.Exit(1);
            }
        }
    }
}
