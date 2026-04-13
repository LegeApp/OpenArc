# OpenArc Makefile
# Cross-platform CLI build entry points.
# GUI projects are intentionally out of the supported build path.

.PHONY: all codecs clean release debug test install help

OS := $(shell uname -s)
HOST_TRIPLE := $(shell rustc -vV | sed -n 's/^host: //p')
JOBS ?= $(shell nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

# Default target
all: release

# Build native codec dependencies used by CLI/backend.
codecs:
	@echo "Building codec dependencies..."
	@mkdir -p native/libs/linux
	@cd native/BPG/libbpg-0.9.8 && $(MAKE) -j$(JOBS) libbpg_native.a
	@cp native/BPG/libbpg-0.9.8/libbpg_native.a native/libs/linux/libbpg_native.a
	@bash crates/arcmax/build_codecs.sh

# Release build
release: codecs
	@if [ "$(OS)" = "Linux" ] || [ "$(OS)" = "Darwin" ]; then \
		echo "Building Linux/macOS openarc CLI..."; \
		cargo build --release --target "$(HOST_TRIPLE)" -p openarc; \
	else \
		echo "Building Windows openarc CLI..."; \
		cargo build --release --target "$(HOST_TRIPLE)" -p openarc --bin openarc; \
	fi

# Debug build
debug: codecs
	@if [ "$(OS)" = "Linux" ] || [ "$(OS)" = "Darwin" ]; then \
		echo "Building Linux/macOS openarc CLI..."; \
		cargo build --target "$(HOST_TRIPLE)" -p openarc; \
	else \
		echo "Building Windows openarc CLI..."; \
		cargo build --target "$(HOST_TRIPLE)" -p openarc --bin openarc; \
	fi

# Clean build artifacts
clean:
	cargo clean
	cd native/BPG/libbpg-0.9.8 && $(MAKE) clean 2>/dev/null || true
	rm -rf crates/arcmax/codec_staging crates/arcmax/codec_build native/libs/linux

# Test backend components
test: codecs
	cargo test -p arcmax --target "$(HOST_TRIPLE)"
	cargo test -p openarc --target "$(HOST_TRIPLE)"

# Install release binary
install: release
	cargo install --path . --bin openarc

# Help target
help:
	@echo "OpenArc Build Commands:"
	@echo "  make codecs   - Build backend codec dependencies (BPG + ArcMax)"
	@echo "  make release  - Build the CLI in release mode"
	@echo "  make debug    - Build the CLI in debug mode"
	@echo "  make clean    - Clean build artifacts"
	@echo "  make test     - Run backend tests"
	@echo "  make install  - Install openarc CLI binary"
	@echo ""
	@echo "Linux quick start:"
	@echo "  ./build-linux-backend.sh --release"
	@echo "Windows quick start:"
	@echo "  ./build-all.ps1 -Release"
