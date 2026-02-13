# OpenArc CLI Guide

## Quick Start

### Interactive Mode (Recommended)

Simply run `openarc` with no arguments to launch the interactive wizard:

```bash
openarc
```

The wizard will guide you through:
1. **Input Selection** - Drag-and-drop or paste file/folder paths
2. **Compression Settings** - Configure image (BPG) and video (H.264/H.265) quality
3. **Processing Mode** - Choose between encode-only or encode+archive
4. **Output Location** - Select where to save results

**Just 4 Enter presses** with default settings to start processing!

### Interactive Mode Features

✨ **Drag-and-Drop Support**
- Paste multiple file or folder paths (one per line or space-separated)
- Supports quoted paths with spaces
- Automatically discovers all media files in folders recursively

✨ **Smart Defaults**
- Images: BPG quality 28, 8-bit depth
- Videos: H.264, CRF 23, medium speed
- Archive: Level 22 compression, catalog enabled, deduplication enabled

✨ **Two Processing Modes**
1. **Encode Only** - Compress files, save to output folder
2. **Encode + Archive** (Recommended) - Compress AND create .oarc archive

✨ **Clear Progress Reporting**
- Real-time progress bars
- File-by-file status updates
- Compression statistics at the end

## Command Line Mode

For advanced users or scripting, use the CLI commands directly:

### Create Archive

```bash
openarc create -o output.oarc /path/to/photos /path/to/videos
```

With custom settings:

```bash
openarc create \
  -o my-archive.oarc \
  --bpg-quality 25 \
  --video-preset 1 \
  --video-crf 20 \
  --compression-level 18 \
  ~/Pictures ~/Videos
```

### Options

#### Image Compression (BPG)
- `--bpg-quality <0-51>` - Quality (lower = better, higher compression). Default: 28
- `--bpg-lossless` - Enable lossless mode (ignores quality setting)

#### Video Compression
- `--video-preset <0-3>` - Preset selection:
  - `0` = H.264, Medium (default)
  - `1` = H.265, Medium
  - `2` = H.264, Fast
  - `3` = H.265, Slow
- `--video-crf <0-51>` - Quality (lower = better). Default: 23

#### Archive Settings
- `--compression-level <1-22>` - ZSTD compression (higher = better). Default: 22
- `--no-catalog` - Disable catalog (incremental backup tracking)
- `--no-dedup` - Disable deduplication
- `--no-skip-compressed` - Re-encode already compressed videos

### Additional Commands

```bash
# Extract archive
openarc extract -i archive.oarc -o output_folder

# List archive contents
openarc list archive.oarc

# Launch interactive mode explicitly
openarc interactive
```

## Supported Formats

### Input Formats

**Images:**
- JPEG, PNG, HEIC/HEIF, TIFF, BMP, WebP
- RAW: DNG, CR2 (Canon), NEF (Nikon), ARW (Sony), ORF (Olympus), RW2, RAF
- JPEG2000 (JP2, J2K)
- BPG (will be re-encoded if settings differ)

**Videos:**
- MP4, MOV, AVI, MKV, WebM, M4V, WMV

### Output Formats

- **Images** → BPG (Better Portable Graphics)
  - 90%+ size reduction with no perceptible quality loss
  - Superior to HEIC format

- **Videos** → MP4 (H.264 or H.265)
  - Smart detection of already-compressed videos
  - Preserves metadata

- **Archive** → .oarc (ArcMax format with FreeArc codecs)
  - High compression with deduplication
  - SQLite-based catalog for incremental backups

## Examples

### Interactive Mode with Drag-and-Drop

```
$ openarc

╔════════════════════════════════════════╗
║   OpenArc - Media Archival Wizard     ║
╚════════════════════════════════════════╝

Step 1/4: Input Files & Folders
Drag-and-drop or paste paths below:
> ~/Pictures/Vacation
> ~/DCIM/Camera
> [Press Enter]

✓ Found 1,234 media files

Step 2/4: Compression Settings
Use defaults? (Y/n): [Press Enter]
✓ Using default settings

Step 3/4: Processing Mode
[1] Encode Only
[2] Encode + Archive (recommended)
Choice [2]: [Press Enter]

Step 4/4: Output Location
Archive output file: openarc_archive.oarc
Path: [Press Enter]

Press Enter to start processing...
```

### CLI Mode for Automation

```bash
# Backup phone photos monthly
openarc create \
  -o "backup_$(date +%Y%m).oarc" \
  --bpg-quality 28 \
  --video-preset 1 \
  ~/phone_backup/DCIM

# High quality archive for important photos
openarc create \
  -o wedding_photos.oarc \
  --bpg-quality 20 \
  --bpg-lossless \
  --compression-level 22 \
  ~/Pictures/Wedding
```

### Scripting Example

```bash
#!/bin/bash
# Automated monthly backup script

BACKUP_DIR="/mnt/backup"
SOURCE_DIRS=(
  ~/Pictures
  ~/Videos
  /media/sdcard/DCIM
)

ARCHIVE_NAME="monthly_backup_$(date +%Y%m%d).oarc"

openarc create \
  -o "$BACKUP_DIR/$ARCHIVE_NAME" \
  --bpg-quality 28 \
  --video-preset 1 \
  --compression-level 22 \
  "${SOURCE_DIRS[@]}"

echo "Backup complete: $ARCHIVE_NAME"
```

## Multi-OS Support

OpenArc CLI works across platforms:

- ✅ **Windows** - Full support with drag-and-drop
- ✅ **Linux** - Full support with terminal path pasting
- ✅ **macOS** - Full support with Finder drag-and-drop

### Platform Notes

**Windows:**
- Drag files from Explorer directly into Command Prompt/PowerShell
- Supports Windows paths (e.g., `C:\Users\...`)

**Linux/macOS:**
- Paste paths from file manager
- Tab completion works for path entry
- Supports `~` expansion for home directory

## Tips & Tricks

### Fast Workflow (4 Enters)

1. Launch: `openarc` [Enter]
2. Paste paths [Enter] [Enter] (twice to finish input)
3. Accept defaults [Enter]
4. Accept mode [Enter]
5. Accept output [Enter]
6. Start processing [Enter]

### Quality vs. Size Trade-offs

**Maximum Compression (smallest files):**
```bash
--bpg-quality 35 --video-preset 3 --compression-level 22
```

**Maximum Quality (larger files):**
```bash
--bpg-quality 18 --bpg-lossless --video-preset 3 --video-crf 18
```

**Balanced (recommended default):**
```bash
--bpg-quality 28 --video-preset 1 --compression-level 22
```

### Deduplication Benefits

Enable catalog and dedup for incremental backups:
- Skips files already in catalog (by hash)
- Automatically detects duplicates
- Massive space savings for photo collections with duplicates

## Troubleshooting

### "No media files found"
- Check file extensions are supported (see Supported Formats)
- Verify paths are correct
- Ensure you have read permissions

### "Permission denied"
- Run with appropriate permissions for input/output paths
- Check output directory is writable

### Slow processing
- Lower compression level (e.g., `--compression-level 10`)
- Use faster video preset (`--video-preset 2`)
- Disable deduplication for first-time processing (`--no-dedup`)

## Getting Help

```bash
# General help
openarc --help

# Command-specific help
openarc create --help
openarc extract --help

# Version info
openarc --version
```

For more information, visit: https://github.com/LegeApp/OpenArc
