// SREP Library Public API
// Thread-safe compression/decompression library based on SREP 3.93
#ifndef SREP_LIB_H
#define SREP_LIB_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Error codes
typedef enum {
    SREP_NO_ERRORS = 0,
    SREP_WARNINGS = 1,
    SREP_ERROR_CMDLINE = 2,
    SREP_ERROR_IO = 3,
    SREP_ERROR_COMPRESSION = 4,
    SREP_ERROR_MEMORY = 5
} srep_error_t;

// Compression methods
typedef enum {
    SREP_METHOD0 = 0,  // In-memory compression
    SREP_METHOD1 = 1,  // Content-defined chunking
    SREP_METHOD2 = 2,  // ZPAQ CDC
    SREP_METHOD3 = 3,  // Precompute digests
    SREP_METHOD4 = 4,  // Future LZ
    SREP_METHOD5 = 5,  // Exhaustive search
    SREP_METHOD_FIRST = SREP_METHOD0,
    SREP_METHOD_LAST = SREP_METHOD5
} srep_method_t;

// Large page modes
typedef enum {
    SREP_LP_DISABLE = 0,
    SREP_LP_TRY = 1,
    SREP_LP_FORCE = 2
} srep_lp_type_t;

// Hash descriptor
typedef struct {
    const char* name;
    unsigned num;
    unsigned seed_size;
    unsigned hash_size;
} srep_hash_descriptor_t;

// Performance counters
typedef struct {
    uint64_t max_offset;
    uint64_t find_match;
    uint64_t find_match_memaccess;
    uint64_t check_hasharr;
    uint64_t hash_found;
    uint64_t check_len;
    uint64_t record_match;
    uint64_t total_match_len;
} srep_perf_counters_t;

// I/O callbacks
typedef size_t (*srep_read_fn)(void* user_data, void* buf, size_t size);
typedef size_t (*srep_write_fn)(void* user_data, const void* buf, size_t size);
typedef int64_t (*srep_seek_fn)(void* user_data, int64_t offset, int whence);
typedef int64_t (*srep_tell_fn)(void* user_data);
typedef void (*srep_log_fn)(void* user_data, int level, const char* message);

// Configuration structure
typedef struct {
    // Compression parameters
    srep_method_t method;
    unsigned min_match;
    unsigned chunk_size;
    size_t dict_size;
    size_t dict_hash_size;
    unsigned dict_min_match;
    unsigned dict_chunk;
    size_t buf_size;
    int accel;
    int accelerator;
    int io_accel;
    size_t file_size;
    int num_threads;
    int verbosity;
    int print_counters;
    uint64_t max_offset;
    int use_mmap;
    srep_lp_type_t large_pages;
    const char* hash_name;
    int future_lz;
    int index_lz;
    int io_lz;
    size_t vm_mem;
    size_t vm_block;
    const char* vm_file;
    size_t max_save;
    double stats_interval;
    int delete_input;
    
    // I/O callbacks (for non-file operations)
    srep_read_fn read_cb;
    srep_write_fn write_cb;
    srep_seek_fn seek_cb;
    srep_tell_fn tell_cb;
    void* io_user_data;
    
    // Log callback
    srep_log_fn log_cb;
    void* log_user_data;
} srep_config_t;

// Opaque context handle
typedef struct srep_ctx srep_ctx_t;

// Initialize default configuration
void srep_config_init(srep_config_t* config);

// Core API functions
srep_error_t srep_init(srep_ctx_t** ctx, const srep_config_t* config);
srep_error_t srep_compress_file(srep_ctx_t* ctx, const char* input, const char* output);
srep_error_t srep_decompress_file(srep_ctx_t* ctx, const char* input, const char* output);
srep_error_t srep_compress_memory(srep_ctx_t* ctx, const void* input, size_t input_size,
                                  void* output, size_t* output_size);
srep_error_t srep_decompress_memory(srep_ctx_t* ctx, const void* input, size_t input_size,
                                    void* output, size_t* output_size);
srep_error_t srep_compress_stream(srep_ctx_t* ctx);
srep_error_t srep_decompress_stream(srep_ctx_t* ctx);
srep_error_t srep_get_info(srep_ctx_t* ctx, const char* filename);

// Utility functions
const srep_perf_counters_t* srep_get_perf_counters(const srep_ctx_t* ctx);
void srep_reset_perf_counters(srep_ctx_t* ctx);
const srep_hash_descriptor_t* srep_get_hash_descriptors(size_t* count);
const char* srep_error_string(srep_error_t error);
const char* srep_get_last_error_msg(const srep_ctx_t* ctx);
void srep_free(srep_ctx_t* ctx);

// Version info
const char* srep_version(void);
const char* srep_date(void);

#ifdef __cplusplus
}
#endif

#endif // SREP_LIB_H
