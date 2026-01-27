# BPG Viewer WPF GUI

A C# WPF-based GUI for viewing BPG images with zoom and pan support. Built to match OpenARC's DocBrakeGUI architecture for easy integration.

## Architecture Overview

This GUI follows the same MVVM pattern and structure as OpenARC's DocBrakeGUI:

```
BpgViewerGUI/
├── App.xaml / .cs               # Application entry point
├── MainWindow.xaml / .cs        # Main window shell
├── BpgViewerGUI.csproj          # .NET 8.0 WPF project
├── NativeInterop/
│   └── BpgViewerFFI.cs          # P/Invoke bindings to bpg_viewer.dll
├── Models/
│   └── BpgImage.cs              # Managed wrapper for BPG images
├── ViewModels/
│   └── ImageViewerViewModel.cs  # MVVM ViewModel with zoom/pan logic
├── Views/
│   └── ImageViewerControl.xaml  # Image display control
├── Commands/
│   └── RelayCommand.cs          # Command pattern implementation
└── Converters/
    └── BoolToVisibilityConverter.cs  # XAML converters
```

## How RGB Data is Displayed in WPF

### 1. **Rust → C FFI → C# → WPF Pipeline**

```
BPG File
   ↓ (Rust: bpg_viewer library)
RGBA32 byte array in Rust memory
   ↓ (FFI: bpg_viewer_get_rgba32)
IntPtr to RGBA data in C#
   ↓ (BpgImage.cs: unsafe pointer conversion)
WriteableBitmap (Bgra32 format)
   ↓ (WPF Image control)
Displayed on screen
```

### 2. **The WriteableBitmap Approach**

In `Models/BpgImage.cs`, we use `WriteableBitmap` which is the best way to display raw pixel data in WPF:

```csharp
// Create a WriteableBitmap with Bgra32 format (WPF's native format)
Bitmap = new WriteableBitmap(
    (int)Width,
    (int)Height,
    96, 96,                    // DPI
    PixelFormats.Bgra32,       // WPF's fastest format
    null);

// Lock for direct memory access
Bitmap.Lock();

// Get RGBA32 data from Rust
BpgViewerFFI.bpg_viewer_get_rgba32(handle, out IntPtr dataPtr, out UIntPtr size);

// Convert RGBA → BGRA and copy to WriteableBitmap
unsafe {
    byte* srcPtr = (byte*)dataPtr.ToPointer();
    byte* dstPtr = (byte*)Bitmap.BackBuffer.ToPointer();

    for (int y = 0; y < Height; y++) {
        for (int x = 0; x < Width; x++) {
            // RGBA → BGRA conversion
            dstRow[dstIdx + 0] = srcRow[srcIdx + 2]; // B
            dstRow[dstIdx + 1] = srcRow[srcIdx + 1]; // G
            dstRow[dstIdx + 2] = srcRow[srcIdx + 0]; // R
            dstRow[dstIdx + 3] = srcRow[srcIdx + 3]; // A
        }
    }
}

Bitmap.AddDirtyRect(new Int32Rect(0, 0, (int)Width, (int)Height));
Bitmap.Unlock();
```

### 3. **Why WriteableBitmap?**

- **Direct Memory Access**: Lock/unlock pattern allows direct pixel manipulation
- **Performance**: Bgra32 is WPF's native format (no conversion overhead)
- **Real-time Updates**: Can update pixels without recreating the bitmap
- **Thread-Safe**: Proper locking ensures thread safety

### 4. **Alternative Approaches (Not Used)**

We could also use these methods, but they're less efficient:

```csharp
// Method 1: BitmapSource.Create (less flexible)
BitmapSource.Create(
    width, height,
    96, 96,
    PixelFormats.Bgra32,
    null,
    pixelData,
    stride);

// Method 2: RenderTargetBitmap (for drawing, not raw pixels)
var rtb = new RenderTargetBitmap(width, height, 96, 96, PixelFormats.Pbgra32);

// Method 3: Image.Source from file (no good for memory buffers)
new BitmapImage(new Uri(filePath));
```

