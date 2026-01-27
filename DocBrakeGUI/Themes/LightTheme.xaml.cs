using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;

namespace DocBrake.Themes
{
    public partial class LightTheme : ResourceDictionary
    {
        public LightTheme()
        {
            InitializeComponent();
        }

        private void PART_VerticalScrollBar_ViewportSizeChanged(object sender, SizeChangedEventArgs e)
        {
            if (sender is ScrollBar scrollBar)
            {
                // Update the viewport size when it changes
                scrollBar.ViewportSize = e.NewSize.Height;
            }
        }

        private void PART_VerticalScrollBar_Scroll(object sender, ScrollEventArgs e)
        {
            if (sender is ScrollBar scrollBar && e.ScrollEventType == ScrollEventType.EndScroll)
            {
                // Handle scroll end if needed
            }
        }

        private void PART_HorizontalScrollBar_ViewportSizeChanged(object sender, SizeChangedEventArgs e)
        {
            if (sender is ScrollBar scrollBar)
            {
                // Update the viewport size when it changes
                scrollBar.ViewportSize = e.NewSize.Width;
            }
        }

        private void PART_HorizontalScrollBar_Scroll(object sender, ScrollEventArgs e)
        {
            if (sender is ScrollBar scrollBar && e.ScrollEventType == ScrollEventType.EndScroll)
            {
                // Handle scroll end if needed
            }
        }
    }
}
