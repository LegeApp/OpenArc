# FreeARC C++ Library Integration via FFI

## Overview
This document outlines the approach to integrate the complete FreeARC C++ codebase via FFI (Foreign Function Interface) to provide full ARC format support in Rust.

## Approach
Instead of extracting individual codec files, we're using the entire FreeARC C++ codebase to maintain:
- Complete functionality
- Proper interdependencies
- Reference implementation accuracy
- All ARC format features

## Components

### 1. FreeARC C++ Source Tree
Located at: `freearc_cpp_lib/`
- Contains the complete FreeARC C++ library
- All compression algorithms (Tornado, GRZip, PPMD, LZP, etc.)
- All dependencies and interconnections
- Encryption support included

### 2. C Wrapper Layer
Located at: `freearc_cpp_lib/freearc_wrapper.c`
- Provides C-compatible interface to FreeARC functions
- Exposes only the functions needed by Rust
- Handles parameter conversions
- Manages memory allocation/deallocation

### 3. Rust FFI Bindings
Located in: `src/codecs/`
- Updated codec files with FFI function declarations
- Safe Rust wrappers around unsafe FFI calls
- Proper error handling and memory management

### 4. Build System Integration
Located at: `build.rs`
- Configures cc crate to compile FreeARC C++ code
- Sets up include paths and compiler flags
- Links the resulting library with Rust binary

## Functions Exposed

### GRZip
- `freearc_grzip_decompress()` - Decompress GRZip blocks
- `freearc_grzip_compress()` - Compress GRZip blocks

### Tornado
- `freearc_tornado_decompress()` - Decompress Tornado blocks

### PPMD
- `freearc_ppmd_decompress()` - Decompress PPMD blocks

### LZP
- `freearc_lzp_decompress()` - Decompress LZP blocks

### Utilities
- `freearc_big_alloc()` / `freearc_big_free()` - Memory management
- `freearc_set_threads()` / `freearc_get_threads()` - Threading control

## Benefits

1. **Complete Compatibility**: Uses the exact same algorithms as FreeARC
2. **No Dependency Issues**: Entire codebase stays together
3. **Full Feature Support**: All FreeARC features available
4. **Maintainable**: Updates to FreeARC automatically available
5. **Accurate**: No risk of porting errors

## Next Steps

1. **Build and Test**: Verify the build system compiles FreeARC correctly
2. **Link and Run**: Ensure FFI calls work from Rust
3. **Integration**: Connect to the main ARC parsing logic
4. **Testing**: Validate against actual FreeARC archives

## Potential Challenges

1. **Build Complexity**: Large C++ codebase may have complex build requirements
2. **Platform Differences**: Windows/Linux/MacOS may need different configurations
3. **Memory Management**: Ensuring proper allocation/deallocation across FFI boundary
4. **Threading**: Managing FreeARC's multithreading in Rust context

## Conclusion

This approach provides the most robust path to full FreeARC compatibility by leveraging the complete, tested, and maintained FreeARC C++ codebase while exposing it through a clean FFI interface to Rust. This ensures maximum compatibility and feature completeness while maintaining the performance of the original C++ implementations.