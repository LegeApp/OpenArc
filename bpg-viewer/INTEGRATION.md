# BPG Viewer Library Integration Guide

This guide explains how to integrate the BPG Viewer library into various projects and languages.

## Table of Contents

1. [Rust Integration](#rust-integration)
2. [C/C++ Integration](#cc-integration)
3. [Python Integration](#python-integration)
4. [C#/.NET Integration](#cnet-integration)
5. [Building as Static Library](#building-as-static-library)
6. [Building as Dynamic Library](#building-as-dynamic-library)

## Rust Integration

### As a Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
bpg-viewer = { path = "../bpg-viewer" }
```

Or specify the git repository:

```toml
[dependencies]
bpg-viewer = { git = "https://github.com/yourorg/bpg-viewer" }
```

### Usage Example

```rust
use bpg_viewer::{decode_file, ThumbnailGenerator};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Decode image
    let img = decode_file("image.bpg")?;
    println!("Image: {}x{}", img.width, img.height);

    // Generate thumbnail
    let thumbnailer = ThumbnailGenerator::with_dimensions(512, 512);
    thumbnailer.generate_thumbnail_to_png(
        Path::new("image.bpg"),
        Path::new("thumb.png")
    )?;

    Ok(())
}
```

## C/C++ Integration

### Static Library Approach

1. Build the static library:

```bash
cargo build --release
```

2. Copy files to your project:
   - `target/release/libbpg_viewer.a` (or `.lib` on Windows)
   - `include/bpg_viewer.h`

3. Link in your project:

```bash
gcc -o myapp main.c -L./lib -lbpg_viewer -lm -lpthread -ldl
```

### CMake Integration

Create `FindBPGViewer.cmake`:

```cmake
find_path(BPG_VIEWER_INCLUDE_DIR bpg_viewer.h
    HINTS ${BPG_VIEWER_ROOT}/include)

find_library(BPG_VIEWER_LIBRARY bpg_viewer
    HINTS ${BPG_VIEWER_ROOT}/lib)

include(FindPackageHandleStandardArgs)
find_package_handle_standard_args(BPGViewer DEFAULT_MSG
    BPG_VIEWER_LIBRARY BPG_VIEWER_INCLUDE_DIR)

if(BPG_VIEWER_FOUND)
    set(BPG_VIEWER_LIBRARIES ${BPG_VIEWER_LIBRARY})
    set(BPG_VIEWER_INCLUDE_DIRS ${BPG_VIEWER_INCLUDE_DIR})
endif()
```

In your `CMakeLists.txt`:

```cmake
find_package(BPGViewer REQUIRED)
include_directories(${BPG_VIEWER_INCLUDE_DIRS})
target_link_libraries(myapp ${BPG_VIEWER_LIBRARIES})
```

### C++ Example

```cpp
#include "bpg_viewer.h"
#include <iostream>

int main() {
    auto img = bpg_viewer_decode_file("image.bpg");
    if (!img) {
        std::cerr << "Failed to decode image\n";
        return 1;
    }

    uint32_t width, height;
    bpg_viewer_get_dimensions(img, &width, &height);
    std::cout << "Image: " << width << "x" << height << "\n";

    bpg_viewer_free_image(img);
    return 0;
}
```

## Python Integration

### Using ctypes

```python
from ctypes import *
import os

class BPGViewer:
    def __init__(self, lib_path="./target/release/libbpg_viewer.so"):
        self.lib = CDLL(lib_path)
        self._setup_functions()

    def _setup_functions(self):
        # Decode file
        self.lib.bpg_viewer_decode_file.argtypes = [c_char_p]
        self.lib.bpg_viewer_decode_file.restype = c_void_p

        # Get dimensions
        self.lib.bpg_viewer_get_dimensions.argtypes = [
            c_void_p, POINTER(c_uint32), POINTER(c_uint32)
        ]
        self.lib.bpg_viewer_get_dimensions.restype = c_int

        # Get RGBA data
        self.lib.bpg_viewer_get_rgba32.argtypes = [
            c_void_p, POINTER(POINTER(c_uint8)), POINTER(c_size_t)
        ]
        self.lib.bpg_viewer_get_rgba32.restype = c_int

        # Free functions
        self.lib.bpg_viewer_free_buffer.argtypes = [c_void_p, c_size_t]
        self.lib.bpg_viewer_free_image.argtypes = [c_void_p]

    def decode_file(self, path):
        img = self.lib.bpg_viewer_decode_file(path.encode('utf-8'))
        if not img:
            raise Exception("Failed to decode image")

        width = c_uint32()
        height = c_uint32()
        self.lib.bpg_viewer_get_dimensions(img, byref(width), byref(height))

        data_ptr = POINTER(c_uint8)()
        data_size = c_size_t()
        self.lib.bpg_viewer_get_rgba32(img, byref(data_ptr), byref(data_size))

        # Copy data to Python bytes
        data = bytes(data_ptr[:data_size.value])

        # Cleanup
        self.lib.bpg_viewer_free_buffer(data_ptr, data_size)
        self.lib.bpg_viewer_free_image(img)

        return {
            'width': width.value,
            'height': height.value,
            'data': data
        }

# Usage
viewer = BPGViewer()
img = viewer.decode_file("image.bpg")
print(f"Image: {img['width']}x{img['height']}")
```

### Using cffi

```python
from cffi import FFI

ffi = FFI()
ffi.cdef("""
    typedef struct BPGImageHandle BPGImageHandle;

    BPGImageHandle* bpg_viewer_decode_file(const char* path);
    int bpg_viewer_get_dimensions(const BPGImageHandle* handle,
                                   uint32_t* width, uint32_t* height);
    void bpg_viewer_free_image(BPGImageHandle* handle);
""")

lib = ffi.dlopen("./target/release/libbpg_viewer.so")

img = lib.bpg_viewer_decode_file(b"image.bpg")
width = ffi.new("uint32_t*")
height = ffi.new("uint32_t*")
lib.bpg_viewer_get_dimensions(img, width, height)
print(f"Image: {width[0]}x{height[0]}")
lib.bpg_viewer_free_image(img)
```

## C#/.NET Integration

### P/Invoke Wrapper

```csharp
using System;
using System.Runtime.InteropServices;

namespace BPGViewer
{
    public class BPGImage : IDisposable
    {
        private IntPtr handle;

        [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr bpg_viewer_decode_file(
            [MarshalAs(UnmanagedType.LPStr)] string path);

        [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern int bpg_viewer_get_dimensions(
            IntPtr handle, out uint width, out uint height);

        [DllImport("bpg_viewer.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern void bpg_viewer_free_image(IntPtr handle);

        public uint Width { get; private set; }
        public uint Height { get; private set; }

        public BPGImage(string path)
        {
            handle = bpg_viewer_decode_file(path);
            if (handle == IntPtr.Zero)
                throw new Exception("Failed to decode image");

            bpg_viewer_get_dimensions(handle, out uint w, out uint h);
            Width = w;
            Height = h;
        }

        public void Dispose()
        {
            if (handle != IntPtr.Zero)
            {
                bpg_viewer_free_image(handle);
                handle = IntPtr.Zero;
            }
        }
    }

    // Usage
    class Program
    {
        static void Main()
        {
            using (var img = new BPGImage("image.bpg"))
            {
                Console.WriteLine($"Image: {img.Width}x{img.Height}");
            }
        }
    }
}
```

## Building as Static Library

```bash
# Build static library
cargo build --release

# Output: target/release/libbpg_viewer.a (Unix)
#         target/release/bpg_viewer.lib (Windows)
```

Configure in `Cargo.toml`:

```toml
[lib]
crate-type = ["staticlib"]
```

## Building as Dynamic Library

```bash
# Build dynamic library
cargo build --release

# Output: target/release/libbpg_viewer.so (Linux)
#         target/release/libbpg_viewer.dylib (macOS)
#         target/release/bpg_viewer.dll (Windows)
```

Configure in `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]
```

## Cross-Platform Considerations

### Windows

- Use `.dll` extension
- May need to distribute runtime DLLs (MSVC runtime)
- Use `__declspec(dllexport)` for exports (handled automatically by Rust)

### Linux

- Use `.so` extension
- Set `LD_LIBRARY_PATH` for dynamic loading
- Consider `rpath` for bundled libraries

### macOS

- Use `.dylib` extension
- Set `DYLD_LIBRARY_PATH` for dynamic loading
- Code signing may be required for distribution

## Environment Setup

### Setting Library Path (Unix)

```bash
export LD_LIBRARY_PATH=/path/to/bpg-viewer/target/release:$LD_LIBRARY_PATH
```

### Setting Library Path (Windows)

```cmd
set PATH=%PATH%;C:\path\to\bpg-viewer\target\release
```

## Troubleshooting

### Symbol Not Found

- Ensure all dependencies are linked (pthread, dl, m)
- Check that the library was built for the correct architecture
- Verify the library path is correct

### Runtime Errors

- Check that BPG libraries are available
- Verify input file paths are correct
- Enable debug logging if available

### Build Errors

- Ensure Rust toolchain is up to date
- Check that BPG library path is set correctly
- Verify all dependencies are installed

## Performance Tips

1. **Reuse handles**: Keep decoder/encoder instances across multiple operations
2. **Batch processing**: Process multiple images in parallel
3. **Memory management**: Free buffers promptly to avoid memory leaks
4. **Release builds**: Always use `--release` for production

## Best Practices

1. Always free allocated resources
2. Check return codes from FFI functions
3. Handle null pointers defensively
4. Use RAII patterns where available
5. Test with various image sizes and formats
