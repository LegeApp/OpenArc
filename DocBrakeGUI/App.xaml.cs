using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using System;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
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
        private static void DebugLog(string message)
        {
            try
            {
                var logPath = System.IO.Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "debug.log");
                var timestamp = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss.fff");
                System.IO.File.AppendAllText(logPath, $"[{timestamp}] [App] {message}\n");
            }
            catch
            {
                // Ignore logging errors
            }
        }

        private IHost? _host;

        private static int _isHandlingDispatcherException;
        private static int _isHandlingDomainException;
        private static int _hasShownCriticalError;

        // Track missing DLLs for status display
        public static string? MissingDllMessage { get; private set; }

        public IHost? Host => _host;
        
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool AllocConsole();

        public App()
        {
            DebugLog("App constructor starting");
            
#if SHOW_CONSOLE
            // Show console window in Debug mode for diagnostic logging
            AllocConsole();
#endif
            // Immediate debug output
            Console.WriteLine("DocBrake App constructor called");
            
            try
            {
                var logPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "app_constructor.log");
                File.WriteAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] App constructor called\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Base Directory: {AppDomain.CurrentDomain.BaseDirectory}\n");
                DebugLog("App constructor log files created");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Error in App constructor: {ex.Message}");
                DebugLog($"App constructor error: {ex.Message}");
            }
            
            // Add global exception handlers
            AppDomain.CurrentDomain.UnhandledException += OnUnhandledException;
            DispatcherUnhandledException += OnDispatcherUnhandledException;
            
            DebugLog("App constructor completed");
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
                
                // Check for required DLLs
                var missingDlls = new System.Collections.Generic.List<string>();

                var openArcDllPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "openarc_ffi.dll");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Checking for OpenArc FFI DLL at: {openArcDllPath}\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] DLL exists: {File.Exists(openArcDllPath)}\n");
                if (!File.Exists(openArcDllPath))
                {
                    File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] WARNING: openarc_ffi.dll not found - archiving functionality will not work\n");
                    missingDlls.Add("openarc_ffi.dll (archiving disabled)");
                }

                var bpgViewerDllPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "bpg_viewer.dll");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Checking for BPG Viewer DLL at: {bpgViewerDllPath}\n");
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] DLL exists: {File.Exists(bpgViewerDllPath)}\n");
                if (!File.Exists(bpgViewerDllPath))
                {
                    File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] WARNING: bpg_viewer.dll not found - thumbnails will not work\n");
                    missingDlls.Add("bpg_viewer.dll (thumbnails disabled)");
                }

                if (missingDlls.Count > 0)
                {
                    MissingDllMessage = "⚠️ Missing DLLs: " + string.Join(", ", missingDlls);
                    File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] {MissingDllMessage}\n");
                }
                
                // Test GPU initialization at startup
                File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] Testing GPU initialization...\n");
                DebugLog("Starting GPU initialization test");
                
                try
                {
                    DebugLog("Creating NativeGpuService instance");
                    var gpuService = DocBrake.Services.NativeGpuService.Instance;
                    DebugLog("NativeGpuService instance created");
                    
                    var gpuAvailable = gpuService.HasGpu;
                    var gpuBackend = gpuService.ActiveBackendName;
                    var gpuDevice = gpuService.DeviceName;
                    
                    var gpuMsg = $"GPU Status: Available={gpuAvailable}, Backend={gpuBackend}, Device={gpuDevice}";
                    File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] {gpuMsg}\n");
                    DebugLog(gpuMsg);
                    
                    // Also output to console for immediate visibility
                    Console.WriteLine($"=== GPU INITIALIZATION AT STARTUP ===");
                    Console.WriteLine($"GPU Available: {gpuAvailable}");
                    Console.WriteLine($"Backend: {gpuBackend}");
                    Console.WriteLine($"Device: {gpuDevice}");
                    Console.WriteLine($"========================================");
                    Console.Out.Flush();
                    
                    // Test GPU pipeline initialization
                    DebugLog("Calling gpu_thumbnail_pipeline_init");
                    int gpuResult = DocBrake.MediaBrowser.NativeInterop.BpgViewerFFI.gpu_thumbnail_pipeline_init();
                    var pipelineResult = gpuResult == 0 ? "SUCCESS" : $"FAILED (code: {gpuResult})";
                    File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] GPU Pipeline Init: {pipelineResult}\n");
                    DebugLog($"GPU pipeline result: {pipelineResult}");
                    
                    Console.WriteLine($"GPU Pipeline: {pipelineResult}");
                    Console.WriteLine($"========================================");
                    Console.Out.Flush();
                }
                catch (Exception ex)
                {
                    var errorMsg = $"GPU initialization failed: {ex.Message}";
                    File.AppendAllText(logPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] ERROR: {errorMsg}\n");
                    DebugLog($"GPU initialization error: {errorMsg}");
                    DebugLog($"GPU exception stack trace: {ex.StackTrace}");
                    Console.WriteLine($"GPU ERROR: {errorMsg}");
                    Console.Out.Flush();
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
                        services.AddSingleton<IStagingService, StagingService>();
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
                        services.AddSingleton<IStagingService, StagingService>();
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
