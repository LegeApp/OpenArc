# OpenArc Makefile
# Cross-platform build entry points.
# Linux path intentionally builds backend/CLI only (no WPF GUI, no MTP/FFI requirement).

.PHONY: all codecs clean release debug test install help

OS := $(shell uname -s)
HOST_TRIPLE := $(shell rustc -vV | sed -n 's/^host: //p')
JOBS ?= $(shell nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

# Default target
all: release

# Build native codec dependencies used by CLI/backend.
codecs:
	@echo "Building codec dependencies..."
	@mkdir -p libs
	@cd BPG/libbpg-0.9.8 && $(MAKE) -j$(JOBS) libbpg_native.a
	@cp BPG/libbpg-0.9.8/libbpg_native.a libs/libbpg_native.a
	@bash arcmax/build_codecs.sh

# Release build
release: codecs
	@if [ "$(OS)" = "Linux" ] || [ "$(OS)" = "Darwin" ]; then \
		echo "Building Linux/macOS backend only (openarc CLI + backend libs)..."; \
		cargo build --release --target "$(HOST_TRIPLE)" -p openarc; \
	else \
		echo "Building full Windows workspace..."; \
		cargo build --release --workspace --exclude codecs; \
	fi

# Debug build
debug: codecs
	@if [ "$(OS)" = "Linux" ] || [ "$(OS)" = "Darwin" ]; then \
		echo "Building Linux/macOS backend only (openarc CLI + backend libs)..."; \
		cargo build --target "$(HOST_TRIPLE)" -p openarc; \
	else \
		echo "Building full Windows workspace..."; \
		cargo build --workspace --exclude codecs; \
	fi

# Clean build artifacts
clean:
	cargo clean
	cd BPG/libbpg-0.9.8 && $(MAKE) clean 2>/dev/null || true
	rm -rf arcmax/codec_staging arcmax/codec_build

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
	@echo "  make release  - Linux/macOS: backend CLI only; Windows: full workspace"
	@echo "  make debug    - Linux/macOS: backend CLI only; Windows: full workspace"
	@echo "  make clean    - Clean build artifacts"
	@echo "  make test     - Run backend tests"
	@echo "  make install  - Install openarc CLI binary"
	@echo ""
	@echo "Linux quick start:"
	@echo "  ./build-linux-backend.sh --release"
