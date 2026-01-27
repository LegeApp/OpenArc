# BPG Viewer Build Instructions

## Quick Build

### Option 1: Using PowerShell (Recommended)
```powershell
.\build-and-deploy.ps1
```

### Option 2: Using Batch Script (Double-click friendly)
```batch
build-and-deploy.bat
```

### Option 3: Manual Build
```batch
# 1. Build Rust library
cargo build --release --lib

# 2. Build C# GUI
dotnet build BpgViewerGUI\BpgViewerGUI.csproj -c Release

# 3. Run from output directory
cd BpgViewerGUI\bin\Release\net8.0-windows
.\BpgViewerGUI.exe
```

## Build Scripts

### build-and-deploy.ps1 (PowerShell)
Full-featured build script with colored output and error handling.

**Parameters:**
- `-Release` (default: true) - Build in release mode
- `-TestFolder` (default: "D:\misc\bpg-viewer-test") - Deployment target

**Example:**
```powershell
# Release build (default)
.\build-and-deploy.ps1

# Debug build
.\build-and-deploy.ps1 -Release:$false

# Custom test folder
.\build-and-deploy.ps1 -TestFolder "C:\MyTestFolder"
```

### build-and-deploy.bat (Batch)
Simple batch script for double-click execution. Always builds in release mode.

## Build Process

The build process consists of three steps:

1. **Rust Library Build**
   - Builds the native BPG decoder library (`bpg_viewer.dll`)
   - Location: `target\release\bpg_viewer.dll`
   - Uses libbpg FFI bindings for BPG decoding
   - Includes color space conversion (BT.601/709/2020 → sRGB)

2. **C# GUI Build**
   - Builds the WPF application (`BpgViewerGUI.exe`)
   - Location: `BpgViewerGUI\bin\Release\net8.0-windows\`
   - Automatically includes the Rust DLL via project reference

3. **Deployment**
   - Copies all files to the test folder
   - Includes executable, DLL, and all dependencies

## Requirements

- **Rust**: Latest stable toolchain (for cargo)
- **.NET SDK**: .NET 8.0 or later (for dotnet CLI)
- **Windows**: Required for WPF

## Output

After building, the application is ready to run from:
- **Test Folder**: `D:\misc\bpg-viewer-test\BpgViewerGUI.exe`
- **Build Output**: `BpgViewerGUI\bin\Release\net8.0-windows\BpgViewerGUI.exe`

## Troubleshooting

### Rust Build Fails
- Ensure Rust is installed: `cargo --version`
- Ensure Visual C++ build tools are installed
- Check that libbpg libraries are present in `libs\` folder

### C# Build Fails
- Ensure .NET 8.0 SDK is installed: `dotnet --version`
- Restore packages: `dotnet restore BpgViewerGUI\BpgViewerGUI.csproj`

### DLL Not Found at Runtime
- Verify `bpg_viewer.dll` is in the same folder as `BpgViewerGUI.exe`
- Check that the DLL is the correct architecture (x64)

## Recent Changes

### 2026-01-25: Fixed Single Image Viewer Tiling Issue
- **Problem**: Only top-left tile displayed in single image viewer
- **Cause**: Used temporary `get_rgba32()` workaround instead of proper `decode_to_buffer()`
- **Fix**: Updated `BpgImage.cs` to use `bpg_viewer_decode_to_buffer()` with proper stride handling
- **File**: `BpgViewerGUI\Models\BpgImage.cs` (lines 139-190)
