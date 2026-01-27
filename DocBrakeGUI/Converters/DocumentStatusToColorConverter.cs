using System;
using System.Globalization;
using System.Windows.Data;
using System.Windows.Media;
using DocBrake.Models;

namespace DocBrake.Converters
{
    /// <summary>
    /// Converts DocumentStatus enum values to appropriate color brushes
    /// </summary>
    public class DocumentStatusToColorConverter : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (value is DocumentStatus status)
            {            return status switch
            {
                DocumentStatus.Pending => new SolidColorBrush(Colors.Gray),
                DocumentStatus.Processing => new SolidColorBrush(Colors.Orange),
                DocumentStatus.Completed => new SolidColorBrush(Colors.Green),
                DocumentStatus.Error => new SolidColorBrush(Colors.Red),
                DocumentStatus.Cancelled => new SolidColorBrush(Colors.DarkGray),
                _ => new SolidColorBrush(Colors.Black)
            };
            }
            
            return new SolidColorBrush(Colors.Black);
        }

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        {
            throw new NotImplementedException("DocumentStatusToColorConverter does not support ConvertBack");
        }
    }
}
