using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Interop;
using DocBrake.MediaBrowser.ViewModels;

namespace DocBrake.MediaBrowser.Views
{
    public partial class MediaImageViewerWindow : Window
    {
        public MediaViewerViewModel ViewModel { get; }

        private const int WM_SIZING = 0x0214;
        private const int WMSZ_LEFT = 1;
        private const int WMSZ_RIGHT = 2;
        private const int WMSZ_TOP = 3;
        private const int WMSZ_TOPLEFT = 4;
        private const int WMSZ_TOPRIGHT = 5;
        private const int WMSZ_BOTTOM = 6;
        private const int WMSZ_BOTTOMLEFT = 7;
        private const int WMSZ_BOTTOMRIGHT = 8;

        [StructLayout(LayoutKind.Sequential)]
        private struct RECT
        {
            public int Left;
            public int Top;
            public int Right;
            public int Bottom;
        }

        private string? _currentFilePath;
        private List<string> _folderFiles = new();
        private int _currentIndex = -1;

        private bool _isFullscreen;
        private Rect _restoreBounds;
        private WindowState _restoreWindowState;

        public MediaImageViewerWindow()
        {
            InitializeComponent();

            ViewModel = new MediaViewerViewModel();
            DataContext = ViewModel;

            KeyDown += OnKeyDown;

            SourceInitialized += (_, _) =>
            {
                var source = PresentationSource.FromVisual(this) as HwndSource;
                source?.AddHook(WndProc);
            };

			SizeChanged += (_, _) =>
			{
				if (_isFullscreen)
					return;

				if (ViewModel.IsVideo)
					return;

				if (!ViewModel.IsFitToWindow)
					return;

				FitToWindow();
			};
        }

        private IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            if (msg != WM_SIZING)
                return IntPtr.Zero;

            if (_isFullscreen)
                return IntPtr.Zero;

            if (ViewModel.IsVideo)
                return IntPtr.Zero;

            if (ViewModel.CurrentImage == null)
                return IntPtr.Zero;

            double imageW = ViewModel.CurrentImage.Width;
            double imageH = ViewModel.CurrentImage.Height;
            if (imageW <= 0 || imageH <= 0)
                return IntPtr.Zero;

            double aspect = imageW / imageH;
            if (aspect <= 0)
                return IntPtr.Zero;

            var rc = Marshal.PtrToStructure<RECT>(lParam);
            int width = Math.Max(1, rc.Right - rc.Left);
            int height = Math.Max(1, rc.Bottom - rc.Top);

            int edge = wParam.ToInt32();

            bool sizingFromLeft = edge is WMSZ_LEFT or WMSZ_TOPLEFT or WMSZ_BOTTOMLEFT;
            bool sizingFromTop = edge is WMSZ_TOP or WMSZ_TOPLEFT or WMSZ_TOPRIGHT;
            bool sizingFromCorner = edge is WMSZ_TOPLEFT or WMSZ_TOPRIGHT or WMSZ_BOTTOMLEFT or WMSZ_BOTTOMRIGHT;

            // If user is dragging a vertical edge, derive width from height.
            // If dragging a horizontal edge, derive height from width.
            // If dragging a corner, keep whichever axis changed most.
            int newWidth = width;
            int newHeight = height;

            if (edge is WMSZ_LEFT or WMSZ_RIGHT)
            {
                newHeight = Math.Max(1, (int)Math.Round(newWidth / aspect));
            }
            else if (edge is WMSZ_TOP or WMSZ_BOTTOM)
            {
                newWidth = Math.Max(1, (int)Math.Round(newHeight * aspect));
            }
            else if (sizingFromCorner)
            {
                double heightFromWidth = newWidth / aspect;
                double widthFromHeight = newHeight * aspect;

                double heightDelta = Math.Abs(newHeight - heightFromWidth);
                double widthDelta = Math.Abs(newWidth - widthFromHeight);

                if (widthDelta < heightDelta)
                {
                    newWidth = Math.Max(1, (int)Math.Round(widthFromHeight));
                }
                else
                {
                    newHeight = Math.Max(1, (int)Math.Round(heightFromWidth));
                }
            }

            // Apply clamped dimensions back to the sizing RECT.
            if (sizingFromLeft)
                rc.Left = rc.Right - newWidth;
            else
                rc.Right = rc.Left + newWidth;

            if (sizingFromTop)
                rc.Top = rc.Bottom - newHeight;
            else
                rc.Bottom = rc.Top + newHeight;

            Marshal.StructureToPtr(rc, lParam, false);
            handled = true;
            return IntPtr.Zero;
        }

        public MediaImageViewerWindow(string filePath) : this()
        {
            _currentFilePath = filePath;
            InitializeFolderNavigation(filePath);

            Loaded += (s, e) =>
            {
                try
                {
                    LoadAndPresent(filePath);

                    Dispatcher.InvokeAsync(() =>
                    {
                        PresentCurrentMedia();
                    }, System.Windows.Threading.DispatcherPriority.Loaded);
                }
                catch (Exception ex)
                {
                    MessageBox.Show($"Error loading image: {ex.Message}", "Error", MessageBoxButton.OK, MessageBoxImage.Error);
                }
            };
        }

