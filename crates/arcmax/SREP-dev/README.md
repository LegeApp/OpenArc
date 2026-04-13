# SREP Library - Refactored Version

This is a thread-safe library refactoring of SREP 3.93 beta, originally by Bulat Ziganshin.

## Overview

The original SREP.cpp (~3k lines) has been refactored into a clean, thread-safe C library with C++ implementation:

- **srep_lib.h** - Public C API header (FFI-friendly for Rust, Python, etc.)
- **srep_lib.cpp** - Main implementation with SrepContext class
- **example.cpp** - Usage examples

## Key Improvements Over Original

### 1. Thread Safety
- All state encapsulated in `SrepContext` class
- No global variables
- Multiple contexts can run concurrently in different threads
- Thread-safe performance counters with mutex protection

### 2. Error Handling
- Returns error codes instead of calling `exit()`
- Detailed error messages via `srep_get_last_error_msg()`
- Allows graceful error recovery

### 3. Memory Management
- RAII pattern ensures no leaks
- Automatic cleanup on context destruction
- Tracks all allocations for proper cleanup

### 4. Configuration
- Clean `srep_config_t` structure
- Sensible defaults via `srep_config_init()`
- All original parameters supported

### 5. Callbacks
- Custom I/O callbacks for non-file operations
- Logging callback for custom output handling
- Enables buffer/stream/pipe compression

### 6. Performance
- Direct function calls (no subprocess overhead)
- Configurable thread pool
- Performance counters for profiling

## API Reference

### Initialization

```c
// Initialize default configuration
srep_config_t config;
srep_config_init(&config);

// Customize as needed
config.method = SREP_METHOD3;
config.min_match = 32;
config.verbosity = 2;

// Create context
srep_ctx_t* ctx = NULL;
srep_error_t err = srep_init(&ctx, &config);
if (err != SREP_NO_ERRORS) {
    fprintf(stderr, "Error: %s\n", srep_error_string(err));
    return 1;
}
```

### Compression

```c
// File compression
err = srep_compress_file(ctx, "input.bin", "output.srep");

// Memory compression
void* output;
size_t output_size;
err = srep_compress_memory(ctx, input, input_size, output, &output_size);

// Stream compression (with callbacks)
config.read_cb = my_read_function;
config.write_cb = my_write_function;
err = srep_compress_stream(ctx);
```

### Decompression

```c
// File decompression
err = srep_decompress_file(ctx, "input.srep", "output.bin");

// Memory decompression
err = srep_decompress_memory(ctx, input, input_size, output, &output_size);
```

### Performance Counters

```c
config.print_counters = 1;
// ... perform compression ...
const srep_perf_counters_t* pc = srep_get_perf_counters(ctx);
printf("Matches found: %llu\n", pc->find_match);
printf("Total match length: %llu\n", pc->total_match_len);
```

### Cleanup

```c
srep_free(ctx);
```

## Configuration Options

### Compression Methods

- `SREP_METHOD0` - In-memory compression (fastest, least compression)
- `SREP_METHOD1` - Content-defined chunking
- `SREP_METHOD2` - ZPAQ CDC
- `SREP_METHOD3` - Precompute digests (recommended, best ratio)
- `SREP_METHOD4` - Future LZ
- `SREP_METHOD5` - Exhaustive search (slowest, best compression)

### Key Parameters

```c
config.min_match = 32;          // Minimum match length (16-256)
config.chunk_size = 32;         // Hash chunk size
config.buf_size = 8*1024*1024;  // Buffer size (larger = faster)
config.dict_size = 0;           // Dictionary size (0 = auto)
config.num_threads = 4;         // Thread pool size
config.large_pages = SREP_LP_TRY; // Use large pages if available
config.hash_name = "vmac";      // Hash algorithm
config.verbosity = 2;           // Logging level (0-3)
```

### Available Hash Algorithms

- `"md5"` - MD5 (16 bytes)
- `"sha1"` - SHA-1 (20 bytes)
- `"sha512"` - SHA-512 (64 bytes)
- `"vmac"` - VMAC (16 bytes, fast, keyed)
- `"siphash"` - SipHash (8 bytes, fast, keyed)

## Thread Safety Example

