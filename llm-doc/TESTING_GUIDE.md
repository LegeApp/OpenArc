# OpenArc GUI Testing Guide

## Overview
This guide will help you test the OpenArc GUI application from loading files to creating a compressed archive.

## Prerequisites

### Required Files
1. **openarc_ffi.dll** - The Rust FFI library must be built and available
   - Location: Should be in the same directory as the GUI executable or in the system PATH
   - Build command: `cargo build --release -p openarc-ffi`

2. **DocBrakeGUI.exe** - The C# WPF GUI application
   - Build in Visual Studio or with: `dotnet build DocBrakeGUI.csproj`

### Test Media Files
Prepare a test folder with:
- JPEG images (*.jpg, *.jpeg)
- PNG images (*.png)
- MP4 videos (*.mp4)
- MOV videos (*.mov)

## Complete Testing Workflow

### Step 1: Launch the Application
1. Run `DocBrakeGUI.exe` (or `reduced-lege.exe`)
2. The main window should open with two mode buttons: "Phone Mode" and "Standard Mode"
3. Default mode is "Phone Mode"

### Step 2: Configure Settings

#### Open Settings Panel
1. Click the **⚙️ Settings** button in the header of either mode view
2. A settings panel will slide in from the right side
3. The panel shows all available compression and archive settings

#### Required: Set Output Archive Path
**⚠️ CRITICAL**: You must set an output archive path before processing!

1. In the Settings panel, find **"Output Settings"** section
2. Click **"Browse..."** button next to "Archive Path"
3. Choose a location and filename for your archive (e.g., `C:\Archives\test.oarc`)
4. The path will appear in the text box
5. A green indicator with 📦 icon will appear in the bottom status bar showing the path

#### Optional: Adjust Compression Settings

**Image Compression (BPG)**
- **Quality**: 0-51 (lower = better quality, larger files)
  - Default: 25
  - Recommended: 20-30 for photos
- **Lossless**: Enable for perfect quality (much larger files)

**Video Compression (FFmpeg)**
- **Preset**: Choose encoding profile
  - Phone (H264/Medium) - Good for phone videos
  - Camera (H265/Medium) - Better compression for camera footage
  - Fast (H264/Fast) - Quick encoding
  - Quality (H265/Slow) - Best quality/compression ratio
- **CRF**: 0-51 (lower = better quality, larger files)
  - Default: 23
  - Recommended: 18-28

**Archive Compression**
- **Method**: 
  - ArcMax (FreeARC-based) - Better compression
  - Zstandard - Faster, good compression
- **Level**: 1-22 (higher = better compression, slower)
  - Default: 3
  - Recommended: 3-10

**Backup Features**
- ✅ **Enable catalog** - Track backups for incremental archiving
- ✅ **Enable deduplication** - Skip duplicate files
- ✅ **Skip already compressed videos** - Don't re-encode efficient videos

#### Save Settings (Optional)
- Click **"Save Settings"** button at the bottom of the settings panel
- Settings will persist between sessions

#### Close Settings Panel
- Click the **✕** button in the settings panel header
- Or click **⚙️ Settings** button again to toggle

### Step 3: Add Files to Queue

#### Standard Mode (Manual Selection)
1. Switch to **"Standard Mode"** if not already selected
2. Add files using one of these methods:
   - Click **📁 Add Files** - Select individual files
   - Click **📂 Add Folder** - Select entire folder (recursive)
   - **Drag & Drop** - Drag files or folders directly onto the window
3. Files will appear in the queue with:
   - Type icon (📷 for images, 🎬 for videos)
   - File name
   - File size
   - Status (Pending)

#### Phone Mode (Phone Storage)
1. Switch to **"Phone Mode"**
2. Click **📂 Import Folder** to browse for phone storage location
3. Or drag & drop phone media files/folders onto the window
4. Files will be added to the queue

#### Managing the Queue
- **Remove files**: Select a file and click **🗑️ Clear All** (removes all files)
- **View count**: Bottom right shows "Files: X"

### Step 4: Verify Configuration

Before processing, verify:
1. ✅ Files are loaded in the queue (Files: X shows count > 0)
2. ✅ Output archive path is set (green 📦 indicator in bottom bar)
3. ✅ Settings are configured as desired

**If output path is NOT set:**
- ⚠️ Orange warning appears: "Set output archive path in Settings"
- Process button will show error: "Please set an output archive path"

### Step 5: Process Files

