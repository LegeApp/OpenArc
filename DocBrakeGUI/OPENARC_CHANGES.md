# OpenArc GUI Changes Summary

## Overview
The DocBrakeGUI has been substantially cleaned up and repurposed for OpenArc media archiving. The existing GUI framework and structure have been preserved while replacing all document processing logic with media compression and archiving functionality.

## Key Changes

### 1. Models (`Models/`)

#### `ProcessingOptions.cs`
**Before:** Document processing settings (JBIG2, CCITT4, JPEG, etc.)
**After:** OpenArc compression settings
- `ArchiveMode` (Phone/Standard)
- BPG image settings (quality, lossless)
- FFmpeg video settings (preset, CRF)
- Archive method (ArcMax/Zstd)
- Phone mode settings (source path, auto-detect)

#### `DocumentItem.cs`
**Before:** PDF document metadata
**After:** Media file metadata
- Added `FileType` enum (Image, Video, Document, Archive)
- Added `CompressedSize` and `CompressionRatio` properties
- Retained status tracking (Pending, Processing, Completed, Error, Cancelled)

### 2. ViewModels (`ViewModels/`)

#### `SettingsViewModel.cs`
**Before:** Document format and binarization settings
**After:** Media compression settings
- BPG quality and lossless mode controls
- FFmpeg video preset and CRF controls
- Archive method selection (ArcMax/Zstd)
- Compression level control
- Phone source path configuration
- Output archive path selection

#### `MainViewModel.cs`
**Before:** PDF file processing queue
**After:** Media file archiving queue
- Changed file filters from `.pdf` to media extensions (`.jpg`, `.png`, `.mp4`, `.mov`, etc.)
- Added file type detection (Image/Video/Document/Archive)
- Updated drag-and-drop to support media files and folders
- Changed output from directory to archive file (`.oarc`)
- Retained queue management, progress tracking, and status updates

### 3. UI (`MainWindow.xaml`)

**Before:** "DocBrake Document Processor"
**After:** "OpenArc - Intelligent Media Archiver"
- Updated window title and branding
- Changed icon from 📄 to 📦
- Increased default window size (700x1100)
- Retained FluentWindow styling and drag-and-drop support

### 4. Converters (`Converters/`)

#### New: `FileTypeToIconConverter.cs`
Maps file types to emoji icons:
- Image → 🖼️
- Video → 🎬
- Document → 📄
- Archive → 📦
- Unknown → 📁

#### Existing: `DocumentStatusToColorConverter.cs`
Retained as-is (works for both document and media file statuses)

### 5. Native Interop (`NativeInterop/`)

#### New: `OpenArcFFI.cs`
FFI interface to Rust backend:
- `CompressionSettings` struct for BPG/FFmpeg/Archive settings
- `CreateArchive()` - Create .oarc archive from files
- `ExtractArchive()` - Extract .oarc archive
- `DetectFileType()` - Detect media file types
- Progress callback support

## Architecture

### Phone Mode vs Standard Mode
The GUI is designed to support two archiving modes:

**Phone Mode:**
- Automatically detects phone storage when connected
- Optimized presets for phone photos/videos
- Quick one-click archiving
- Source path: Phone storage location

**Standard Mode:**
- Manual file/folder selection
- Customizable compression settings
- Drag-and-drop support
- Source: Any local files

### File Type Routing
Files are automatically routed to appropriate codecs:
- **Images** (JPG, PNG, TIFF, RAW) → BPG codec
- **Videos** (MP4, MOV, AVI, MKV) → FFmpeg codec
- **Other files** → ArcMax or Zstandard

### Compression Settings

**BPG (Images):**
- Quality: 0-51 (lower = better quality)
- Lossless mode toggle

**FFmpeg (Videos):**
- Preset: Phone, Camera, Fast, Quality
- CRF: 0-51 (lower = better quality)

**Archive:**
- Method: ArcMax (FreeARC) or Zstandard
- Compression level: 1-9

## What Was Preserved

1. **GUI Framework:** WPF-UI (FluentWindow, modern styling)
2. **MVVM Pattern:** ViewModels, Commands, INotifyPropertyChanged
3. **Dependency Injection:** Microsoft.Extensions.DependencyInjection
4. **Services Layer:** IFileDialogService, ISettingsService
5. **Queue Management:** ObservableCollection, status tracking
6. **Progress Tracking:** Real-time progress updates
7. **Drag-and-Drop:** File and folder drag-and-drop support
8. **Error Handling:** Try-catch, error messages, cancellation

## What Was Removed

1. Document-specific logic (PDF processing, page detection)
2. Binarization settings (threshold, window size, k-factor)
3. EPUB generation
4. Model path validation
5. Document format options (JBIG2, CCITT4, etc.)

## Integration Points

### Rust Backend
The C# GUI communicates with the Rust backend via FFI:
- `openarc_ffi.dll` - Rust library exposing C ABI
- Progress callbacks for real-time updates
- Error handling via `GetLastError()`

### Workspace Structure
```
openarc/
├── DocBrakeGUI/          # C# WPF GUI (this project)
├── openarc-gui/          # Rust egui GUI (alternative)
├── arcmax/               # FreeARC compression
├── zstd-archive/         # Zstandard compression
└── codecs/               # BPG and FFmpeg codecs
```

## Next Steps

1. **Implement Phone Mode Detection:**
   - Auto-detect connected phones (USB/MTP)
   - Scan for DCIM folders
   - Populate file queue automatically

2. **Add Mode Tabs:**
   - Create TabControl in MainView
   - "Phone Mode" tab with auto-detection
   - "Standard Mode" tab with manual selection

3. **Build Rust FFI:**
   - Create `openarc-ffi` crate
   - Implement C ABI exports
   - Build `openarc_ffi.dll`

4. **Test Integration:**
   - Test archive creation with sample files
   - Verify compression settings work
   - Test progress callbacks

5. **Polish UI:**
   - Add file type icons in queue
   - Show compression ratio after completion
   - Add archive preview/extraction

## Building

```powershell
# Build C# GUI
cd DocBrakeGUI
dotnet build -c Release

# Build Rust backend (future)
cd ..
cargo build -p openarc-ffi --release

# Copy DLL to GUI output
copy target\release\openarc_ffi.dll DocBrakeGUI\bin\Release\net8.0-windows\
```

## Running

```powershell
cd DocBrakeGUI
dotnet run
```

Or run the executable:
```powershell
.\bin\Release\net8.0-windows\DocBrakeGUI.exe
```

## Notes

- The GUI retains the "DocBrake" namespace to avoid breaking existing references
- All document processing services can be replaced with OpenArc services
- The existing Views folder structure can be reused for Phone/Standard mode tabs
- Settings are saved to JSON (compatible with existing settings service)
