using System;
using System.Collections.Concurrent;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using DocBrake.MediaBrowser.Models;

namespace DocBrake.MediaBrowser.Services
{
    public class FileThumbnailService
    {
        private readonly ConcurrentDictionary<string, BitmapSource> _badgeCache = new(StringComparer.OrdinalIgnoreCase);

        public bool TryLoadThumbnail(ThumbnailItem item, int size)
        {
            if (item == null) return false;

            item.IsLoading = true;
            item.HasError = false;

            try
            {
                var icon = TryGetShellThumbnail(item.FilePath, size);
                if (icon != null)
                {
                    item.ThumbnailImage = icon;
                    item.IsLoading = false;
                    return true;
                }

                var ext = item.FileExtension;
                if (string.IsNullOrWhiteSpace(ext))
                    ext = "FILE";

                var badgeKey = $"{ext}:{size}";
                item.ThumbnailImage = _badgeCache.GetOrAdd(badgeKey, _ => CreateExtensionBadge(ext, size));
                item.IsLoading = false;
                return true;
            }
            catch (Exception ex)
            {
                item.HasError = true;
                item.ErrorMessage = ex.Message;
                item.IsLoading = false;
                return false;
            }
        }

        private static BitmapSource? TryGetShellThumbnail(string filePath, int size)
        {
            try
            {
                var iid = typeof(IShellItemImageFactory).GUID;
                int hr = SHCreateItemFromParsingName(filePath, IntPtr.Zero, ref iid, out var factory);
                if (hr != 0 || factory == null)
                    return null;

                var flags = SIIGBF.SIIGBF_BIGGERSIZEOK;
                var sz = new SIZE(size, size);
                hr = factory.GetImage(sz, flags, out var hBitmap);
                if (hr != 0 || hBitmap == IntPtr.Zero)
                    return null;

                try
                {
                    var bmp = Imaging.CreateBitmapSourceFromHBitmap(
                        hBitmap,
                        IntPtr.Zero,
                        Int32Rect.Empty,
                        BitmapSizeOptions.FromWidthAndHeight(size, size));
                    bmp.Freeze();
                    return bmp;
                }
                finally
                {
                    DeleteObject(hBitmap);
                }
            }
            catch
            {
                return null;
            }
        }

        private static BitmapSource CreateExtensionBadge(string extension, int size)
        {
            var clean = extension.Trim().TrimStart('.').ToUpperInvariant();
            if (clean.Length == 0)
                clean = "FILE";
            if (clean.Length > 6)
                clean = clean.Substring(0, 6);

            var (accent, textBrush) = GetStableAccentBrushes(clean);

            var visual = new DrawingVisual();
            using (var dc = visual.RenderOpen())
            {
                var border = new Pen(new SolidColorBrush(Color.FromRgb(0xD0, 0xD0, 0xD0)), 2);
                border.Freeze();

                dc.DrawRectangle(Brushes.White, border, new Rect(0, 0, size, size));

                var bandHeight = Math.Max(20, size * 0.32);
                var bandRect = new Rect(0, size - bandHeight, size, bandHeight);
                dc.DrawRectangle(accent, null, bandRect);

                var typeface = new Typeface(new FontFamily("Segoe UI"), FontStyles.Normal, FontWeights.SemiBold, FontStretches.Normal);
                var fontSize = Math.Max(10, bandHeight * 0.42);

                var ft = new FormattedText(
                    clean,
                    CultureInfo.InvariantCulture,
                    FlowDirection.LeftToRight,
                    typeface,
                    fontSize,
                    textBrush,
                    1.0);

                ft.MaxTextWidth = size - 4; // Add padding
                ft.TextAlignment = TextAlignment.Center;

                var x = 2.0; // Add padding
                var y = bandRect.Y + (bandRect.Height - ft.Height) / 2.0 - 1; // Adjust vertical position

                var outline = new Pen(new SolidColorBrush(Color.FromArgb(0xB0, 0x00, 0x00, 0x00)), Math.Max(1.5, size * 0.028));
                outline.Freeze();

                var geo = ft.BuildGeometry(new Point(x, y));
                dc.DrawGeometry(null, outline, geo);
                dc.DrawText(ft, new Point(x, y));
            }

            var bmp = new RenderTargetBitmap(size, size, 96, 96, PixelFormats.Pbgra32);
            bmp.Render(visual);
            bmp.Freeze();
            return bmp;
        }

        private static (Brush accent, Brush text) GetStableAccentBrushes(string key)
        {
            uint h = 2166136261;
            for (int i = 0; i < key.Length; i++)
            {
                h ^= key[i];
                h *= 16777619;
            }

            byte r = (byte)(60 + (h % 160));
            byte g = (byte)(60 + ((h >> 8) % 160));
            byte b = (byte)(60 + ((h >> 16) % 160));

            var accent = new SolidColorBrush(Color.FromRgb(r, g, b));
            accent.Freeze();

            // Choose black/white for contrast against the accent background.
            var luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0;
            Brush text = luminance > 0.62 ? Brushes.Black : Brushes.White;
            return (accent, text);
        }

        [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = true)]
        private static extern int SHCreateItemFromParsingName(
            [MarshalAs(UnmanagedType.LPWStr)] string pszPath,
            IntPtr pbc,
            ref Guid riid,
            [MarshalAs(UnmanagedType.Interface)] out IShellItemImageFactory ppv);

        [DllImport("gdi32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool DeleteObject(IntPtr hObject);

        [ComImport]
        [Guid("bcc18b79-ba16-442f-80c4-8a59c30c463b")]
        [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IShellItemImageFactory
        {
            int GetImage(SIZE size, SIIGBF flags, out IntPtr phbm);
        }

        [StructLayout(LayoutKind.Sequential)]
        private readonly struct SIZE
        {
            public readonly int cx;
            public readonly int cy;

            public SIZE(int cx, int cy)
            {
                this.cx = cx;
                this.cy = cy;
            }
        }

        [Flags]
        private enum SIIGBF
        {
            SIIGBF_RESIZETOFIT = 0x00,
            SIIGBF_BIGGERSIZEOK = 0x01,
            SIIGBF_MEMORYONLY = 0x02,
            SIIGBF_ICONONLY = 0x04,
            SIIGBF_THUMBNAILONLY = 0x08,
            SIIGBF_INCACHEONLY = 0x10
        }
    }
}