## Zoom and Pan Implementation

### Transform-Based Approach

In `Views/ImageViewerControl.xaml`, we use WPF transforms for smooth zoom/pan:

```xaml
<Image Source="{Binding DisplayBitmap}">
    <Image.RenderTransform>
        <TransformGroup>
            <!-- Zoom -->
            <ScaleTransform ScaleX="{Binding ZoomLevel}"
                          ScaleY="{Binding ZoomLevel}"/>
            <!-- Pan -->
            <TranslateTransform X="{Binding PanOffset.X}"
                              Y="{Binding PanOffset.Y}"/>
        </TransformGroup>
    </Image.RenderTransform>
</Image>
```

### Mouse Wheel Zoom (Zoom to Cursor)

```csharp
public void HandleMouseWheel(int delta, Point mousePosition)
{
    double zoomFactor = delta > 0 ? 1.1 : 0.9;
    double oldZoom = ZoomLevel;
    ZoomLevel *= zoomFactor;

    // Keep cursor position steady during zoom
    double zoomChange = ZoomLevel / oldZoom;
    PanOffset = new Point(
        mousePosition.X - (mousePosition.X - PanOffset.X) * zoomChange,
        mousePosition.Y - (mousePosition.Y - PanOffset.Y) * zoomChange
    );
}
```

### Drag to Pan

```csharp
private void UserControl_MouseMove(object sender, MouseEventArgs e)
{
    if (e.LeftButton == MouseButtonState.Pressed)
    {
        Point mousePos = e.GetPosition(ImageCanvas);
        ViewModel.UpdatePan(mousePos);
    }
}
```

## Building the Project

### Prerequisites

1. **.NET 8.0 SDK** (Windows)
   ```bash
   winget install Microsoft.DotNet.SDK.8
   ```

2. **Rust toolchain** (for building the native library)
   ```bash
   rustup update stable
   ```

3. **BPG library** (libbpg)
   - Already in `D:\misc\BPG\libbpg-0.9.8\`

### Build Steps

#### 1. Build the Rust Library (cdylib for Windows)

```bash
cd bpg-viewer
cargo build --release --lib
```

This creates `bpg_viewer.dll` in `target/release/`.

#### 2. Build the C# GUI

```bash
cd BpgViewerGUI
dotnet build -c Release
```

Or use Visual Studio 2022:
- Open `BpgViewerGUI.csproj`
- Build → Build Solution (Ctrl+Shift+B)

#### 3. Run the Application

```bash
dotnet run

