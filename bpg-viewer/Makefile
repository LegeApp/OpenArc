# Makefile for BPG Viewer Library

.PHONY: all build release test clean examples docs install

# Default target
all: build

# Build in debug mode
build:
	cargo build

# Build in release mode (optimized)
release:
	cargo build --release

# Build with specific BPG library path
release-custom:
	BPG_LIB_PATH=$(BPG_PATH) cargo build --release

# Run tests
test:
	cargo test

# Run tests with output
test-verbose:
	cargo test -- --nocapture

# Build examples
examples:
	cargo build --release --examples
	@echo "Examples built in target/release/examples/"

# Build C FFI example
example-c: release
	gcc -o target/release/ffi_example examples/ffi_usage.c \
		-I./include \
		-L./target/release \
		-lbpg_viewer \
		-lm -lpthread -ldl
	@echo "C example built: target/release/ffi_example"

# Build command-line tools
tools:
	cargo build --release --bin bpg-view --bin bpg-thumb
	@echo "Tools built:"
	@echo "  - target/release/bpg-view"
	@echo "  - target/release/bpg-thumb"

# Build GUI viewer
gui:
	cargo build --release --features gui --bin bpg-gui
	@echo "GUI viewer built: target/release/bpg-gui"

# Run GUI viewer
run-gui: gui
	./target/release/bpg-gui

# Build all binaries (tools + GUI)
all-bins: tools gui
	@echo "All binaries built"

# Generate documentation
docs:
	cargo doc --no-deps --open

# Clean build artifacts
clean:
	cargo clean
	rm -f target/release/ffi_example

# Install to system (requires sudo on Linux/macOS)
install: release
	cargo install --path .

# Uninstall from system
uninstall:
	cargo uninstall bpg-viewer

# Check code formatting
fmt-check:
	cargo fmt -- --check

# Format code
fmt:
	cargo fmt

# Run clippy linter
lint:
	cargo clippy -- -D warnings

# Run all checks (format, lint, test)
check: fmt-check lint test

# Create release package
package: release
	mkdir -p dist
	cp target/release/libbpg_viewer.a dist/
	cp include/bpg_viewer.h dist/
	cp README.md dist/
	@echo "Package created in dist/"

# Show help
help:
	@echo "BPG Viewer Library - Makefile Targets"
	@echo ""
	@echo "Building:"
	@echo "  make build          - Build in debug mode"
	@echo "  make release        - Build in release mode (optimized)"
	@echo "  make tools          - Build command-line tools"
	@echo "  make gui            - Build GUI viewer"
	@echo "  make all-bins       - Build all binaries (tools + GUI)"
	@echo "  make run-gui        - Build and run GUI viewer"
	@echo "  make examples       - Build Rust examples"
	@echo "  make example-c      - Build C FFI example"
	@echo ""
	@echo "Testing:"
	@echo "  make test           - Run tests"
	@echo "  make test-verbose   - Run tests with output"
	@echo "  make check          - Run all checks (format, lint, test)"
	@echo ""
	@echo "Documentation:"
	@echo "  make docs           - Generate and open documentation"
	@echo ""
	@echo "Maintenance:"
	@echo "  make fmt            - Format code"
	@echo "  make lint           - Run clippy linter"
	@echo "  make clean          - Clean build artifacts"
	@echo ""
	@echo "Installation:"
	@echo "  make install        - Install to system"
	@echo "  make uninstall      - Uninstall from system"
	@echo "  make package        - Create release package"
