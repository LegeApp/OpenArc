# Building FreeArc Codecs with GCC

This document explains how to build the FreeArc codecs using GCC instead of MSVC, which is required for proper compatibility.

## Prerequisites

1. Install GCC (MinGW-w64) on Windows
   - Download from: https://www.mingw-w64.org/downloads/
   - Or use MSYS2: `pacman -S mingw-w64-x86_64-gcc`
   - Ensure GCC and G++ are in your PATH

2. Verify installation:
   ```bash
   gcc --version
   g++ --version
   ar --version
   ```

## Build Process

### Step 1: Build the codecs with GCC

Run the build script appropriate for your platform:

**On Windows:**
```cmd
build_codecs.bat
```

**On Linux/WSL:**
```bash
chmod +x build_codecs.sh
./build_codecs.sh
```

This will:
- Build each codec separately using GCC
- Create static libraries (.a files) in `codec_staging/`
- Build common components (Common.o, CompressionLibrary.o, etc.)
- Create a combined `libfreearc.a` library

### Step 2: Update Cargo to use GCC-built libraries

Replace your `build.rs` with `build_gcc.rs`:
```cmd
copy build_gcc.rs build.rs
```

Or modify your existing `build.rs` to use the codec staging directory.

### Step 3: Build the Rust project

```cmd
cargo build --lib
```

## Directory Structure

After building, you'll have:

```
arcmax/
├── codec_staging/          # GCC-built libraries
│   ├── libfreearc.a       # Combined library
│   ├── liblzma2.a         # Individual codec libraries
│   ├── libppmd.a
│   ├── libtornado.a
│   ├── libgrzip.a
│   ├── liblzp.a
│   ├── libdelta.a
│   ├── libdict.a
│   ├── libmm.a
│   ├── librep.a
│   ├── lib4x4.a
│   └── *.o                # Object files
├── codec_build/           # Temporary build files
└── freearc_cpp_lib/       # Source code
```

## Troubleshooting

### "gcc not found" error
- Ensure MinGW-w64 is installed and in your PATH
- On Windows, add `C:\mingw64\bin` to PATH

### Link errors
- Make sure all codecs were built successfully
- Check that `libfreearc.a` exists in `codec_staging/`

### Compilation fails
- Check that all required headers are available
- Ensure the FreeArc source code is complete

## Customization

### Modifying Build Flags

Edit `build_codecs.bat` (Windows) or `build_codecs.sh` (Linux) to change:
- Optimization levels (`-O2`, `-O3`)
- Debug information (`-g`)
- Warning levels

### Adding New Codecs

1. Add the codec to the build script
2. Include necessary header directories
3. Compile the source files
4. Create a static library

## Notes

- The build process uses static linking to avoid runtime dependencies
- All codecs are built with the same compiler flags for consistency
- The FFI wrapper (`freearc_wrapper_minimal.cpp`) provides the C interface for Rust

## Performance

GCC-built codecs typically provide:
- Better optimization for x86/x64
- Smaller binary size
- Faster compression/decompression speeds

## Integration with Rust

The `build_gcc.rs` script automatically:
- Detects the GCC-built libraries
- Links them properly with the Rust build
- Handles all necessary compiler flags

Just run `cargo build` after building the codecs with GCC.
