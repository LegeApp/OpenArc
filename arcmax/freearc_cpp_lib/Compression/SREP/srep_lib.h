// srep_lib.h - C API for SREP compression library
// Refactored from standalone srep.cpp to be callable as a library

#ifndef SREP_LIB_H
#define SREP_LIB_H

#ifdef __cplusplus
extern "C" {
#endif

// Large page allocation modes (missing from Common.h)
typedef enum {
    LP_TRY = 0,
    LP_FORCE = 1,
    LP_DISABLE = 2
} LPType;

// SREP compression methods
typedef enum {
    SREP_METHOD0 = 0,
    SREP_METHOD1 = 1,
    SREP_METHOD2,
    SREP_METHOD3,
    SREP_METHOD4,
    SREP_METHOD5
} SREP_METHOD;

// Command modes
typedef enum {
    SREP_COMPRESS = 0,
    SREP_DECOMPRESS = 1,
    SREP_INFO = 2
} SREP_COMMAND;

// SREP library parameters structure
typedef struct {
    // Configuration
    SREP_COMMAND command;        // compress/decompress/info
    SREP_METHOD method;         // 0..5 (-m0..-m5)
    size_t dict_size;          // -d (default 0)
    size_t buf_size;           // -b (default some mb)
    int accel;                 // -a (acceleration level)
    int accelerator;           // Internal accelerator value
    unsigned min_match;        // -l
    unsigned L;                // -c (chunk size)
    size_t filesize;           // -s (input size)
    int num_threads;           // -t
    int verbosity;             // -v
    int print_pc;             // -pc
    int max_offset;           // for -pc
    int use_mmap;             // -mmap
    LPType large_page_mode;    // -slp
    const char* hash_name;     // -hash=
    int future_lz;            // -f
    int io_lz;                // Derived
    int index_lz;             // Derived
    int delete_input;         // -delete
    const char* temp_file;     // -temp=
    const char* vm_file;       // -vmfile=
    size_t vm_block;           // -vmblock=
    size_t vm_mem;             // -mem
    size_t maximum_save;       // -mBYTES (max match save)
    double time_interval;      // -sX.Y

    // I/O Callbacks (replaces FILE*)
    size_t (*read_cb)(void* user_ctx, void* buf, size_t size);
    size_t (*write_cb)(void* user_ctx, const void* buf, size_t size);
    int64_t (*seek_cb)(void* user_ctx, int64_t offset, int whence);  // SEEK_SET=0, SEEK_CUR=1, SEEK_END=2
    int64_t (*tell_cb)(void* user_ctx);
    void* user_ctx;            // User data passed to callbacks

    // Logging callback (optional, defaults to stderr)
    void (*log_cb)(void* user_ctx, int level, const char* msg);

    // Internal state (opaque)
    void* internal_state;
} srep_params_t;

// SREP library API
int srep_init(srep_params_t* params);
int srep_run(srep_params_t* params);
void srep_free(srep_params_t* params);

// Error codes (from Common.h)
#define SREP_NO_ERRORS        0
#define SREP_WARNINGS          1
#define SREP_ERROR_MEMORY     -1
#define SREP_ERROR_IO         -2
#define SREP_ERROR_CMDLINE    -3

#ifdef __cplusplus
}
#endif

#endif // SREP_LIB_H