# Or run with a file
dotnet run -- image.bpg
```

The .csproj automatically copies `bpg_viewer.dll` to the output directory.

## Integration with OpenARC

This GUI is designed to be merged into OpenARC later:

### Similarities with DocBrakeGUI

1. **Same .NET Version**: NET 8.0-windows
2. **Same UI Framework**: WPF with WPF-UI (Fluent Design)
3. **Same Patterns**: MVVM with CommunityToolkit.Mvvm
4. **Same FFI Approach**: P/Invoke to Rust DLL
5. **Same Project Structure**: Views/ViewModels/Models/Services

### How to Merge Later

1. **Add as a new mode** in OpenARC's MainView:
   ```xaml
   <TabItem Header="BPG Viewer">
       <bpgviewer:ImageViewerControl/>
   </TabItem>
   ```

2. **Share the FFI DLL**: Both use similar FFI patterns
   ```csharp
   // OpenARC: openarc_ffi.dll
   // BpgViewer: bpg_viewer.dll
   // Can combine into one FFI DLL later
   ```

3. **Reuse Services**: Thumbnail generation can be shared
   ```csharp
   // BpgViewer.Services.ThumbnailService
   // → OpenARC.Services.BpgThumbnailService
   ```

## Key Features

### Current Features

- ✅ Load and display BPG images
- ✅ Zoom (10% - 1000%)
- ✅ Pan (click and drag)
- ✅ Mouse wheel zoom (zoom to cursor)
- ✅ Keyboard shortcuts (F, 1, +, -, I, Ctrl+O)
- ✅ Drag-and-drop file support
- ✅ Info panel with image metadata
- ✅ Status bar with zoom level
- ✅ Dark theme (matches OpenARC)

### Next Step: Catalog View

The next enhancement will add thumbnail catalog viewing:

```
┌─────────────────────────────────────────┐
│ [Thumbnail Grid]                        │
│  [img1] [img2] [img3] [img4]           │
│  [img5] [img6] [img7] [img8]           │
│                                         │
│ Click thumbnail → full view             │
└─────────────────────────────────────────┘
```

Implementation plan:
1. Create `ThumbnailCatalogViewModel`
2. Use `VirtualizingWrapPanel` for performance
3. Generate thumbnails via `bpg_thumbnail_generate_png`
4. Cache thumbnails in AppData\Local\BpgViewer
5. Mode switcher between Single View and Catalog View

## Performance Considerations

### Memory Usage

- **WriteableBitmap**: ~4 bytes per pixel (BGRA32)
- **Example**: 4K image (3840×2160) = ~32 MB per image
- **Catalog mode**: Use thumbnails (256×256) = ~256 KB per thumbnail

### Rendering Performance

- **GPU Accelerated**: WPF uses DirectX for rendering
- **Transform-based zoom**: No pixel resampling (fast)
- **Lazy loading**: Images loaded on demand

### Optimization Tips

1. **Dispose images** when switching files:
   ```csharp
   CurrentImage?.Dispose(); // Frees Rust memory
   ```

2. **Use thumbnail cache** for catalog mode:
   ```csharp
   string appData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
   string cacheDir = Path.Combine(appData, "BpgViewer", "Cache", "Thumbnails");
   ```

3. **Virtualize thumbnail grid**:
   ```xaml
   <ItemsControl VirtualizingPanel.IsVirtualizing="True">
   ```

## Troubleshooting

### DLL Not Found

```
System.DllNotFoundException: Unable to load DLL 'bpg_viewer.dll'
```

**Solution**: Ensure `bpg_viewer.dll` is in the same directory as the .exe:
```bash
# Manual copy if needed
copy ..\target\release\bpg_viewer.dll bin\Release\net8.0-windows\
```

### Image Won't Load

**Check**:
1. Is the file a valid BPG?
2. Does the Rust library have BPG dependencies (x265, libpng)?
3. Check status bar for error message

### Unsafe Code Error

```
Error CS0227: Unsafe code may only appear if compiling with /unsafe
```

**Solution**: Already set in .csproj:
```xml
<AllowUnsafeBlocks>true</AllowUnsafeBlocks>
```

## Comparison with egui Version

| Feature | WPF (C#) | egui (Rust) |
|---------|----------|-------------|
| Native Look | ✅ Windows native | ❌ Custom styling |
| Integration with OpenARC | ✅ Same stack | ❌ Different stack |
| File Dialogs | ✅ Native dialogs | ⚠️ rfd crate |
| Menu Bar | ✅ Native menus | ⚠️ Custom |
| Accessibility | ✅ Full support | ⚠️ Limited |
| Development Speed | ✅ XAML designer | ⚠️ Code-based |
| Cross-platform | ❌ Windows only | ✅ Win/Mac/Linux |
| Performance | ✅ GPU accelerated | ✅ GPU accelerated |
| Memory Safety | ⚠️ C# (GC) | ✅ Rust (compile-time) |

For OpenARC integration, **WPF is the better choice** since DocBrakeGUI is already WPF.

## License

Part of the BPG Viewer project. See main README for license information.
