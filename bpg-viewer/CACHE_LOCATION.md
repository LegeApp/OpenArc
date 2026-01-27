# BPG Viewer Thumbnail Cache Location

## Cache Directory

The BPG Viewer stores thumbnail cache in the Windows user AppData folder for better data persistence and organization.

### Location

```
C:\Users\<username>\AppData\Local\BpgViewer\Cache\Thumbnails\
```

### Path Construction

```csharp
string appDataPath = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
string cacheDirectory = Path.Combine(
    appDataPath,
    "BpgViewer",
    "Cache",
    "Thumbnails");
```

## Why AppData\Local?

### Benefits

1. **Persistence**: Data survives temporary file cleanups
2. **User-specific**: Each Windows user has their own cache
3. **Standard Location**: Follows Windows application data guidelines
4. **Organized**: Clear application folder structure
5. **Accessible**: Easy to find and manage by users

### Previous Location (Changed)

Previously used: `%TEMP%\BpgViewerCache\thumbnails\`

**Issues with temp directory:**
- Could be cleared by disk cleanup tools
- Less organized (mixed with system temp files)
- May not persist across reboots on some systems

## Cache Structure

```
C:\Users\YourName\AppData\Local\
└── BpgViewer\
    └── Cache\
        └── Thumbnails\
            ├── A1B2C3D4.png  (cached thumbnail)
            ├── E5F6G7H8.png
            ├── I9J0K1L2.png
            └── ...
```

### Cache File Naming

Each cached thumbnail is named using a hash:
```csharp
string cacheKey = $"{FilePath}_{LastModified.Ticks}".GetHashCode().ToString("X8");
string cacheFile = $"{cacheKey}.png";
```

**Example:**
- Original file: `C:\Photos\vacation.bpg`
- Last modified: `638421234567890123` (ticks)
- Hash: `A1B2C3D4`
- Cache file: `A1B2C3D4.png`

### Cache Invalidation

Thumbnails are automatically invalidated when:
- Source file modification time changes
- Source file is moved/renamed
- Cache is manually cleared via UI

## Cache Management

### View Cache Size

The cache size is displayed in:
1. **Info Panel** (Catalog mode)
2. **Clear Cache button tooltip**

### Clear Cache

**Via UI:**
- Catalog toolbar: Click "🗑️ Clear Cache" button
- Menu: (future enhancement)

**Manually:**
1. Close BPG Viewer
2. Delete folder: `C:\Users\<username>\AppData\Local\BpgViewer\Cache\Thumbnails\`

### Cache Statistics

The `ThumbnailCacheService` provides:
- `GetCacheSize()`: Returns total cache size in bytes
- `CacheSizeFormatted`: Human-readable size (KB/MB)

Example output: `"5.2 MB"`, `"127.3 KB"`, `"42 B"`

## Performance

### Cache Hit vs Miss

| Operation | Time | Notes |
|-----------|------|-------|
| **Cache Hit** | ~5-10ms | Load PNG from disk |
| **Cache Miss** | ~50-200ms | Decode BPG + resize + save PNG |
| **First Load** | ~150-300ms | Full BPG decode + thumbnail generation |

### Concurrency

- Maximum **4 concurrent** thumbnail generations
- Prevents CPU/disk overload
- Controlled by `SemaphoreSlim` in `ThumbnailCacheService`

### Cache Benefits

For a folder with 100 BPG images:
- **First load**: ~15-30 seconds (generating all thumbnails)
- **Subsequent loads**: ~1-2 seconds (loading from cache)
- **After file modification**: Only changed files regenerate

## Implementation Details

### Service Class

**File:** `BpgViewerGUI/Services/ThumbnailCacheService.cs`

**Constructor:**
```csharp
public ThumbnailCacheService(int thumbnailWidth = 256, int thumbnailHeight = 256, int maxConcurrency = 4)
{
    _thumbnailWidth = thumbnailWidth;
    _thumbnailHeight = thumbnailHeight;
    _semaphore = new SemaphoreSlim(maxConcurrency);

    // Create cache directory in AppData\Local
    string appDataPath = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
    _cacheDirectory = Path.Combine(
        appDataPath,
        "BpgViewer",
        "Cache",
        "Thumbnails");

    Directory.CreateDirectory(_cacheDirectory);

    // Create thumbnail generator handle (FFI)
    _thumbnailHandle = BpgViewerFFI.bpg_thumbnail_create_with_size(
        (uint)thumbnailWidth,
        (uint)thumbnailHeight);
}
```

### Cache Check Logic

```csharp
public async Task<bool> LoadThumbnailAsync(ThumbnailItem item, CancellationToken cancellationToken)
{
    string cachePath = item.GetCachePath(_cacheDirectory);

    // Check if cached thumbnail exists
    if (File.Exists(cachePath))
    {
        return await LoadFromCacheAsync(item, cachePath, cancellationToken);
    }

    // Generate new thumbnail
    return await GenerateThumbnailAsync(item, cachePath, cancellationToken);
}
```

## Integration with OpenARC

When merging into OpenARC, the cache structure will be:

```
C:\Users\YourName\AppData\Local\
└── OpenARC\                    (or keep BpgViewer subfolder)
    ├── Database\
    │   └── archives.db
    └── Cache\
        ├── Thumbnails\
        │   └── *.png
        └── BPG\
            └── *.png
```

Shared cache service can be used across OpenARC modules.

## Troubleshooting

### Cache Not Working

**Symptom:** Thumbnails regenerate every time

**Possible causes:**
1. File modification time changing
2. Cache directory permissions issue
3. Disk full

**Solution:**
1. Check file properties (modification date)
2. Verify write permissions to AppData\Local
3. Clear cache and try again

### Cache Taking Too Much Space

**Check size:**
```csharp
ThumbnailCacheService service = ...;
long sizeBytes = service.GetCacheSize();
```

**Reduce if needed:**
- Clear cache via UI
- Or delete old cache files manually

### Can't Find Cache Folder

**Quick access in Windows:**
1. Press `Win+R`
2. Type: `%LOCALAPPDATA%\BpgViewer\Cache\Thumbnails`
3. Press Enter

**Or via Explorer:**
1. Open File Explorer
2. Navigate to: `C:\Users\<YourName>\AppData\Local\BpgViewer\Cache\Thumbnails`

## Future Enhancements

Potential improvements:
- [ ] Cache size limit (e.g., max 500MB)
- [ ] LRU eviction policy
- [ ] Cache statistics dashboard
- [ ] Background cache preloading
- [ ] Smart prefetching based on usage patterns
- [ ] Compressed cache storage
- [ ] Settings for cache location customization

## Summary

The thumbnail cache now uses the standard Windows AppData\Local folder, providing:
- Better data persistence
- Proper organization
- User-specific storage
- Alignment with Windows best practices

**Default location:** `%LOCALAPPDATA%\BpgViewer\Cache\Thumbnails\`
