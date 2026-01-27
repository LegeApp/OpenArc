# Lessons Learned: WPF Image Rendering for Large Images

## Date: 2026-01-25
## Project: BPG Viewer

---

## The Problem

Large images (3264x2448 in our case) displayed only a small portion (approximately 1/30th of the image, like a 256x256 tile) stretched to fill the entire viewport, despite:
- Correct image decoding (all scanlines decoded)
- Correct WriteableBitmap dimensions
- Correct pixel data (verified with sampling)
- No DPI scaling issues

**The issue was in WPF's rendering system, not the image data.**

---

## What BREAKS Large Image Rendering in WPF

### ❌ 1. Grid with ClipToBounds="True"
```xaml
<Grid ClipToBounds="True">
    <Image Source="{Binding}" />
</Grid>
```
**Problem**: ClipToBounds can interfere with how WPF renders large images, causing only a portion to display.

### ❌ 2. RenderTransform on Large Images
```xaml
<Image Source="{Binding}">
    <Image.RenderTransform>
        <TransformGroup>
            <ScaleTransform />
            <TranslateTransform />
        </TransformGroup>
    </Image.RenderTransform>
</Image>
```
**Problem**: RenderTransform can cause rendering glitches with large images. The transform is applied AFTER layout, which can confuse WPF's rendering engine when combined with large bitmap sources.

### ❌ 3. Complex Transform Chains
Multiple nested transforms (Scale + Translate + custom pan logic) create unpredictable behavior with large images.

### ❌ 4. Alignment Constraints with Transforms
```xaml
<Image HorizontalAlignment="Left"
       VerticalAlignment="Top"
       RenderTransformOrigin="0,0" />
```
When combined with RenderTransform, these constraints can cause WPF to render only a portion of the image.

### ❌ 5. Auto-Fit/Scale Logic During Layout
Calculating window vs. image size during Loaded/SizeChanged events and applying transforms can create race conditions in WPF's rendering pipeline.

---

## What WORKS for Large Image Rendering in WPF

### ✅ 1. ScrollViewer (Standard Pattern)
```xaml
<ScrollViewer HorizontalScrollBarVisibility="Auto"
              VerticalScrollBarVisibility="Auto">
    <Image Source="{Binding}" Stretch="None" />
</ScrollViewer>
```
**Why it works**:
- Standard WPF control designed for large content
- Handles scrolling/panning automatically
- No ClipToBounds issues
- Well-tested for large images

### ✅ 2. LayoutTransform Instead of RenderTransform
```xaml
<Image Source="{Binding}">
    <Image.LayoutTransform>
        <ScaleTransform ScaleX="{Binding ZoomLevel}"
                        ScaleY="{Binding ZoomLevel}" />
    </Image.LayoutTransform>
</Image>
```
**Why it works**:
- Applied BEFORE layout, so WPF knows the final size
- More reliable for scaling large images
- Doesn't interfere with rendering pipeline

### ✅ 3. Stretch="None"
```xaml
<Image Source="{Binding}" Stretch="None" />
```
**Why it works**:
- Displays image at actual pixel dimensions
- No automatic scaling that could interact with transforms
- Predictable behavior

### ✅ 4. Minimal Layout Constraints
Let WPF handle layout naturally:
- No HorizontalAlignment/VerticalAlignment on Image
- No ClipToBounds on parent containers
- No RenderTransformOrigin unless necessary

### ✅ 5. Simple, Predictable Transforms
- Single transform (just ScaleTransform for zoom)
- No complex chains
- Applied via LayoutTransform, not RenderTransform

---

## Best Practices for Large Images in WPF

### Architecture
1. **Use ScrollViewer + Image with LayoutTransform**
   - This is the standard, proven pattern
   - Don't reinvent the wheel with custom pan logic

2. **Keep Transforms Simple**
   - One transform at a time
   - Prefer LayoutTransform over RenderTransform
   - Let ScrollViewer handle panning

3. **Let WPF Handle Layout**
   - Don't fight the layout system
   - Avoid complex size calculations during events
   - Use auto-sizing when possible

