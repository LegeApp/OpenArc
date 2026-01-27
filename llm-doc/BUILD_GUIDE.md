# OpenArc Build Guide

## Quick Start

### Prerequisites

1. **Rust Toolchain**
   ```powershell
   # Install from https://rustup.rs/
   rustup default stable
   rustup target add x86_64-pc-windows-msvc
   ```

2. **Visual Studio Build Tools** (for C++ compilation)
   - Install "Desktop development with C++" workload
   - Or install MinGW-w64 for GCC

3. **.NET SDK 8.0+** (for DocBrakeGUI)
   ```powershell
   # Download from https://dotnet.microsoft.com/download
   dotnet --version  # Verify installation
   ```

### Build Steps

#### 1. Build DocBrakeGUI C# Component

```powershell
cd D:\misc\arc\openarc\DocBrakeGUI
dotnet build -c Release
```

This produces `reduced_lege_ffi.dll` needed for FFI integration.

#### 2. Build arcmax (FreeARC) Codecs

```powershell
cd D:\misc\arc\openarc\arcmax

# Build codecs with GCC (if not already built)
.\build_codecs.bat

# This creates codec_staging/libfreearc.a
```

#### 3. Build Entire Workspace

```powershell
cd D:\misc\arc\openarc

# Build all subcrates
cargo build --workspace --release

# Or build just the GUI
cargo build -p openarc-gui --release
```

#### 4. Run the Application

```powershell
# Run from workspace root
cargo run -p openarc-gui --release

# Or run the built executable
.\target\release\openarc-gui.exe
```

## Individual Subcrate Builds

### Build arcmax Only
```powershell
cargo build -p arcmax --release
```

### Build zstd-archive Only
```powershell
cargo build -p zstd-archive --release
```

### Build codecs Only
```powershell
cargo build -p codecs --release
```

## Troubleshooting

### Issue: "GCC-built codecs not found"

**Solution**: Build the arcmax codecs first:
```powershell
cd arcmax
.\build_codecs.bat
```

### Issue: "Cannot find reduced_lege_ffi.dll"

**Solution**: Build the C# component:
```powershell
cd DocBrakeGUI
dotnet build -c Release
```

### Issue: "linker error: cannot find -lfreearc"

**Solution**: Ensure `arcmax/codec_staging/libfreearc.a` exists. Run:
```powershell
cd arcmax
.\build_codecs.bat
```

### Issue: Rust compilation errors

**Solution**: Update Rust toolchain:
```powershell
rustup update stable
cargo clean
cargo build --release
```

## Development Build

For faster iteration during development:

```powershell
# Build without optimizations (faster compile)
cargo build -p openarc-gui

# Run with debug logging
$env:RUST_LOG="debug"
cargo run -p openarc-gui
```

## Testing

```powershell
# Test all subcrates
cargo test --workspace

# Test specific subcrate
cargo test -p zstd-archive
cargo test -p arcmax

# Run with output
cargo test --workspace -- --nocapture
```

## Clean Build

```powershell
# Clean all build artifacts
cargo clean

# Clean specific subcrate
cargo clean -p openarc-gui
```

## Release Build

```powershell
# Build optimized release version
cargo build --workspace --release

# Executable location
.\target\release\openarc-gui.exe
```

## Distribution

To create a distributable package:

1. Build release version
2. Copy required DLLs:
   - `DocBrakeGUI/reduced_lege_ffi.dll`
   - Any FFmpeg/BPG DLLs (if using dynamic linking)
3. Package with installer or ZIP

```powershell
# Create distribution folder
mkdir dist
copy target\release\openarc-gui.exe dist\
copy DocBrakeGUI\reduced_lege_ffi.dll dist\
copy DocBrakeGUI\icon.ico dist\
```

## Build Configuration

### Cargo Features

Currently no optional features, but you can add them:

```toml
[features]
default = ["gui"]
gui = ["eframe", "egui"]
cli = ["clap"]
```

### Environment Variables

- `RUST_LOG`: Set logging level (trace, debug, info, warn, error)
- `CARGO_MANIFEST_DIR`: Automatically set by Cargo
- `OUT_DIR`: Build script output directory

## Platform-Specific Notes

### Windows (Primary Target)
- Uses MSVC or MinGW toolchain
- Requires Visual Studio Build Tools or MinGW-w64
- C# component requires .NET 8.0+

### Future: Linux Support
- Would need to adapt build scripts
- Replace C# component or use Mono
- Adjust FFI integration

### Future: macOS Support
- Similar to Linux considerations
- May need different GUI framework
- FFI adjustments needed

## Performance Optimization

### Link-Time Optimization (LTO)

Add to workspace `Cargo.toml`:
```toml
[profile.release]
lto = true
codegen-units = 1
```

### Strip Symbols

```powershell
cargo build --release
strip target\release\openarc-gui.exe
```

## CI/CD Integration

Example GitHub Actions workflow:

```yaml
name: Build OpenArc

on: [push, pull_request]

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - uses: actions/setup-dotnet@v3
        with:
          dotnet-version: '8.0.x'
      - name: Build C# Component
        run: |
          cd DocBrakeGUI
          dotnet build -c Release
      - name: Build Rust Workspace
        run: cargo build --workspace --release
      - name: Run Tests
        run: cargo test --workspace
```

## Next Steps

After successful build:
1. Test with sample files (images, videos, documents)
2. Verify compression ratios and quality
3. Check performance with large files
4. Test FFI integration with DocBrakeGUI
5. Create user documentation
