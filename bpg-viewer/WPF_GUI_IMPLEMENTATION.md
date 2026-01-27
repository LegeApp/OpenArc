# BPG Viewer WPF GUI - Implementation Summary

## Overview

A complete C# WPF GUI for viewing BPG images with zoom and pan capabilities, designed to match OpenARC's DocBrakeGUI architecture for seamless future integration.

## Key Question: How to Display RGB Data in WPF?

### The Solution: WriteableBitmap with Unsafe Pointer Manipulation

WPF's `WriteableBitmap` class provides direct access to pixel data, making it perfect for displaying RGB/RGBA data from native libraries.

```csharp
// 1. Create a WriteableBitmap (Bgra32 is WPF's native format)
WriteableBitmap bitmap = new WriteableBitmap(
    width, height,
    96, 96,                    // DPI
    PixelFormats.Bgra32,       // Most efficient format
    null);                     // No palette

// 2. Lock the bitmap for writing
bitmap.Lock();

// 3. Get raw RGBA data from Rust FFI
BpgViewerFFI.bpg_viewer_get_rgba32(handle, out IntPtr dataPtr, out UIntPtr size);

// 4. Copy and convert RGBA → BGRA using unsafe pointers
unsafe {
    byte* srcPtr = (byte*)dataPtr.ToPointer();     // Rust memory
    byte* dstPtr = (byte*)bitmap.BackBuffer.ToPointer();  // WPF memory

    for (int y = 0; y < height; y++) {
        byte* srcRow = srcPtr + (y * width * 4);
        byte* dstRow = dstPtr + (y * stride);

        for (int x = 0; x < width; x++) {
            // RGBA → BGRA pixel conversion
            dstRow[x*4 + 0] = srcRow[x*4 + 2];  // B
            dstRow[x*4 + 1] = srcRow[x*4 + 1];  // G
            dstRow[x*4 + 2] = srcRow[x*4 + 0];  // R
            dstRow[x*4 + 3] = srcRow[x*4 + 3];  // A
        }
    }

    // 5. Mark as changed and unlock
    bitmap.AddDirtyRect(new Int32Rect(0, 0, width, height));
    bitmap.Unlock();
}

// 6. Free Rust-allocated memory
BpgViewerFFI.bpg_viewer_free_buffer(dataPtr, size);

// 7. Bind to WPF Image control
<Image Source="{Binding DisplayBitmap}"/>
```

### Why This Approach?

1. **Performance**: Direct memory access (no managed array copying)
2. **Efficiency**: Bgra32 is WPF's native format (no conversion overhead)
3. **Control**: Full pixel-level control for effects/transformations
4. **Real-time**: Can update pixels dynamically

## Architecture Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                        User Interaction                       │
│  (Click, Drag, Scroll, Keyboard)                             │
└────────────────┬─────────────────────────────────────────────┘
                 │
                 ▼