        private void InitializeFolderNavigation(string filePath)
        {
            try
            {
                string? dir = Path.GetDirectoryName(filePath);
                if (string.IsNullOrEmpty(dir) || !Directory.Exists(dir))
                {
                    _folderFiles = new List<string>();
                    _currentIndex = -1;
                    return;
                }

                var extensions = new[]
                {
                    // Common images
                    "*.jpg", "*.jpeg", "*.png", "*.bmp", "*.gif", "*.tiff", "*.tif", "*.webp", "*.bpg", "*.ico",
                    // HEIC/HEIF
                    "*.heic", "*.heif",
                    // RAW / DNG
                    "*.dng", "*.cr2", "*.nef", "*.arw", "*.orf", "*.rw2", "*.raf",
                    // JPEG2000
                    "*.jp2", "*.j2k", "*.j2c", "*.jpc", "*.jpt", "*.jph", "*.jhc",
                    // Common videos
                    "*.mp4", "*.mov", "*.m4v", "*.avi", "*.mkv", "*.wmv"
                };
                _folderFiles = extensions
                    .SelectMany(ext => Directory.EnumerateFiles(dir, ext, SearchOption.TopDirectoryOnly))
                    .OrderBy(p => p, StringComparer.OrdinalIgnoreCase)
                    .ToList();

                _currentIndex = _folderFiles.FindIndex(p => string.Equals(p, filePath, StringComparison.OrdinalIgnoreCase));
            }
            catch
            {
                _folderFiles = new List<string>();
                _currentIndex = -1;
            }
        }

        private void LoadAndPresent(string filePath)
        {
            _currentFilePath = filePath;
            ViewModel.LoadImage(filePath);

            if (_folderFiles.Count == 0)
            {
                InitializeFolderNavigation(filePath);
            }
            else
            {
                int idx = _folderFiles.FindIndex(p => string.Equals(p, filePath, StringComparison.OrdinalIgnoreCase));
                if (idx >= 0)
                    _currentIndex = idx;
            }
        }

        private void PresentCurrentMedia()
        {
            if (_isFullscreen)
            {
                FitToWindow();
                return;
            }

            Rect workArea = SystemParameters.WorkArea;

            // Standard default window size based on usable desktop area.
            // Clamp to avoid being too small (Min*) or too large (90% of screen).
            const double defaultFraction = 0.35;
            double maxW = Math.Max(MinWidth, workArea.Width * 0.90);
            double maxH = Math.Max(MinHeight, workArea.Height * 0.90);
            double targetW = Math.Max(MinWidth, Math.Min(workArea.Width * defaultFraction, maxW));
            double targetH = Math.Max(MinHeight, Math.Min(workArea.Height * defaultFraction, maxH));

            Width = targetW;
            Height = targetH;

            // Fit images to the chosen default window size. Videos use MediaElement Stretch=Uniform.
            FitToWindow();
        }

        private void FitToWindow()
        {
            if (ViewModel.IsVideo)
                return;

            double availableHeight = Math.Max(0, ActualHeight - 12);
            double availableWidth = Math.Max(0, ActualWidth - 12);
            ViewModel.FitToWindowCommand.Execute((Size?)new Size(availableWidth, availableHeight));
        }

        private void ToggleFullscreen()
        {
            if (!_isFullscreen)
            {
                _restoreWindowState = WindowState;
                _restoreBounds = new Rect(Left, Top, Width, Height);

                _isFullscreen = true;
                WindowState = WindowState.Normal;
                WindowState = WindowState.Maximized;
                FitToWindow();
            }
            else
            {
                _isFullscreen = false;

                WindowState = WindowState.Normal;
                Left = _restoreBounds.Left;
                Top = _restoreBounds.Top;
                Width = _restoreBounds.Width;
                Height = _restoreBounds.Height;
                WindowState = _restoreWindowState;
                PresentCurrentMedia();
            }
        }

        private void NavigateRelative(int delta)
        {
            if (_folderFiles.Count == 0)
                return;

            if (_currentIndex < 0)
                _currentIndex = 0;

            int nextIndex = (_currentIndex + delta) % _folderFiles.Count;
            if (nextIndex < 0)
                nextIndex += _folderFiles.Count;

            _currentIndex = nextIndex;
            string nextPath = _folderFiles[_currentIndex];
            LoadAndPresent(nextPath);
            PresentCurrentMedia();
        }

        private void OnKeyDown(object sender, KeyEventArgs e)
        {
            if (e.Key == Key.Escape)
            {
                Close();
                e.Handled = true;
                return;
            }

            if (e.Key == Key.F)
            {
                ToggleFullscreen();
                e.Handled = true;
                return;
            }

            if (e.Key == Key.Left || e.Key == Key.Up)
            {
                NavigateRelative(-1);
                e.Handled = true;
                return;
            }

            if (e.Key == Key.Right || e.Key == Key.Down)
            {
                NavigateRelative(1);
                e.Handled = true;
                return;
            }

            if (e.Key == Key.D0 || e.Key == Key.NumPad0)
            {
                FitToWindow();
                e.Handled = true;
                return;
            }

            if (e.Key == Key.D1 || e.Key == Key.NumPad1)
            {
                ViewModel.ActualSizeCommand.Execute(null);
                e.Handled = true;
            }
        }

        private void Window_PreviewMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ButtonState != MouseButtonState.Pressed)
                return;

            Point p = e.GetPosition(this);
            double border = 8;
            bool onResizeBorder = p.X <= border || p.Y <= border || p.X >= ActualWidth - border || p.Y >= ActualHeight - border;

            if (onResizeBorder)
                return;

            try
            {
                DragMove();
                e.Handled = true;
            }
            catch
            {
            }
        }

        private void Window_PreviewMouseRightButtonDown(object sender, MouseButtonEventArgs e)
        {
            Close();
            e.Handled = true;
        }
    }
}
