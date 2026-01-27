#!/bin/bash
# Build script for BPG Viewer GUI (Linux/macOS)
# Note: GUI is Windows-only (.NET WPF), but Rust library can be built

set -e

CLEAN=false
RELEASE=true

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --clean)
            CLEAN=true
            shift
            ;;
        --debug)
            RELEASE=false
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "BPG Viewer GUI Build Script"
echo "============================"
echo ""

# Clean if requested
if [ "$CLEAN" = true ]; then
    echo "Cleaning build artifacts..."
    cargo clean
    echo "Clean complete."
    echo ""
fi

# Build Rust library
echo "[1/2] Building Rust library (libbpg_viewer.so)..."

if [ "$RELEASE" = true ]; then
    BUILD_TYPE="release"
    BUILD_FLAG="--release"
else
    BUILD_TYPE="debug"
    BUILD_FLAG=""
fi

echo "  Running: cargo build $BUILD_FLAG --lib"
cargo build $BUILD_FLAG --lib

if [ ! -f "target/$BUILD_TYPE/libbpg_viewer.so" ] && [ ! -f "target/$BUILD_TYPE/libbpg_viewer.dylib" ]; then
    echo "ERROR: Shared library not found!"
    exit 1
fi

echo "  ✓ Rust library built: target/$BUILD_TYPE/libbpg_viewer.*"
echo ""

# Note about C# GUI
echo "[2/2] C# GUI Build (Windows Only)"
echo "  Note: The WPF GUI requires Windows and .NET 8.0"
echo "  On Linux/macOS, use the Rust egui version instead:"
echo "    cargo build --release --features gui --bin bpg-gui"
echo ""

echo "============================"
echo "Rust library build complete!"
echo ""
