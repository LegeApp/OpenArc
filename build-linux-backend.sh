#!/usr/bin/env bash
set -euo pipefail

# Linux backend-only build for OpenArc.
# Intentionally skips DocBrakeGUI/WPF and openarc-ffi/MTP.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BPG_DIR="$ROOT_DIR/native/BPG/libbpg-0.9.8"
ARCMAX_DIR="$ROOT_DIR/crates/arcmax"
BUILD_MODE="release"
SKIP_CODECS=0

usage() {
  cat <<USAGE
Usage: ./build-linux-backend.sh [--debug] [--release] [--skip-codecs]

Options:
  --debug         Build debug binaries
  --release       Build release binaries (default)
  --skip-codecs   Skip native codec builds (BPG + ArcMax)
  -h, --help      Show this help
USAGE
}

for arg in "$@"; do
  case "$arg" in
    --debug) BUILD_MODE="debug" ;;
    --release) BUILD_MODE="release" ;;
    --skip-codecs) SKIP_CODECS=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $arg" >&2; usage; exit 1 ;;
  esac
done

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This script is for Linux hosts only." >&2
  exit 1
fi

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing required tool: $1" >&2
    exit 1
  }
}

need_cmd cargo
need_cmd rustc
need_cmd make
need_cmd gcc
need_cmd g++
need_cmd ar
need_cmd ranlib
need_cmd pkg-config

if ! pkg-config --exists x265; then
  echo "Missing x265 development package (pkg-config x265 not found)." >&2
  echo "Install libx265-dev (or distro equivalent) and retry." >&2
  exit 1
fi
if ! pkg-config --exists libpng; then
  echo "Missing libpng development package (pkg-config libpng not found)." >&2
  exit 1
fi
if ! pkg-config --exists libjpeg; then
  echo "Missing libjpeg development package (pkg-config libjpeg not found)." >&2
  exit 1
fi
if ! pkg-config --exists lcms2; then
  echo "Missing lcms2 development package (pkg-config lcms2 not found)." >&2
  exit 1
fi
if ! pkg-config --exists libavcodec libavformat libavutil libswscale; then
  echo "Missing FFmpeg development packages (libavcodec/libavformat/libavutil/libswscale)." >&2
  echo "Install ffmpeg dev packages (e.g. libavcodec-dev libavformat-dev libavutil-dev libswscale-dev)." >&2
  exit 1
fi

rust_ver="$(rustc --version | awk '{print $2}')"
rust_major="${rust_ver%%.*}"
rust_minor_patch="${rust_ver#*.}"
rust_minor="${rust_minor_patch%%.*}"

# heic-decoder-rs/Cargo.toml declares rust-version = 1.92
if [[ "$rust_major" -lt 1 || ( "$rust_major" -eq 1 && "$rust_minor" -lt 92 ) ]]; then
  cat >&2 <<ERR
Rust toolchain is too old for this workspace: rustc $rust_ver
Required: rustc >= 1.92 (heic-decoder-rs sets rust-version = 1.92)

Fix:
  rustup toolchain install stable
  rustup update stable
  # then either:
  cargo +stable build -p openarc
  # or rerun this script after making stable your active/default toolchain
ERR
  exit 1
fi

HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
if [[ -z "$HOST_TRIPLE" ]]; then
  echo "Unable to determine host triple from rustc -vV" >&2
  exit 1
fi

echo "== OpenArc Linux backend build =="
echo "Root: $ROOT_DIR"
echo "Mode: $BUILD_MODE"
echo "Target: $HOST_TRIPLE"
echo "Skip codecs: $SKIP_CODECS"

if [[ "$SKIP_CODECS" -eq 0 ]]; then
  echo "\n[1/3] Building BPG native library..."
  make -C "$BPG_DIR" USE_JCTVC= clean >/dev/null 2>&1 || true
  make -C "$BPG_DIR" -j"$(nproc 2>/dev/null || echo 4)" USE_JCTVC= libbpg_native.a
  mkdir -p "$ROOT_DIR/native/libs/linux"
  cp "$BPG_DIR/libbpg_native.a" "$ROOT_DIR/native/libs/linux/libbpg_native.a"

  echo "\n[2/3] Building ArcMax codec staging libraries..."
  bash "$ARCMAX_DIR/build_codecs.sh"

  echo "\n[3/3] Building Linux FFmpeg bridge (openarc_ffmpeg.so)..."
  mkdir -p "$ROOT_DIR/dist/linux"
  gcc -shared -fPIC -O2 \
    -o "$ROOT_DIR/dist/linux/openarc_ffmpeg.so" \
    "$ROOT_DIR/crates/codecs/ffmpeg_wrapper.c" \
    $(pkg-config --cflags --libs libavcodec libavformat libavutil libswscale)

else
  echo "\n[skip] Skipping native codec builds (--skip-codecs)."
fi

echo "\n[final] Building CLI/backend crates only (no GUI/WPF, no MTP/FFI)..."
if [[ "$BUILD_MODE" == "release" ]]; then
  cargo build --release --target "$HOST_TRIPLE" -p openarc
else
  cargo build --target "$HOST_TRIPLE" -p openarc
fi

echo "\nBuild complete."
if [[ "$BUILD_MODE" == "release" ]]; then
  echo "Binary: $ROOT_DIR/target/$HOST_TRIPLE/release/openarc"
else
  echo "Binary: $ROOT_DIR/target/$HOST_TRIPLE/debug/openarc"
fi