1. Click the **"Process"** button (blue/primary button in bottom right)
2. Processing will begin:
   - Progress bar appears showing overall progress
   - Status message updates with current file being processed
   - File counter shows: "X/Y" (current/total)
   - Individual file status updates in the queue

#### During Processing
- **Progress indicators**:
  - Overall progress bar (0-100%)
  - Current file name displayed
  - Files processed counter
- **Cancel**: Click **"Cancel"** button to stop processing
  - Files will be marked as "Cancelled"

### Step 6: Completion

When processing completes:
1. Progress bar disappears
2. Status message shows: "Processing completed"
3. Files in queue show status:
   - ✅ Completed - Successfully processed
   - ❌ Error - Processing failed (with error message)
4. **Output file created**: Check the output archive path you specified
   - File format: `.oarc` (OpenArc Archive)
   - This is a TAR+ZSTD archive containing compressed media

### Step 7: Test Archive Operations

#### Archive Listing
1. Click the **"List"** button in the bottom bar
2. Select an `.oarc` archive file
3. View the file list in the popup dialog
4. Status bar shows: "Listed X files"

#### Archive Extraction
1. Click the **"Extract"** button in the bottom bar
2. Select an `.oarc` archive file
3. Choose an output directory for extraction
4. Progress bar shows extraction progress
5. Status message shows: "Extraction completed" when done
6. Check the output directory for extracted files

### Step 7: Verify Output

The output `.oarc` file should contain:
- Compressed images (BPG format)
- Compressed videos (H264/H265)
- Metadata and catalog (if enabled)
- Hash verification file (HASHES.sha256)

**File size**: Should be significantly smaller than original files, depending on settings.

## Troubleshooting

### Common Issues

#### "DLL not found" error
- **Cause**: `openarc_ffi.dll` is missing
- **Solution**: 
  1. Build the FFI library: `cargo build --release -p openarc-ffi`
  2. Copy `target/release/openarc_ffi.dll` to the GUI executable directory
  3. Or add the DLL location to your PATH

#### "Please set an output archive path"
- **Cause**: Output archive path not configured
- **Solution**: Open Settings → Browse for archive path → Save

#### Processing fails immediately
- **Cause**: Invalid input files or missing codecs
- **Solution**: 
  - Check error message in status bar
  - Verify FFmpeg and BPG codecs are available
  - Check `startup.log` for detailed errors

#### Settings button doesn't work
- **Cause**: ViewModel binding issue
- **Solution**: Check that MainViewModel.ShowSettingsCommand is properly bound

#### Files not appearing in queue
- **Cause**: Unsupported file types
- **Solution**: Only these extensions are supported:
  - Images: .jpg, .jpeg, .png, .bmp, .tiff, .raw, .cr2, .nef, .arw
  - Videos: .mp4, .mov, .avi, .mkv, .webm

### Debug Logs

Check these files for detailed error information:
- `startup.log` - Application startup and initialization
- Console output (if running from command line)

## Expected Results

### Successful Test Run
1. ✅ GUI launches without errors
2. ✅ Settings panel opens and closes smoothly
3. ✅ Files can be added via all methods (browse, drag-drop)
4. ✅ Output path can be set and is displayed
5. ✅ Processing starts and shows progress
6. ✅ Processing completes successfully
7. ✅ Output `.oarc` file is created
8. ✅ File size is reduced compared to originals

### Performance Expectations
- **Images**: 50-80% size reduction (lossy), 10-30% (lossless)
- **Videos**: 30-70% size reduction (depends on source codec)
- **Processing speed**: Varies by hardware and settings
  - Fast preset: ~1-2 seconds per image, ~5-30 seconds per video
  - Quality preset: ~2-5 seconds per image, ~30-120 seconds per video

## Next Steps After Testing

Once basic workflow is confirmed:
1. Test with larger file sets (100+ files)
2. Test different compression settings
3. Test extraction (when implemented)
4. Test incremental backups (catalog feature)
5. Test deduplication with duplicate files
6. Performance profiling with various file types

## Known Limitations (Alpha Version)

- ⚠️ Archive listing shows basic file info only (full parsing not implemented)
- ⚠️ No progress for individual file compression
- ⚠️ Phone auto-detection is basic (USB devices only, limited MTP support)
- ⚠️ Large video files may take significant time to process
- ⚠️ Extraction settings (BPG decode quality) not configurable in GUI

## Reporting Issues

When reporting issues, include:
1. Steps to reproduce
2. Error messages from status bar
3. Contents of `startup.log`
4. File types and sizes being processed
5. Settings configuration used
6. Expected vs actual behavior
