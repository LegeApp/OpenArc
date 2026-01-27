# OpenArc Quick Start Guide

## Installation & First Run

### Step 1: Install Prerequisites

```powershell
# Install Rust (if not already installed)
# Visit: https://rustup.rs/
# Then verify:
rustc --version

# Install .NET SDK 8.0+ (if not already installed)
# Visit: https://dotnet.microsoft.com/download
# Then verify:
dotnet --version
```

### Step 2: Build the Project

```powershell
# Navigate to project root
cd D:\misc\arc\openarc

# Build C# component first
cd DocBrakeGUI
dotnet build -c Release
cd ..

# Build arcmax codecs (if not already built)
cd arcmax
.\build_codecs.bat
cd ..

# Build entire workspace
cargo build --workspace --release
```

### Step 3: Run OpenArc

```powershell
# From project root
cargo run -p openarc-gui --release
```

## Using OpenArc

### Basic Workflow

1. **Add Files**
   - Click "📁 Add Files" to select individual files
   - Click "📂 Add Folder" to add entire directories
   - Or drag and drop files/folders into the window

2. **Configure Compression**
   - Expand "Image Settings (BPG)" to adjust image quality
   - Expand "Video Settings (FFmpeg)" to choose video preset
   - Expand "Archive Settings" to select compression method

3. **Set Output Location**
   - Click "Browse..." next to "Output archive"
   - Choose location and name for your .oarc archive file

4. **Start Compression**
   - Click "🚀 Start Compression"
   - Monitor progress in real-time
   - Wait for completion message

### Recommended Settings

#### For Phone Photos & Videos
```
Image Settings:
  Quality: 23 (good balance)
  Lossless: OFF

Video Settings:
  Preset: Phone
  CRF: 23

Archive Settings:
  Compression: ArcMax (FreeARC)
```

#### For Camera RAW Files
```
Image Settings:
  Quality: 0 (best quality)
  Lossless: ON

Video Settings:
  Preset: Camera
  CRF: 20

Archive Settings:
  Compression: ArcMax (FreeARC)
```

#### For Quick Backup
```
Image Settings:
  Quality: 28 (faster)
  Lossless: OFF

Video Settings:
  Preset: Fast
  CRF: 28

Archive Settings:
  Compression: Zstandard
```

## Understanding Compression Settings

### Image Quality (BPG)
- **0-17**: Excellent quality, larger files
- **18-28**: Good quality, balanced size
- **29-51**: Lower quality, smaller files
- **Lossless**: No quality loss (recommended for RAW)

### Video CRF (Constant Rate Factor)
- **0-17**: Visually lossless, very large
- **18-23**: High quality (recommended)
- **24-28**: Good quality, smaller files
- **29-51**: Lower quality, much smaller

### Video Presets
- **Phone**: Optimized for phone videos (H.264, 1080p)
- **Camera**: High quality for camera footage (H.265, 4K)
- **Fast**: Quick compression, lower quality
- **Quality**: Best quality, slower compression

### Archive Methods
- **ArcMax (FreeARC)**: Best compression ratio, slower
- **Zstandard**: Fast compression, good ratio

## File Type Handling

OpenArc automatically detects and routes files:

| File Type | Codec | Notes |
|-----------|-------|-------|
| JPG, JPEG | BPG | Better compression than JPEG |
| PNG | BPG | Lossless or lossy compression |
| TIFF, BMP | BPG | Efficient compression |
| RAW (CR2, NEF, ARW) | BPG | Use lossless mode |
| MP4, MOV | FFmpeg | Re-encode with chosen preset |
| AVI, MKV | FFmpeg | Convert to MP4 |
| Documents, Archives | ArcMax/Zstd | General compression |

## Example Use Cases

### Scenario 1: Vacation Photos
```
Input: 500 JPG photos (2GB)
Settings: Quality 23, ArcMax
Expected: ~1GB archive (50% reduction)
Time: ~5-10 minutes
```

### Scenario 2: Phone Videos
```
Input: 20 MP4 videos (5GB)
Settings: Phone preset, CRF 23
Expected: ~2.5GB archive (50% reduction)
Time: ~10-20 minutes (depends on hardware)
```

### Scenario 3: Mixed Content
```
Input: Photos + Videos + Documents
Settings: Default settings
Expected: Automatic routing to best codec
Time: Varies by content
```

## Tips & Tricks

### Maximize Compression
- Use lossless mode for RAW files only
- For JPGs, quality 28-30 is often imperceptible
- Videos: CRF 23-25 is sweet spot for most content
- Use ArcMax for best compression ratio

### Maximize Speed
- Use Zstandard for archive compression
- Use Fast preset for videos
- Higher quality numbers = faster compression
- Process fewer files at once

### Preserve Quality
- Use lossless mode for important images
- Use Quality preset for videos
- Lower CRF values (18-20)
- Test settings on sample files first

## Troubleshooting

### Application Won't Start
- Ensure `reduced_lege_ffi.dll` is in DocBrakeGUI folder
- Check that all dependencies are built
- Run from command line to see error messages

### Compression Fails
- Check disk space (need ~2x input size free)
- Verify file permissions
- Check for corrupted input files
- Review error message in status area

### Slow Compression
- Videos are CPU-intensive (normal)
- Close other applications
- Use Fast preset for quicker results
- Consider hardware acceleration (future feature)

### Poor Quality Results
- Lower quality/CRF numbers for better quality
- Use lossless mode for critical images
- Test settings on sample files first
- Some formats don't compress well (already compressed)

## Advanced Usage

### Command Line (Future)
```powershell
# CLI tool coming soon
openarc create -o archive.oarc -q 23 input_folder/
openarc extract -i archive.oarc -o output_folder/
```

### Batch Processing
- Add multiple folders at once
- All files processed automatically
- Progress shown for each file

### Configuration Persistence
- Settings saved automatically
- Stored in: `%APPDATA%\openarc\config.json`
- Edit manually for advanced options

## Getting Help

### Documentation
- `README.md`: General overview
- `WORKSPACE_ARCHITECTURE.md`: Technical details
- `BUILD_GUIDE.md`: Build instructions

### Common Issues
1. Build errors → See BUILD_GUIDE.md
2. Runtime errors → Check logs in console
3. Quality issues → Adjust settings and test

### Reporting Bugs
Include:
- OpenArc version
- Input file types and sizes
- Settings used
- Error messages
- System information (Windows version, RAM, CPU)

## What's Next?

After getting comfortable with basic usage:
1. Experiment with different settings
2. Compare compression ratios
3. Test with your actual media files
4. Provide feedback for improvements

## Future Features

Planned enhancements:
- Hardware acceleration (NVENC, QSV)
- Incremental backups
- Cloud storage integration
- Metadata preservation (EXIF, GPS)
- Deduplication
- Multi-threaded compression
- Archive encryption
