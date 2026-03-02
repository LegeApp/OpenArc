using System;
using System.Globalization;
using System.Windows.Data;

namespace DocBrake.Converters
{
    public class ValueEqualsConverter : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (parameter == null)
                return false;

            if (value == null)
                return false;

            var paramString = parameter.ToString();
            if (string.IsNullOrWhiteSpace(paramString))
                return false;

            if (value.GetType().IsEnum)
                return string.Equals(value.ToString(), paramString, StringComparison.OrdinalIgnoreCase);

            return string.Equals(System.Convert.ToString(value, CultureInfo.InvariantCulture), paramString, StringComparison.OrdinalIgnoreCase);
        }

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (value is not bool isChecked)
                return Binding.DoNothing;

            if (!isChecked)
                return Binding.DoNothing;

            if (parameter == null)
                return Binding.DoNothing;

            var paramString = parameter.ToString();
            if (string.IsNullOrWhiteSpace(paramString))
                return Binding.DoNothing;

            if (targetType.IsEnum)
                return Enum.Parse(targetType, paramString, ignoreCase: true);

            if (targetType == typeof(int) || targetType == typeof(int?))
                return int.Parse(paramString, CultureInfo.InvariantCulture);

            return System.Convert.ChangeType(paramString, targetType, CultureInfo.InvariantCulture);
        }
    }
}