```c
void compress_in_thread(const char* input, const char* output) {
    srep_config_t config;
    srep_config_init(&config);
    
    srep_ctx_t* ctx;
    srep_init(&ctx, &config);
    srep_compress_file(ctx, input, output);
    srep_free(ctx);
}

// Safe to run in multiple threads
pthread_create(&t1, NULL, thread1_func, NULL);
pthread_create(&t2, NULL, thread2_func, NULL);
```

## Building

### Basic Build

```bash
# Compile library
g++ -c -O3 -std=c++11 srep_lib.cpp -o srep_lib.o

# Create static library
ar rcs libsrep.a srep_lib.o

# Build example
g++ -std=c++11 example.cpp -L. -lsrep -o srep_example
```

### With Dependencies

If you have the original FreeArc dependencies:

```bash
g++ -c -O3 -std=c++11 -DHAVE_FREEARC_DEPS \
    -I../path/to/freearc \
    srep_lib.cpp -o srep_lib.o
```

### CMake Build

```cmake
cmake_minimum_required(VERSION 3.10)
project(srep_lib)

set(CMAKE_CXX_STANDARD 11)

add_library(srep STATIC srep_lib.cpp)
target_include_directories(srep PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})

add_executable(srep_example example.cpp)
target_link_libraries(srep_example srep)
```

## Rust FFI Example

Using `bindgen` or manual bindings:

```rust
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[link(name = "srep")]
extern "C" {
    fn srep_config_init(config: *mut SrepConfig);
    fn srep_init(ctx: *mut *mut SrepCtx, config: *const SrepConfig) -> i32;
    fn srep_compress_file(ctx: *mut SrepCtx, input: *const c_char, 
                          output: *const c_char) -> i32;
    fn srep_free(ctx: *mut SrepCtx);
}

pub fn compress_file(input: &str, output: &str) -> Result<(), String> {
    unsafe {
        let mut config = std::mem::zeroed();
        srep_config_init(&mut config);
        
        let mut ctx = std::ptr::null_mut();
        let err = srep_init(&mut ctx, &config);
        if err != 0 {
            return Err("Failed to initialize SREP".to_string());
        }
        
        let input_c = CString::new(input).unwrap();
        let output_c = CString::new(output).unwrap();
        
        let err = srep_compress_file(ctx, input_c.as_ptr(), output_c.as_ptr());
        srep_free(ctx);
        
        if err == 0 { Ok(()) } else { Err("Compression failed".to_string()) }
    }
}
```

## Implementation Status

### ✅ Completed
- Header file with full API
- Context class with state encapsulation
- Configuration and initialization
- Error handling and logging
- Performance counters
- Thread safety infrastructure
- Memory management
- Hash algorithm selection

### 🚧 To Be Implemented
- File compression/decompression logic (port from original)
- Memory buffer compression
- Stream I/O with callbacks
- Template functions from original code
- Background threading support
- Virtual memory management
- Dictionary compression

The core infrastructure is complete. The compression/decompression logic needs to be ported from the original template functions, adapted to use the context instead of globals.

## Migration from Original

### Before (Original CLI)
```bash
srep -m3 -l32 input.bin output.srep
```

### After (Library)
```c
srep_config_t config;
srep_config_init(&config);
config.method = SREP_METHOD3;
config.min_match = 32;

srep_ctx_t* ctx;
srep_init(&ctx, &config);
srep_compress_file(ctx, "input.bin", "output.srep");
srep_free(ctx);
```

### Advantages of Library Approach

1. **No subprocess overhead** - Direct function calls instead of spawning processes
2. **Better error handling** - Return codes instead of exit()
3. **Thread safety** - Run multiple compressions concurrently
4. **Memory efficiency** - Share resources between operations
5. **Integration** - Easy to embed in applications
6. **FFI friendly** - Clean C API for any language

## Performance Notes

- Use `SREP_METHOD3` for best compression ratio
- Increase `buf_size` for better performance (more RAM)
- Set `num_threads` to CPU count for parallel processing
- Use `SREP_LP_TRY` for large page support (requires privileges)
- Enable `print_counters` for profiling

## License

Original SREP code by Bulat Ziganshin (http://freearc.org)
Library refactoring maintains original licensing terms.

## References

- Original SREP: http://freearc.org/research/SREP.aspx
- FreeArc Download: http://freearc.org/Download-Alpha.aspx
- Email: Bulat.Ziganshin@gmail.com
