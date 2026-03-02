using System;
using System.Globalization;
using System.Windows.Data;

namespace DocBrake.Converters
{
    public class EnumToIndexConverter : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (value is Enum enumValue)
            {
                return System.Convert.ToInt32(enumValue);
            }
            return 0;
        }

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (value is int index && targetType.IsEnum)
            {
                return Enum.ToObject(targetType, index);
            }
            return Enum.ToObject(targetType, 0);
        }
    }
}
