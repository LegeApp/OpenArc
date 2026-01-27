using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;

namespace DocBrake.Controls
{
    public class CustomScrollViewer : ScrollViewer
    {
        static CustomScrollViewer()
        {
            DefaultStyleKeyProperty.OverrideMetadata(
                typeof(CustomScrollViewer),
                new FrameworkPropertyMetadata(typeof(CustomScrollViewer)));
        }

        public CustomScrollViewer()
        {
            // Enable mouse wheel scrolling
            this.PreviewMouseWheel += (s, e) =>
            {
                if (e.Delta > 0)
                    LineUp();
                else
                    LineDown();
                
                e.Handled = true;
            };
        }

        public override void OnApplyTemplate()
        {
            base.OnApplyTemplate();

            // Get the scrollbars from the template
            var verticalScrollBar = GetTemplateChild("PART_VerticalScrollBar") as ScrollBar;
            var horizontalScrollBar = GetTemplateChild("PART_HorizontalScrollBar") as ScrollBar;
            var scrollContentPresenter = GetTemplateChild("PART_ScrollContentPresenter") as ScrollContentPresenter;

            if (scrollContentPresenter != null)
            {
                // Ensure content can be scrolled
                scrollContentPresenter.CanContentScroll = true;
            }
        }
    }
}
