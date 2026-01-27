# BPG Viewer and Thumbnail Library

A standalone, FFI-capable library for viewing BPG (Better Portable Graphics) images and generating thumbnails. This library provides both Rust and C APIs for easy integration into any project.

## Features

- **BPG Decoding**: Decode BPG files to raw image data
- **Format Conversion**: Convert between various image formats (RGB, RGBA, BGR, BGRA, YCbCr)
- **Thumbnail Generation**: Create thumbnails from BPG images with configurable dimensions
- **FFI Support**: C-compatible API for embedding in other languages
- **Command-Line Tools**: Standalone viewer and thumbnail generator applications
- **Memory Safe**: Rust implementation with proper memory management

## Project Structure

```
bpg-viewer/
├── src/
│   ├── lib.rs          # Main library with C FFI exports
│   ├── ffi.rs          # Low-level BPG FFI bindings
│   ├── decoder.rs      # Image decoding module
│   ├── encoder.rs      # Image encoding module
│   ├── thumbnail.rs    # Thumbnail generation
│   └── bin/
│       ├── viewer.rs   # CLI viewer application
│       └── thumbnail.rs # CLI thumbnail generator
├── include/
│   └── bpg_viewer.h    # C header file for FFI
├── build.rs            # Build script for linking
└── Cargo.toml          # Rust project manifest
```

## Building

### Prerequisites

- Rust toolchain (1.70+)
- BPG library (libbpg)
- x265 encoder library
- libpng, libjpeg

### Build Commands

```bash
# Build the library
cargo build --release

# Build with specific BPG library path
BPG_LIB_PATH=/path/to/libbpg cargo build --release

# Build command-line tools
cargo build --release --bins
```

### Output Artifacts

- **Library**: `target/release/libbpg_viewer.a` (static), `target/release/bpg_viewer.dll/so` (dynamic)
- **CLI Tools**:
  - `target/release/bpg-view` - Image viewer
  - `target/release/bpg-thumb` - Thumbnail generator

## Usage

### Rust API

```rust
use bpg_viewer::{decode_file, ThumbnailGenerator};
use std::path::Path;

// Decode a BPG file
let decoded = decode_file("image.bpg")?;
println!("Image: {}x{}", decoded.width, decoded.height);

// Convert to RGBA32
let rgba_data = decoded.to_rgba32()?;

// Generate thumbnail
let generator = ThumbnailGenerator::with_dimensions(256, 256);
generator.generate_thumbnail_to_png(
    Path::new("image.bpg"),
    Path::new("thumb.png")
)?;
```

### C API

```c
#include "bpg_viewer.h"

// Decode image
BPGImageHandle* img = bpg_viewer_decode_file("image.bpg");
if (!img) {
    fprintf(stderr, "Failed to decode image\n");
    return 1;
}

// Get dimensions
uint32_t width, height;
bpg_viewer_get_dimensions(img, &width, &height);
printf("Image: %ux%u\n", width, height);

// Get RGBA data
uint8_t* rgba_data;
size_t rgba_size;
bpg_viewer_get_rgba32(img, &rgba_data, &rgba_size);

// Use the data...

// Cleanup
bpg_viewer_free_buffer(rgba_data, rgba_size);
bpg_viewer_free_image(img);

// Generate thumbnail
BPGThumbnailHandle* thumb = bpg_thumbnail_create_with_size(512, 512);
bpg_thumbnail_generate_png(thumb, "image.bpg", "thumb.png");
bpg_thumbnail_free(thumb);
```

### Command-Line Tools

#### BPG Viewer

```bash
# View image information
bpg-view image.bpg --info

# Decode and convert to PNG
bpg-view image.bpg
```

#### Thumbnail Generator

```bash
# Generate thumbnail with default settings (256x256)
bpg-thumb image.bpg

# Custom dimensions
bpg-thumb -w 512 -h 512 image.bpg -o thumb.png

# With quality setting
bpg-thumb -w 256 -h 256 -q 25 image.bpg
```

## FFI Integration

This library can be integrated into other languages using FFI:

### Python (via ctypes)

```python
from ctypes import *

lib = CDLL("./target/release/libbpg_viewer.so")

# Decode image
lib.bpg_viewer_decode_file.restype = c_void_p
img = lib.bpg_viewer_decode_file(b"image.bpg")

# Get dimensions
width = c_uint32()
height = c_uint32()
lib.bpg_viewer_get_dimensions(img, byref(width), byref(height))
print(f"Image: {width.value}x{height.value}")

# Cleanup
lib.bpg_viewer_free_image(img)
```

### C#/.NET (via P/Invoke)

```csharp
[DllImport("bpg_viewer.dll")]
private static extern IntPtr bpg_viewer_decode_file(string path);

[DllImport("bpg_viewer.dll")]
private static extern int bpg_viewer_get_dimensions(
    IntPtr handle, out uint width, out uint height);

[DllImport("bpg_viewer.dll")]
private static extern void bpg_viewer_free_image(IntPtr handle);

// Usage
var img = bpg_viewer_decode_file("image.bpg");
bpg_viewer_get_dimensions(img, out uint width, out uint height);
Console.WriteLine($"Image: {width}x{height}");
bpg_viewer_free_image(img);
```

## Library Configuration

The library can be configured at build time using environment variables:

- `BPG_LIB_PATH`: Path to the BPG library directory
- `CARGO_BUILD_TARGET`: Target platform (e.g., x86_64-pc-windows-msvc)

## Performance Considerations

- Direct FFI calls eliminate subprocess overhead
- Native decoding is significantly faster than external tools
- Efficient image scaling algorithms using the `image` crate
- Zero-copy operations where possible

## Memory Management

- All C-allocated buffers are properly freed using RAII in Rust
- FFI functions return owned data that must be freed by the caller
- Use `bpg_viewer_free_buffer()` for buffers allocated by the library

## Testing

```bash
# Run unit tests
cargo test

# Run with output
cargo test -- --nocapture

# Test specific module
cargo test decoder
```

## License

This library interfaces with libbpg, which is licensed under LGPL/BSD.
Check the original BPG library license for distribution requirements.

## Implementation Notes

Based on the BPG implementation from the OpenARC project, this standalone
library provides a clean, reusable interface for BPG operations that can
be embedded in any application via FFI.

### Differences from Subprocess Approach

1. **No subprocess overhead** - Direct library calls
2. **Better memory management** - Controlled allocation/deallocation
3. **Type safety** - Rust wrapper provides safety guarantees
4. **Better error handling** - Detailed error reporting
5. **Platform integration** - Native library linking

## Future Enhancements

- [ ] Animated BPG support
- [ ] GPU-accelerated decoding
- [ ] Batch processing utilities
- [ ] Web assembly bindings
- [ ] Additional output formats (WebP, AVIF)
