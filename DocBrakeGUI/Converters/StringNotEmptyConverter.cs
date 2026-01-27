using System;
using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace DocBrake.Converters
{
    public class StringNotEmptyConverter : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            string stringValue = value as string;
            return string.IsNullOrEmpty(stringValue) ? "(No path)" : stringValue;
        }

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        {
            throw new NotImplementedException();
        }
    }
}