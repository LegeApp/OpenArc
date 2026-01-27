# OpenArc GUI - Quick Start

## 🚀 Quick Workflow

### 1. Launch Application
```
DocBrakeGUI.exe
```

### 2. Set Output Path (REQUIRED)
1. Click **⚙️ Settings** button
2. Click **Browse...** next to "Archive Path"
3. Choose location and filename (e.g., `my-archive.oarc`)
4. Verify green "Archive:" indicator appears in bottom bar

### 3. Add Files
- **Drag & Drop**: Drag files/folders onto window
- **Browse**: Click **📁 Add Files** or **📂 Add Folder**

### 4. Process
Click **Process** button → Wait for completion → Done!

### 5. Archive Operations (Optional)
- **List**: Click **List** button → Select archive → View contents
- **Extract**: Click **Extract** button → Select archive → Choose output folder

---

## ⚙️ Settings Reference

### Image Compression (BPG)
| Setting | Range | Default | Description |
|---------|-------|---------|-------------|
| Quality | 0-51 | 25 | Lower = better quality, larger files |
| Lossless | On/Off | Off | Perfect quality, much larger |

**Recommendations:**
- Photos: Quality 20-30
- Screenshots: Lossless or Quality 15-20

### Video Compression (FFmpeg)
| Preset | Codec | Speed | Use Case |
|--------|-------|-------|----------|
| Phone | H264 | Medium | Phone videos |
| Camera | H265 | Medium | Camera footage (best default) |
| Fast | H264 | Fast | Quick encoding |
| Quality | H265 | Slow | Maximum quality |

| Setting | Range | Default | Description |
|---------|-------|---------|-------------|
| CRF | 0-51 | 23 | Lower = better quality, larger files |

**Recommendations:**
- General use: Camera preset, CRF 23
- High quality: Quality preset, CRF 18-20
- Fast processing: Fast preset, CRF 23-28

### Archive Compression
| Method | Speed | Compression | Notes |
|--------|-------|-------------|-------|
| ArcMax | Slower | Better | FreeARC-based |
| Zstd | Faster | Good | Recommended default |

| Setting | Range | Default | Description |
|---------|-------|---------|-------------|
| Level | 1-22 | 3 | Higher = better compression, slower |

**Recommendations:**
- Fast: Level 1-3
- Balanced: Level 3-6 (default)
- Maximum: Level 10-15

### Backup Features
| Feature | Default | Description |
|---------|---------|-------------|
| Enable catalog | ✅ On | Track files for incremental backups |
| Enable deduplication | ✅ On | Skip duplicate files |
| Skip compressed videos | ✅ On | Don't re-encode efficient videos |

---

## 📊 Expected Results

### Compression Ratios
- **JPEG photos**: 50-70% of original size
- **PNG images**: 30-60% of original size
- **Phone videos (H264)**: 40-60% (if re-encoded)
- **Camera videos (various)**: 30-70% depending on source

### Processing Speed (approximate)
- **Images**: 1-5 seconds each
- **Videos**: 5-120 seconds each (depends on length and settings)

---

## ⚠️ Important Notes

1. **Output path is REQUIRED** - Set it first in Settings
2. **FFI DLL required** - `openarc_ffi.dll` must be in app directory
3. **Supported formats**:
   - Images: JPG, PNG, BMP, TIFF, RAW, CR2, NEF, ARW
   - Videos: MP4, MOV, AVI, MKV, WEBM
4. **Archive format**: `.oarc` files are TAR+ZSTD archives

---

## 🐛 Troubleshooting

| Issue | Solution |
|-------|----------|
| DLL not found | Build `openarc-ffi` and copy DLL to app directory |
| No output path warning | Open Settings → Browse for archive path |
| Files not added | Check file extensions are supported |
| Processing fails | Check `startup.log` for errors |

---

## 📁 Output File Structure

Your `.oarc` archive contains:
```
archive.oarc (TAR+ZSTD compressed)
├── image001.bpg          (compressed images)
├── image002.bpg
├── video001.mp4          (compressed videos)
├── video002.mp4
├── CATALOG.json          (if catalog enabled)
└── HASHES.sha256         (integrity verification)
```

---

## 🎯 Modes

### Phone Mode
- Optimized for phone media backup
- Set phone source path in Settings
- Auto-detection (planned feature)

### Standard Mode
- Manual file/folder selection
- Full control over what to archive
- Recommended for general use

---

For detailed testing instructions, see `TESTING_GUIDE.md`
