# BPG GUI Viewer Guide

A modern, feature-rich GUI application for viewing BPG images with zoom, pan, and catalog support.

## Features

### Current (v0.1)
- ✅ Load and display BPG images
- ✅ Smooth zoom and pan controls
- ✅ Keyboard shortcuts
- ✅ Mouse controls (scroll to zoom, drag to pan)
- ✅ File drag-and-drop support
- ✅ Image information panel
- ✅ Fit to window / actual size modes
- ✅ Status bar with zoom level
- ✅ Menu bar navigation

### Coming Soon
- 🔄 Thumbnail catalog view
- 🔄 Multi-image browsing
- 🔄 Batch operations
- 🔄 Thumbnail cache

## Building

```bash
# Build GUI viewer
make gui

# Or using cargo directly
cargo build --release --features gui --bin bpg-gui

# Build and run
make run-gui
```

The executable will be created at:
- **Windows**: `target/release/bpg-gui.exe`
- **Linux/macOS**: `target/release/bpg-gui`

## Usage

### Starting the Application

```bash
# Run directly
./target/release/bpg-gui

# Open with a specific file
./target/release/bpg-gui image.bpg
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `O` | Open file dialog |
| `F` | Fit image to window |
| `1` | Show actual size (100% zoom) |
| `+` or `=` | Zoom in |
| `-` | Zoom out |
| `I` | Toggle info panel |
| `Q` | Quit application |

### Mouse Controls

| Action | Control |
|--------|---------|
| **Zoom In/Out** | Mouse wheel scroll |
| **Pan** | Click and drag |
| **Open File** | Click empty area |
| **Drag & Drop** | Drag BPG file into window |

## Interface Layout

```
┌─────────────────────────────────────────────────────────┐
│ File  View  Mode  Help                                  │ <- Menu Bar
├──────────────────────────────────┬──────────────────────┤
│                                  │   Image Info         │
│                                  │   ────────────       │
│        Image Display             │   File: image.bpg    │
│        Area                      │   Size: 1920x1080    │
│        (Zoom & Pan)              │   Zoom: 45%          │
│                                  │                      │
│                                  │   Controls:          │
│                                  │   • Scroll: Zoom     │
│                                  │   • Drag: Pan        │
│                                  │   • F: Fit window    │
│                                  └──────────────────────┤
├──────────────────────────────────────────────────────────┤
│ Status: Loaded image.bpg                  Zoom: 45% ... │ <- Status Bar
└──────────────────────────────────────────────────────────┘
```

## Menu Bar

### File Menu
- **Open... (O)**: Open a BPG file using file dialog
- **Quit (Q)**: Exit the application

### View Menu
- **Fit to Window (F)**: Scale image to fit available space
- **Actual Size (1)**: Display image at 100% (1:1 pixel ratio)
- **Zoom In (+)**: Increase zoom level by 20%
- **Zoom Out (-)**: Decrease zoom level by 20%
- **Show Info Panel (I)**: Toggle the information sidebar

### Mode Menu
- **Single Image**: View one image at a time (current mode)
- **Catalog View**: Browse thumbnails (coming soon)

### Help Menu
- **Keyboard Shortcuts**: Display shortcut reference
- **About**: Show version information

## Info Panel

The right sidebar displays:
- **File Information**
  - Filename
  - File path
  - Original dimensions
  - Current zoom level
  - Display size

- **Quick Controls Reference**
  - Mouse and keyboard controls
  - Shortcut reminders

- **Status Messages**
  - Load progress
  - Error messages
  - Operation feedback

## Zoom & Pan Behavior

### Zoom
- **Range**: 10% to 1000%
- **Mouse Wheel**: Fine-grained zoom (±0.1% per tick)
- **Keyboard**: 20% increments/decrements
- **Centered**: Zoom is always centered on image

### Pan
- **Method**: Click and drag with mouse
- **Bounds**: Can pan beyond window edges
- **Reset**: Use "Fit to Window" (F) to recenter

### View Modes
1. **Fit to Window (F)**
   - Automatically scales image to fit available space
   - Maintains aspect ratio
   - Leaves 5% margin around edges
   - Resets pan offset to center

2. **Actual Size (1)**
   - Displays image at 100% zoom (1:1 pixels)
   - No scaling or interpolation
   - Useful for pixel-perfect viewing
   - Resets pan offset to center

## Drag and Drop

The viewer supports drag-and-drop:
1. Drag a `.bpg` file from your file manager
2. Drop it anywhere in the viewer window
3. The image loads automatically

Supported during:
- Empty viewer (no image loaded)
- Image already displayed (replaces current image)

## Status Bar

The bottom status bar shows:
- **Left**: Current status message
  - File loading progress
  - Error messages
  - Operation results
  - Help hints

- **Right**: Quick stats
  - Original image dimensions
  - Current zoom percentage

## Color Scheme

The viewer uses a dark theme optimized for image viewing:
- **Background**: Dark gray (`#1E1E1E`) - neutral, non-distracting
- **UI Elements**: Medium gray - clear but not overwhelming
- **Image Border**: Light gray - subtle frame
- **Empty Area**: Checkerboard pattern - shows transparency

## Image Rendering

- **Interpolation**: Linear filtering for smooth scaling
- **Transparency**: Checkerboard background pattern
- **Border**: 1px gray outline around image
- **Centering**: Image always centered in viewport

## Performance Tips

1. **Large Images**
   - Use "Fit to Window" mode initially
   - Zoom in on specific areas as needed
   - Pan to navigate large images

2. **Multiple Files**
   - Load one file at a time (catalog mode coming soon)
   - Use file dialog for quick switching

3. **Memory Usage**
   - Images are decoded to RGBA32 in memory
   - Close and reopen for very large files
   - Monitor system memory for huge images

## Troubleshooting

### Image Won't Load
- Verify the file is a valid BPG image
- Check file permissions
- Look for error message in status bar

### Poor Performance
- Try "Fit to Window" mode first
- Reduce zoom level for very large images
- Close other applications if system is low on memory

### Display Issues
- Ensure graphics drivers are up to date
- Try toggling info panel (I) if rendering is slow
- Restart application if texture becomes corrupted

## System Requirements

### Minimum
- **OS**: Windows 10, Linux (X11/Wayland), macOS 10.15+
- **RAM**: 2 GB (4 GB recommended for large images)
- **GPU**: Any with OpenGL 3.3+ support

### Recommended
- **OS**: Windows 11, Modern Linux, macOS 12+
- **RAM**: 8 GB
- **GPU**: Dedicated graphics with 1GB+ VRAM

## File Format Support

- **Input**: `.bpg` files only
- **Internal**: RGBA32 for display
- **Export**: Use CLI tools for conversion

## Future Enhancements (Next Release)

### Catalog Mode
- Thumbnail grid view
- Multi-image browsing
- Folder scanning
- Quick preview
- Batch operations

### Additional Features
- Image rotation
- Export to PNG/JPEG
- Slideshow mode
- Full-screen viewing
- Thumbnail cache
- Recent files list
- Preferences dialog

## Development

Built with:
- **eframe**: Cross-platform native windowing
- **egui**: Immediate mode GUI
- **bpg-viewer**: BPG decoding library (FFI to libbpg)
- **image**: Format conversion and scaling
- **rfd**: Native file dialogs

## License

Part of the BPG Viewer project. See main README for license information.

## Getting Help

- **Issues**: Report bugs on GitHub
- **Documentation**: See README.md and INTEGRATION.md
- **Shortcuts**: Press `Help > Keyboard Shortcuts` in the app