┌────────────────────────────────────────────────────────────────┐
│                    WPF Layer (C#)                              │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ MainWindow.xaml                                          │ │
│  │  • Menu bar (File, View, Mode, Help)                    │ │
│  │  • Status bar (zoom, dimensions)                        │ │
│  │  • Info panel (image metadata)                          │ │
│  └──────┬───────────────────────────────────────────────────┘ │
│         │                                                      │
│         ▼                                                      │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ ImageViewerControl.xaml                                  │ │
│  │  • Canvas with Image                                     │ │
│  │  • TransformGroup (Scale + Translate)                   │ │
│  │  • Mouse/Keyboard event handlers                        │ │
│  └──────┬───────────────────────────────────────────────────┘ │
│         │                                                      │
│         ▼                                                      │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ ImageViewerViewModel (MVVM)                              │ │
│  │  • Properties: ZoomLevel, PanOffset, DisplayBitmap      │ │
│  │  • Commands: ZoomIn, ZoomOut, FitToWindow, Open         │ │
│  │  • Logic: Zoom to cursor, pan calculations              │ │
│  └──────┬───────────────────────────────────────────────────┘ │
│         │                                                      │
│         ▼                                                      │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ BpgImage Model                                           │ │
│  │  • Manages BPG image lifecycle                          │ │
│  │  • Converts RGBA → WriteableBitmap                      │ │
│  │  • Disposes Rust resources                             │ │
│  └──────┬───────────────────────────────────────────────────┘ │
│         │                                                      │
└─────────┼──────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    FFI Boundary                                 │
│  BpgViewerFFI.cs (P/Invoke)                                    │
│    [DllImport("bpg_viewer.dll")]                              │
│    • bpg_viewer_decode_file(path) → IntPtr                   │
│    • bpg_viewer_get_rgba32(handle, out data, out size)       │
│    • bpg_viewer_free_image(handle)                           │
└─────────┬───────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Rust Layer                                   │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ src/lib.rs (C FFI exports)                               │  │
│  │  #[no_mangle]                                            │  │
│  │  pub extern "C" fn bpg_viewer_decode_file(...)           │  │
│  └──────┬───────────────────────────────────────────────────┘  │
│         │                                                       │
│         ▼                                                       │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ src/decoder.rs                                           │  │
│  │  • decode_file(path) → DecodedImage                     │  │
│  │  • to_rgba32() → Vec<u8>                                │  │
│  └──────┬───────────────────────────────────────────────────┘  │
│         │                                                       │
│         ▼                                                       │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ src/ffi.rs (libbpg bindings)                            │  │
│  │  extern "C" {                                            │  │
│  │    fn bpg_decode_file(...);                             │  │
│  │    fn bpg_free(...);                                    │  │
│  │  }                                                       │  │
│  └──────┬───────────────────────────────────────────────────┘  │
└─────────┼───────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Native Layer (C/C++)                         │
│  libbpg (BPG decoder)                                          │
│    • x265 (HEVC decoder)                                       │
│    • libpng, libjpeg (image I/O)                              │
│                                                                 │
│  BPG File → HEVC decode → YUV → RGB → Output                  │
└─────────────────────────────────────────────────────────────────┘
```

## Data Flow: File to Screen

```
1. User clicks "Open" or drags BPG file
   ↓
2. ImageViewerViewModel.LoadImage(filePath)
   ↓
3. BpgImage.Load(filePath)
   ↓
4. P/Invoke: BpgViewerFFI.bpg_viewer_decode_file(filePath)
   ↓ [FFI BOUNDARY - Managed → Native]
   ↓
5. Rust: decode_file(filePath)
   ↓
6. Rust: bpg_decode_file() calls libbpg
   ↓
7. libbpg decodes BPG → RGBA32 in Rust memory
   ↓
8. P/Invoke: BpgViewerFFI.bpg_viewer_get_rgba32(handle)
   ↓ [FFI BOUNDARY - Native → Managed]
   ↓
9. C#: Receives IntPtr to RGBA data
   ↓
10. C#: Unsafe pointer copy RGBA → WriteableBitmap (BGRA)
   ↓
11. C#: Free Rust memory with bpg_viewer_free_buffer()
   ↓
12. WPF: Image control displays WriteableBitmap
   ↓
13. GPU renders to screen
```

## Zoom and Pan Implementation

### Transform-Based (No Pixel Resampling)

```xaml
<Image Source="{Binding DisplayBitmap}">
    <Image.RenderTransform>
        <TransformGroup>
            <ScaleTransform ScaleX="{Binding ZoomLevel}"
                          ScaleY="{Binding ZoomLevel}"/>
            <TranslateTransform X="{Binding PanOffset.X}"
                              Y="{Binding PanOffset.Y}"/>
        </TransformGroup>
    </Image.RenderTransform>
</Image>
```

**Benefits**:
- GPU-accelerated (no CPU pixel operations)
- Smooth and fast
- Works with any zoom level
- No quality loss

### Zoom to Cursor Algorithm

```csharp
public void HandleMouseWheel(int delta, Point mousePosition)
{
    // Calculate zoom factor from mouse wheel
    double zoomFactor = delta > 0 ? 1.1 : 0.9;

    // Store old zoom level
    double oldZoom = ZoomLevel;

    // Apply zoom (clamped to 10%-1000%)
    ZoomLevel *= zoomFactor;
    ZoomLevel = Math.Max(0.1, Math.Min(10.0, ZoomLevel));

    // Calculate how much the zoom changed
    double zoomChange = ZoomLevel / oldZoom;

    // Adjust pan offset to keep cursor position steady
    // Formula: new_offset = cursor - (cursor - old_offset) * zoom_ratio
    PanOffset = new Point(
        mousePosition.X - (mousePosition.X - PanOffset.X) * zoomChange,
        mousePosition.Y - (mousePosition.Y - PanOffset.Y) * zoomChange
    );
}
```

This ensures the point under the cursor stays in the same place when zooming.

## Compatibility with OpenARC

### Matching Architecture

| Component | OpenARC DocBrakeGUI | BPG Viewer GUI | Match |
|-----------|---------------------|----------------|-------|
| Framework | .NET 8.0-windows | .NET 8.0-windows | ✅ |
| UI Toolkit | WPF | WPF | ✅ |
| UI Library | WPF-UI 4.0 | WPF-UI 4.0 | ✅ |
| Pattern | MVVM (CommunityToolkit.Mvvm) | MVVM (CommunityToolkit.Mvvm) | ✅ |
| DI | Microsoft.Extensions.DependencyInjection | Microsoft.Extensions.DependencyInjection | ✅ |
| FFI | P/Invoke to openarc_ffi.dll | P/Invoke to bpg_viewer.dll | ✅ |
| Structure | Views/ViewModels/Models/Services | Views/ViewModels/Models/Services | ✅ |

### Integration Path

To merge BPG Viewer into OpenARC:

1. **Add as a new tab** in OpenARC's MainView:
```xaml
<TabControl>
    <TabItem Header="Archive">...</TabItem>
    <TabItem Header="BPG Viewer">
        <bpgviewer:ImageViewerControl/>
    </TabItem>
</TabControl>
```

2. **Combine FFI DLLs**:
```rust
// In openarc-ffi/src/lib.rs
pub mod bpg_viewer;  // Include BPG viewer exports

#[no_mangle]
pub extern "C" fn bpg_viewer_decode_file(...) {
    bpg_viewer::decode_file(...)
}
```

3. **Share services**:
```csharp
// OpenARC.Services.BpgViewerService
// OpenARC.Services.ThumbnailCacheService
```

## File Structure

```
BpgViewerGUI/
├── BpgViewerGUI.csproj          # .NET 8.0 WPF project
├── App.xaml / .cs               # Application entry, DI setup
├── MainWindow.xaml / .cs        # Main window with menus
│
├── NativeInterop/
│   └── BpgViewerFFI.cs          # P/Invoke declarations
│
├── Models/
│   └── BpgImage.cs              # Managed image wrapper
│                                # ← RGB → WriteableBitmap conversion here
│
├── ViewModels/
│   └── ImageViewerViewModel.cs  # MVVM logic, zoom/pan
│
├── Views/
│   └── ImageViewerControl.xaml  # Image display canvas
│
├── Commands/
│   └── RelayCommand.cs          # ICommand implementation
│
├── Converters/
│   └── BoolToVisibilityConverter.cs  # XAML converters
│
└── Resources/
    └── Icons/                   # UI icons
```

## Building

### Quick Build

```powershell
# Windows PowerShell
.\build-gui.ps1

# Or manually:
cargo build --release --lib
dotnet build BpgViewerGUI\BpgViewerGUI.csproj -c Release
```

### Output

- **Rust DLL**: `target/release/bpg_viewer.dll`
- **C# EXE**: `BpgViewerGUI/bin/Release/net8.0-windows/BpgViewerGUI.exe`

The .csproj automatically copies the DLL to the output directory.

## Next Steps: Catalog View

The next enhancement is thumbnail catalog viewing:

### Design

```
┌─────────────────────────────────────────────────────────────┐
│ [Folder] C:\Photos\Vacation\                                │
├─────────────────────────────────────────────────────────────┤
│  ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐                │
│  │img1│ │img2│ │img3│ │img4│ │img5│ │img6│                │
│  └────┘ └────┘ └────┘ └────┘ └────┘ └────┘                │
│  ┌────┐ ┌────┐ ┌────┐ ┌────┐                              │
│  │img7│ │img8│ │img9│ │im10│                              │
│  └────┘ └────┘ └────┘ └────┘                              │
│                                                             │
│  [Click thumbnail to view full size]                       │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Plan

1. **ViewModel**: `ThumbnailCatalogViewModel`
   - ObservableCollection of thumbnail items
   - Directory scanning
   - Thumbnail generation via FFI

2. **View**: `ThumbnailCatalogView.xaml`
   ```xaml
   <ItemsControl ItemsSource="{Binding Thumbnails}">
       <ItemsControl.ItemsPanel>
           <VirtualizingWrapPanel/>  <!-- Performance -->
       </ItemsControl.ItemsPanel>
       <ItemsControl.ItemTemplate>
           <DataTemplate>
               <Image Source="{Binding ThumbnailBitmap}"
                      Width="256" Height="256"/>
           </DataTemplate>
       </ItemsControl.ItemTemplate>
   </ItemsControl>
   ```

3. **Caching**:
   ```csharp
   string appData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
   string cacheDir = Path.Combine(
       appData,
       "BpgViewer",
       "Cache",
       "Thumbnails");
   ```

4. **Generation**:
   ```csharp
   BpgViewerFFI.bpg_thumbnail_generate_png(
       thumbnailHandle,
       inputPath,
       cachePath);
   ```

## Performance Benchmarks (Estimated)

| Image Size | Decode Time | RGBA Convert | Display Time |
|------------|-------------|--------------|--------------|
| 1920×1080 | ~100ms | ~5ms | <1ms |
| 3840×2160 (4K) | ~400ms | ~20ms | <1ms |
| 7680×4320 (8K) | ~1600ms | ~80ms | <1ms |

*Note: Times vary based on CPU, GPU, and compression settings*

## Summary

This WPF GUI provides:

✅ Full BPG image viewing
✅ Smooth zoom and pan
✅ WriteableBitmap-based RGB display
✅ OpenARC-compatible architecture
✅ Ready for catalog view enhancement
✅ FFI-based integration with Rust

The key innovation is using `WriteableBitmap` with unsafe pointers to efficiently copy and convert RGB data from Rust memory to WPF's rendering pipeline, providing native-like performance while maintaining the safety and convenience of managed code where appropriate.