### Zoom Implementation
```csharp
// Good: Simple zoom via LayoutTransform
public double ZoomLevel { get; set; } = 1.0;

// In XAML:
<Image.LayoutTransform>
    <ScaleTransform ScaleX="{Binding ZoomLevel}"
                    ScaleY="{Binding ZoomLevel}" />
</Image.LayoutTransform>
```

### Fit-to-Window Implementation
```csharp
// Calculate zoom to fit
double scaleX = availableWidth / image.Width;
double scaleY = availableHeight / image.Height;
ZoomLevel = Math.Min(scaleX, scaleY);

// LayoutTransform automatically updates
```

### Auto-Fit on Load
```csharp
// Good: Use Dispatcher.InvokeAsync with proper priority
Loaded += (s, e) =>
{
    LoadImage(filePath);

    Dispatcher.InvokeAsync(() =>
    {
        // Calculate and apply fit-to-window
        FitToWindow(new Size(ActualWidth, ActualHeight));
    }, DispatcherPriority.Loaded);
};
```

---

## Debugging Large Image Issues

### Step 1: Verify Image Data
Check that decoding is correct:
```csharp
// Log dimensions
Debug.WriteLine($"Image: {width}x{height}");
Debug.WriteLine($"Bitmap: {bitmap.PixelWidth}x{bitmap.PixelHeight}");

// Sample pixels at different locations
Debug.WriteLine($"Pixel (0,0): {GetPixel(0, 0)}");
Debug.WriteLine($"Pixel (width-1, height-1): {GetPixel(width-1, height-1)}");

// Check row checksums to verify data varies
for (int row : [0, height/4, height/2, 3*height/4, height-1])
{
    uint sum = ChecksumRow(row);
    Debug.WriteLine($"Row {row} checksum: {sum}");
}
```
If checksums are all similar → data is repetitive (decoder issue)
If checksums vary → data is correct (rendering issue)

### Step 2: Simplify Rendering
If data is correct but rendering is wrong:
1. Remove all transforms temporarily
2. Remove ClipToBounds
3. Replace custom containers with ScrollViewer
4. Use Stretch="None"

### Step 3: Check DPI
```csharp
Debug.WriteLine($"DPI: {bitmap.DpiX}x{bitmap.DpiY}");
Debug.WriteLine($"Bitmap.Width: {bitmap.Width}");
Debug.WriteLine($"Bitmap.PixelWidth: {bitmap.PixelWidth}");
```
If Width != PixelWidth → DPI scaling is occurring

---

## Key Takeaways

1. **Don't use RenderTransform for zooming large images** - use LayoutTransform
2. **Don't use ClipToBounds on containers with large images** - causes rendering issues
3. **Use ScrollViewer for panning** - don't implement custom pan logic
4. **Keep transforms simple** - single ScaleTransform via LayoutTransform
5. **Trust the standard WPF pattern** - ScrollViewer + Image + LayoutTransform

### The Golden Rule
**When displaying large images in WPF, use the simplest possible approach. Complex transform chains and custom layout logic are unnecessary and cause problems.**

---

## Working Configuration Summary

```xaml
<ScrollViewer HorizontalScrollBarVisibility="Auto"
              VerticalScrollBarVisibility="Auto">
    <Image Source="{Binding DisplayBitmap}"
           Stretch="None"
           RenderOptions.BitmapScalingMode="HighQuality">
        <Image.LayoutTransform>
            <ScaleTransform ScaleX="{Binding ZoomLevel}"
                            ScaleY="{Binding ZoomLevel}"/>
        </Image.LayoutTransform>
    </Image>
</ScrollViewer>
```

```csharp
// ViewModel
public double ZoomLevel { get; set; } = 1.0;

public void FitToWindow(Size availableSize)
{
    double scaleX = availableSize.Width / ImageWidth;
    double scaleY = availableSize.Height / ImageHeight;
    ZoomLevel = Math.Min(scaleX, scaleY);
}

public void ZoomIn() => ZoomLevel *= 1.2;
public void ZoomOut() => ZoomLevel /= 1.2;
public void ActualSize() => ZoomLevel = 1.0;
```

This configuration has been proven to work with images up to 3264x2448 pixels and beyond.
