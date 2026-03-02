using System;
using System.Globalization;
using System.Windows.Data;

namespace DocBrake.Converters
{
    /// <summary>
    /// Converts file size in bytes to human-readable string format
    /// </summary>
    public class FileSizeConverter : IValueConverter
    {
        private static readonly string[] SizeSuffixes = { "bytes", "KB", "MB", "GB", "TB", "PB", "EB" };

        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (value is long size)
            {
                return FormatFileSize(size);
            }
            
            if (value is int intSize)
            {
                return FormatFileSize(intSize);
            }
            
            if (value is double doubleSize)
            {
                return FormatFileSize((long)doubleSize);
            }
            
            return "0 bytes";
        }

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        {
            throw new NotImplementedException("FileSizeConverter does not support ConvertBack");
        }

        private static string FormatFileSize(long size)
        {
            if (size == 0)
                return "0 bytes";

            int suffixIndex = 0;
            double fileSize = size;

            while (fileSize >= 1024 && suffixIndex < SizeSuffixes.Length - 1)
            {
                fileSize /= 1024;
                suffixIndex++;
            }

            return $"{fileSize:F1} {SizeSuffixes[suffixIndex]}";
        }
    }
}
