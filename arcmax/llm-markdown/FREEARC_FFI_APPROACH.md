# FreeARC FFI Approach for Compression Algorithms

## Overview
Instead of porting complex C++ compression algorithms to Rust, this document outlines the approach to use FFI (Foreign Function Interface) to call the existing FreeARC C++ implementations directly.

## Advantages of FFI Approach

### 1. Accuracy and Compatibility
- **Exact behavior**: Uses the same algorithms as FreeARC, ensuring 100% compatibility
- **No translation errors**: Eliminates risks from manual porting of complex algorithms
- **Reference implementation**: Leverages proven, tested C++ code

### 2. Development Efficiency
- **Faster implementation**: Much quicker than porting complex C++ algorithms
- **Reduced maintenance**: Updates to FreeARC algorithms automatically available
- **Less debugging**: No need to debug porting errors or subtle behavioral differences

### 3. Technical Benefits
- **Performance**: Native C++ performance maintained
- **Memory management**: C++ handles its own memory allocation/deallocation
- **Feature completeness**: Access to all FreeARC features without porting gaps

## FreeARC C++ Structure Analysis

### Available Libraries in `freearc/clibs/Compression/`
- **Tornado**: `Tornado/Tornado.cpp` - Fast LZ77 with multiple encoders
- **GRZip**: `GRZip/C_GRZip.cpp` - BWT/ST4 + MTF/WFC + arithmetic coding
- **LZP**: `LZP/LZP.c` - LZ-based preprocessing
- **PPMD**: `PPMD/` - PPMII with Subbotin carryless rangecoder
- **Delta**: `Delta/` - Delta encoding
- **Dict**: `Dict/` - Dictionary preprocessing (E8E9, Intel, etc.)

### Exportable Functions
Most FreeARC compression libraries expose C-style interfaces:
```cpp
// Example from Tornado
extern "C" {
int tor_compress(PackMethod m, CALLBACK_FUNC *callback, void *auxdata);
int tor_decompress(CALLBACK_FUNC *callback, void *auxdata);
}

// Example from GRZip
extern "C" {
sint32 __cdecl GRZip_CompressBlock(uint8 *Input, sint32 Size, uint8 *Output, sint32 Mode);
sint32 __cdecl GRZip_DecompressBlock(uint8 *Input, sint32 Size, uint8 *Output);
}
```

## FFI Implementation Strategy

### 1. Create C-Compatible Wrappers
For each algorithm that needs FFI exposure, create thin C wrappers that:
- Accept simple data types (pointers, integers, sizes)
- Handle memory allocation/deallocation safely
- Provide consistent error handling

### 2. Rust FFI Bindings
Create Rust FFI bindings that:
- Declare extern "C" functions matching the C++ exports
- Handle data conversion between Rust and C++
- Provide safe Rust wrappers around unsafe FFI calls

### 3. Build System Integration
- Compile FreeARC C++ libraries as static or dynamic libraries
- Link with Rust build process
- Handle cross-platform compilation differences

## Implementation Plan

### Phase 1: Tornado FFI
```rust
// Proposed interface
pub fn tornado_decompress(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    // Call C++ function via FFI
    unsafe {
        let mut output = vec![0u8; expected_size];
        let result = freearc_tornado_decompress(
            input.as_ptr(), 
            input.len() as i32, 
            output.as_mut_ptr(), 
            expected_size as i32
        );
        if result >= 0 {
            Ok(output)
        } else {
            Err(anyhow!("Tornado decompression failed: {}", result))
        }
    }
}
```

### Phase 2: GRZip FFI
```rust
// Similar approach for GRZip
pub fn grzip_decompress(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    // Call C++ GRZip decompression
}
```

### Phase 3: Other Algorithms
- LZP, Delta, Dict preprocessing algorithms
- PPMD with Subbotin rangecoder
- Repetition detection algorithms

## Technical Considerations

### 1. Memory Management
- C++ allocates/deallocates its own buffers where possible
- Rust manages the final output buffer
- Clear ownership boundaries to prevent memory leaks

### 2. Error Handling
- Map C++ return codes to Rust Result types
- Handle C++ exceptions at FFI boundary
- Provide meaningful error messages

### 3. Thread Safety
- Ensure FFI calls are thread-safe if using parallel extraction
- Consider mutex protection for shared resources if needed

### 4. Platform Compatibility
- Handle different calling conventions (Windows/Linux)
- Manage different data type sizes (32-bit vs 64-bit)
- Cross-compilation considerations

## Building FreeARC Libraries

### Static Library Approach
1. Compile FreeARC C++ sources to static libraries
2. Link statically with Rust binary
3. Ensures single executable with no external dependencies

### Dynamic Library Approach
1. Build FreeARC as shared libraries (.dll, .so, .dylib)
2. Load at runtime from Rust
3. Allows updates to compression libraries independently

## Risk Mitigation

### 1. Gradual Migration
- Keep existing Rust implementations as fallback
- Switch to FFI implementations gradually
- Maintain compatibility during transition

### 2. Testing Framework
- Compare FFI results with known test vectors
- Verify against FreeARC reference output
- Maintain regression tests

### 3. Fallback Mechanisms
- Keep simplified Rust implementations for basic cases
- Graceful degradation if FFI calls fail
- Diagnostic information for debugging

## Expected Timeline

### Week 1: Infrastructure Setup
- Configure build system for C++ compilation
- Create basic FFI wrapper for one algorithm
- Test basic functionality

### Week 2: Core Algorithms
- Implement FFI for Tornado and GRZip
- Add error handling and memory management
- Test with sample archives

### Week 3: Remaining Algorithms
- Add FFI for LZP, Dict, PPMD, etc.
- Integrate with existing FreeARC parser
- Comprehensive testing

### Week 4: Optimization and Polish
- Performance optimization
- Error handling refinement
- Documentation and cleanup

## Conclusion

The FFI approach offers significant advantages over porting:
- **Reliability**: Uses proven C++ implementations
- **Compatibility**: Guaranteed FreeARC format compatibility
- **Efficiency**: Faster development with better results
- **Maintainability**: Easier to maintain and update

This approach aligns perfectly with the goal of creating a FreeARC-compatible archiver while leveraging the existing, well-tested compression algorithms.