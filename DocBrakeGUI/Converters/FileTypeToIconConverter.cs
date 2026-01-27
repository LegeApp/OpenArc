using System;
using System.Globalization;
using System.Windows.Data;
using DocBrake.Models;

namespace DocBrake.Converters
{
    public class FileTypeToIconConverter : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (value is FileType fileType)
            {
                return fileType switch
                {
                    FileType.Image => "🖼️",
                    FileType.Video => "🎬",
                    FileType.Document => "📄",
                    FileType.Archive => "📦",
                    _ => "📁"
                };
            }
            
            return "📁";
        }

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        {
            throw new NotImplementedException("FileTypeToIconConverter does not support ConvertBack");
        }
    }
}
