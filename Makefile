# OpenArc Makefile
# Provides cross-platform build commands

.PHONY: all codecs clean release debug test install help

# Default target
all: codecs
	cargo build --workspace --exclude codecs

# Build only codecs
codecs:
	@echo "Building codec dependencies..."
	@if command -v make >/dev/null 2>&1; then \
		cd BPG && make -j4 && cd ..; \
		if [ -f BPG/libbpg.a ]; then \
			cp BPG/libbpg.a libs/libbpg_native.a; \
		fi; \
		cd arcmax/codec_staging && make -j4 && cd ../..; \
	else \
		echo "ERROR: make command not found. Please install build tools."; \
		exit 1; \
	fi

# Release build
release: codecs
	cargo build --release --workspace --exclude codecs

# Debug build
debug: codecs
	cargo build --workspace --exclude codecs

# Clean all build artifacts
clean:
	cargo clean
	cd BPG && make clean 2>/dev/null || true
	cd arcmax/codec_staging && make clean 2>/dev/null || true

# Test all components
test: codecs
	cargo test --workspace --exclude codecs

# Install release binaries
install: release
	cargo install --path .

# Help target
help:
	@echo "OpenArc Build Commands:"
	@echo "  make all      - Build codecs and all workspace components (default)"
	@echo "  make codecs   - Build only codec dependencies"
	@echo "  make release  - Build release version"
	@echo "  make debug    - Build debug version"
	@echo "  make clean    - Clean all build artifacts"
	@echo "  make test     - Run tests"
	@echo "  make install  - Install release binaries"
	@echo "  make help     - Show this help"
	@echo ""
	@echo "Quick start:"
	@echo "  make all          # Build everything"
	@echo "  make release      # Build release version"
	@echo "  cargo run --bin openarc -- [args]  # Run CLI"
	@echo ""
	@echo "GUI: Build DocBrakeGUI separately with dotnet publish"
