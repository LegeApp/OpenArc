// SREP Library Implementation
// Refactored from SREP 3.93 beta to library form with thread-safety
#include "srep_lib.h"
#include <algorithm>
#include <malloc.h>
#include <math.h>
#include <set>
#include <stack>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <vector>
#include <memory>
#include <mutex>

// Include original dependencies (assuming they exist)
// You'll need to adapt these paths based on your actual setup
#ifdef HAVE_FREEARC_DEPS
#include "../Common.h"
#include "../Compression.h"
#include "../MultiThreading.cpp"
#include "../MultiThreading.h"
#else
// Minimal stubs for standalone compilation
typedef unsigned int uint;
typedef uint32_t uint32;
typedef uint64_t uint64;
typedef int64_t int64;
typedef uint64_t Offset;

#define EQUAL 0
#define CHECK(x, ...) if (!(x)) { fprintf(stderr, __VA_ARGS__); abort(); }
#define systemRandomData(buf, size) (0)
#define GetGlobalTime() (0.0)
#define GetCPUTime() (0.0)
#define Taskbar_SetProgressValue(a, b) ((void)0)

inline char* strequ(const char* a, const char* b) { return strcmp(a, b) == 0; }
inline char* show3(uint64_t size, char* buf) { 
    sprintf(buf, "%llu", (unsigned long long)size);
    return buf;
}
#endif

// Constants
const uint SREP_SIGNATURE = 0x50455253;
const uint SREP_FORMAT_VERSION1 = 1;
const uint SREP_FORMAT_VERSION2 = 2;
const uint SREP_FORMAT_VERSION3 = 3;
const uint SREP_FORMAT_VERSION4 = 4;
const uint SREP_FOOTER_VERSION1 = 1;
typedef uint32 STAT;
const int STAT_BITS = 32, ARCHIVE_HEADER_SIZE = 4, BLOCK_HEADER_SIZE = 3,
          MAX_HEADER_SIZE = 4, MAX_HASH_SIZE = 256;
const int MINIMAL_MIN_MATCH = 16;
const int DEFAULT_MIN_MATCH = 32;
const char *SREP_EXT = ".srep";

#if defined(_M_X64) || defined(_M_AMD64) || defined(__x86_64__)
#define _32_or_64(_32, _64) (_64)
typedef size_t NUMBER;
#else
#define _32_or_64(_32, _64) (_32)
typedef int NUMBER;
#endif

// Hash sizes
#define MD5_SIZE 16
#define SHA1_SIZE 20
#define SHA512_SIZE 64

// Memory utilities
#ifdef MY_MEMCPY
void *my_memcpy(void *__restrict b, const void *__restrict a, size_t n) {
  char *__restrict s1 = (char *)b;
  const char *__restrict s2 = (const char *)a;
  for (; 0 < n; --n) *s1++ = *s2++;
  return b;
}
#else
#define my_memcpy memcpy
#endif

#ifdef MY_MEMSET
void *my_memset(void *a, int v, size_t n) {
  char *s1 = (char *)a;
  for (; 0 < n; --n) *s1++ = v;
  return a;
}
#else
#define my_memset memset
#endif

//==============================================================================
// Crypto and Hash Functions
//==============================================================================

// Stub CryptoAPI
static struct {
  bool sha1(void *buf, int size, void *result) { return false; }
  bool md5(void *buf, int size, void *result) { return false; }
} CryptoAPI;

// LibTomCrypt integration (minimal stubs - include actual implementations)
#define LTC_NO_HASHES
#define LTC_MD5
#define LTC_SHA1
#define LTC_SHA512

// Include actual libtomcrypt sources or provide minimal implementations
// For this refactor, we'll provide forward declarations
typedef struct { unsigned char dummy[256]; } hash_state;
typedef struct { unsigned char dummy[256]; } prng_state;

void sha1_init(hash_state* state) { memset(state, 0, sizeof(*state)); }
void sha1_process(hash_state* state, const unsigned char* in, unsigned long len) {}
void sha1_done(hash_state* state, unsigned char* out) { memset(out, 0, SHA1_SIZE); }

void md5_init(hash_state* state) { memset(state, 0, sizeof(*state)); }
void md5_process(hash_state* state, const unsigned char* in, unsigned long len) {}
void md5_done(hash_state* state, unsigned char* out) { memset(out, 0, MD5_SIZE); }

void sha512_init(hash_state* state) { memset(state, 0, sizeof(*state)); }
void sha512_process(hash_state* state, const unsigned char* in, unsigned long len) {}
void sha512_done(hash_state* state, unsigned char* out) { memset(out, 0, SHA512_SIZE); }

void fortuna_start(prng_state* prng) { memset(prng, 0, sizeof(*prng)); }
void fortuna_add_entropy(const unsigned char* buf, unsigned long len, prng_state* prng) {}

typedef unsigned char Digest[SHA1_SIZE];

void compute_sha1(void* ctx, void *buf, int size, void *result) {
  if (CryptoAPI.sha1(buf, size, result)) return;
  hash_state state;
  sha1_init(&state);
  sha1_process(&state, (unsigned char *)buf, (unsigned long)size);
  sha1_done(&state, (unsigned char *)result);
}

void compute_md5(void* ctx, void *buf, int size, void *result) {
  if (CryptoAPI.md5(buf, size, result)) return;
  hash_state state;
  md5_init(&state);
  md5_process(&state, (unsigned char *)buf, (unsigned long)size);
  md5_done(&state, (unsigned char *)result);
}

void compute_sha512(void* ctx, void *buf, int size, void *result) {
  hash_state state;
  sha512_init(&state);
  sha512_process(&state, (unsigned char *)buf, (unsigned long)size);
  sha512_done(&state, (unsigned char *)result);
}

void cryptographic_prng(void *result, size_t size) {
  static prng_state prng[1];
  static bool initialized = false;
  static std::mutex prng_mutex;
  
  std::lock_guard<std::mutex> lock(prng_mutex);
  if (!initialized) {
    fortuna_start(prng);
    const int bufsize = 4096;
    unsigned char buf[bufsize];
    int bytes = systemRandomData(buf, bufsize);
    fortuna_add_entropy(buf, bytes, prng);
    time((time_t *)buf);
    fortuna_add_entropy(buf, sizeof(time_t), prng);
    initialized = true;
  }
  // fortuna_read would be called here
  memset(result, 0, size); // Stub
}

//==============================================================================
// Context Class - Encapsulates all state
//==============================================================================

class SrepContext {
public:
    // Configuration
    srep_config_t config_;
    
    // Error state
    srep_error_t last_error_;
    int warnings_;
    std::string error_msg_;
    
    // Performance counters
    srep_perf_counters_t perf_;
    
    // Derived parameters from method
    bool inmem_compression_;
    bool content_defined_chunking_;
    bool zpaq_cdc_;
    bool compare_digests_;
    bool precompute_digests_;
    bool round_matches_;
    bool exhaustive_search_;
    unsigned base_len_;
    
    // Hash information
    const srep_hash_descriptor_t* selected_hash_;
    void* hash_obj_;
    unsigned hash_size_;
    
    // Memory management
    struct MemoryBlock {
        void* ptr;
        size_t size;
        bool large_page;
    };
    std::vector<MemoryBlock> allocated_blocks_;
    
    // Thread safety
    std::mutex log_mutex_;
    std::mutex perf_mutex_;
    
    SrepContext(const srep_config_t* config);
    ~SrepContext();
    
    // Initialization
    srep_error_t initialize();
    srep_error_t validate_config();
    void derive_parameters();
    srep_error_t setup_hash();
    
    // Core operations
    srep_error_t compress_file(const char* input, const char* output);
    srep_error_t decompress_file(const char* input, const char* output);
    srep_error_t compress_memory(const void* input, size_t input_size,
                                 void* output, size_t* output_size);
    srep_error_t decompress_memory(const void* input, size_t input_size,
                                   void* output, size_t* output_size);
    srep_error_t get_info(const char* filename);
    
    // Memory management
    void* alloc(size_t size, srep_lp_type_t lp_mode);
    void free_all();
    
    // Error handling
    void set_error(srep_error_t error, const char* format, ...);
    void log_message(int level, const char* format, ...);
    
    // Performance counters
    void reset_perf_counters() {
        std::lock_guard<std::mutex> lock(perf_mutex_);
        memset(&perf_, 0, sizeof(perf_));
    }
    
    const srep_perf_counters_t* get_perf_counters() const {
        return &perf_;
    }
    
    // Increment performance counter (thread-safe)
    void perf_inc(uint64_t* counter, uint64_t value = 1) {
        std::lock_guard<std::mutex> lock(perf_mutex_);
        *counter += value;
    }
};

SrepContext::SrepContext(const srep_config_t* config) {
    if (config) {
        config_ = *config;
    } else {
        memset(&config_, 0, sizeof(config_));
        // Set defaults
        config_.method = SREP_METHOD3;
        config_.min_match = DEFAULT_MIN_MATCH;
        config_.chunk_size = 32;
        config_.buf_size = 8 * 1024 * 1024;
        config_.large_pages = SREP_LP_TRY;
        config_.hash_name = "vmac";
        config_.verbosity = 1;
        config_.stats_interval = 0.2;
    }
    
    last_error_ = SREP_NO_ERRORS;
    warnings_ = 0;
    hash_obj_ = nullptr;
    selected_hash_ = nullptr;
    hash_size_ = 0;
    memset(&perf_, 0, sizeof(perf_));
    
    inmem_compression_ = false;
    content_defined_chunking_ = false;
    zpaq_cdc_ = false;
    compare_digests_ = false;
    precompute_digests_ = false;
    round_matches_ = false;
    exhaustive_search_ = false;
    base_len_ = 0;
}

SrepContext::~SrepContext() {
    free_all();
    if (hash_obj_) {
        free(hash_obj_);
    }
}

srep_error_t SrepContext::initialize() {
    srep_error_t err = validate_config();
    if (err != SREP_NO_ERRORS) {
        return err;
    }
    
    derive_parameters();
    
    err = setup_hash();
    if (err != SREP_NO_ERRORS) {
        return err;
    }
    
    return SREP_NO_ERRORS;
}

srep_error_t SrepContext::validate_config() {
    // Validate method
    if (config_.method < SREP_METHOD_FIRST || config_.method > SREP_METHOD_LAST) {
        set_error(SREP_ERROR_CMDLINE, "Invalid method: %d", config_.method);
        return last_error_;
    }
    
    // Validate sizes
    if (config_.chunk_size == 0 && config_.method != SREP_METHOD5) {
        set_error(SREP_ERROR_CMDLINE, "Chunk size cannot be zero");
        return last_error_;
    }
    
    if (config_.min_match == 0) {
        set_error(SREP_ERROR_CMDLINE, "Minimum match length cannot be zero");
        return last_error_;
    }
    
    if (config_.min_match < MINIMAL_MIN_MATCH) {
        log_message(1, "Warning: min_match < %d may not compress well", MINIMAL_MIN_MATCH);
        warnings_++;
    }
    
    return SREP_NO_ERRORS;
}

void SrepContext::derive_parameters() {
    srep_method_t method = config_.method;
    
    inmem_compression_ = (method == SREP_METHOD0);
    content_defined_chunking_ = (method >= SREP_METHOD1 && method <= SREP_METHOD2);
    zpaq_cdc_ = (method == SREP_METHOD2);
    compare_digests_ = (method <= SREP_METHOD3);
    precompute_digests_ = (method == SREP_METHOD3);
    round_matches_ = (method == SREP_METHOD3) && (config_.dict_size == 0);
    exhaustive_search_ = (method == SREP_METHOD5);
    
    if (content_defined_chunking_) {
        base_len_ = config_.min_match;
        config_.min_match = 0;
    } else {
        base_len_ = std::min(config_.min_match, 
                            config_.dict_min_match > 0 ? config_.dict_min_match : config_.min_match);
    }
    
    // Adjust L for exhaustive search
    if (exhaustive_search_ && config_.chunk_size == 0) {
        unsigned power = 1;
        while (power < config_.min_match) power <<= 1;
        config_.chunk_size = power / 2;
    }
}

srep_error_t SrepContext::setup_hash() {
    // Get hash descriptors
    size_t count = 0;
    const srep_hash_descriptor_t* descriptors = srep_get_hash_descriptors(&count);
    
    // Find selected hash
    for (size_t i = 0; i < count; i++) {
        if (strcmp(descriptors[i].name, config_.hash_name) == 0) {
            selected_hash_ = &descriptors[i];
            hash_size_ = descriptors[i].hash_size;
            break;
        }
    }
    
    if (!selected_hash_ && config_.hash_name && *config_.hash_name) {
        set_error(SREP_ERROR_CMDLINE, "Unknown hash algorithm: %s", config_.hash_name);
        return last_error_;
    }
    
    // Allocate hash object if needed (for keyed hashes)
    if (selected_hash_ && selected_hash_->seed_size > 0) {
        hash_obj_ = malloc(selected_hash_->seed_size);
        if (!hash_obj_) {
            set_error(SREP_ERROR_MEMORY, "Cannot allocate hash object");
            return last_error_;
        }
        cryptographic_prng(hash_obj_, selected_hash_->seed_size);
    }
    
    return SREP_NO_ERRORS;
}

void* SrepContext::alloc(size_t size, srep_lp_type_t lp_mode) {
    // Simplified allocation - in real implementation would use BigAlloc
    void* ptr = malloc(size);
    if (!ptr) {
        set_error(SREP_ERROR_MEMORY, "Cannot allocate %zu bytes", size);
        return nullptr;
    }
    
    MemoryBlock block;
    block.ptr = ptr;
    block.size = size;
    block.large_page = false;
    allocated_blocks_.push_back(block);
    
    return ptr;
}

void SrepContext::free_all() {
    for (auto& block : allocated_blocks_) {
        if (block.ptr) {
            free(block.ptr);
        }
    }
    allocated_blocks_.clear();
}

void SrepContext::set_error(srep_error_t error, const char* format, ...) {
    last_error_ = error;
    
    char buffer[1024];
    va_list args;
    va_start(args, format);
    vsnprintf(buffer, sizeof(buffer), format, args);
    va_end(args);
    
    error_msg_ = buffer;
    
    if (config_.log_cb) {
        config_.log_cb(config_.log_user_data, 0, buffer);
    }
}

void SrepContext::log_message(int level, const char* format, ...) {
    if (!config_.log_cb || level > config_.verbosity) {
        return;
    }
    
    char buffer[1024];
    va_list args;
    va_start(args, format);
    vsnprintf(buffer, sizeof(buffer), format, args);
    va_end(args);
    
    std::lock_guard<std::mutex> lock(log_mutex_);
    config_.log_cb(config_.log_user_data, level, buffer);
}

srep_error_t SrepContext::compress_file(const char* input, const char* output) {
    set_error(SREP_ERROR_CMDLINE, "File compression not yet implemented in library version");
    return last_error_;
}

srep_error_t SrepContext::decompress_file(const char* input, const char* output) {
    set_error(SREP_ERROR_CMDLINE, "File decompression not yet implemented in library version");
    return last_error_;
}

srep_error_t SrepContext::compress_memory(const void* input, size_t input_size,
                                          void* output, size_t* output_size) {
    set_error(SREP_ERROR_CMDLINE, "Memory compression not yet implemented in library version");
    return last_error_;
}

srep_error_t SrepContext::decompress_memory(const void* input, size_t input_size,
                                            void* output, size_t* output_size) {
    set_error(SREP_ERROR_CMDLINE, "Memory decompression not yet implemented in library version");
    return last_error_;
}

srep_error_t SrepContext::get_info(const char* filename) {
    set_error(SREP_ERROR_CMDLINE, "Get info not yet implemented in library version");
    return last_error_;
}

//==============================================================================
// C API Implementation
//==============================================================================

// C-compatible wrapper
struct srep_ctx {
    std::unique_ptr<SrepContext> impl;
};

// Hash descriptors (from original code)
static const srep_hash_descriptor_t hash_descriptors[] = {
    {"md5", 0, 0, MD5_SIZE},
    {"", 1, 0, MD5_SIZE},
    {"sha1", 2, 0, SHA1_SIZE},
    {"sha512", 3, 0, SHA512_SIZE},
    {"vmac", 4, 32, 16},     // VMAC_KEY_LEN_BYTES, VMAC_TAG_LEN_BYTES
    {"siphash", 5, 16, 8},   // SIPHASH_KEY_LEN_BYTES, SIPHASH_TAG_LEN_BYTES
};

extern "C" {

void srep_config_init(srep_config_t* config) {
    if (!config) return;
    
    memset(config, 0, sizeof(*config));
    config->method = SREP_METHOD3;
    config->min_match = DEFAULT_MIN_MATCH;
    config->chunk_size = 32;
    config->buf_size = 8 * 1024 * 1024;
    config->large_pages = SREP_LP_TRY;
    config->hash_name = "vmac";
    config->verbosity = 1;
    config->stats_interval = 0.2;
}

srep_error_t srep_init(srep_ctx_t** ctx, const srep_config_t* config) {
    if (!ctx) {
        return SREP_ERROR_CMDLINE;
    }
    
    *ctx = new srep_ctx_t;
    (*ctx)->impl = std::make_unique<SrepContext>(config);
    
    srep_error_t err = (*ctx)->impl->initialize();
    if (err != SREP_NO_ERRORS) {
        delete *ctx;
        *ctx = nullptr;
    }
    
    return err;
}

srep_error_t srep_compress_file(srep_ctx_t* ctx, const char* input, const char* output) {
    if (!ctx || !ctx->impl || !input || !output) {
        return SREP_ERROR_CMDLINE;
    }
    return ctx->impl->compress_file(input, output);
}

srep_error_t srep_decompress_file(srep_ctx_t* ctx, const char* input, const char* output) {
    if (!ctx || !ctx->impl || !input || !output) {
        return SREP_ERROR_CMDLINE;
    }
    return ctx->impl->decompress_file(input, output);
}

srep_error_t srep_compress_memory(srep_ctx_t* ctx, const void* input, size_t input_size,
                                  void* output, size_t* output_size) {
    if (!ctx || !ctx->impl || !input || !output || !output_size) {
        return SREP_ERROR_CMDLINE;
    }
    return ctx->impl->compress_memory(input, input_size, output, output_size);
}

srep_error_t srep_decompress_memory(srep_ctx_t* ctx, const void* input, size_t input_size,
                                    void* output, size_t* output_size) {
    if (!ctx || !ctx->impl || !input || !output || !output_size) {
        return SREP_ERROR_CMDLINE;
    }
    return ctx->impl->decompress_memory(input, input_size, output, output_size);
}

srep_error_t srep_compress_stream(srep_ctx_t* ctx) {
    if (!ctx || !ctx->impl) {
        return SREP_ERROR_CMDLINE;
    }
    // Would use callbacks from config
    return SREP_ERROR_CMDLINE;
}

srep_error_t srep_decompress_stream(srep_ctx_t* ctx) {
    if (!ctx || !ctx->impl) {
        return SREP_ERROR_CMDLINE;
    }
    // Would use callbacks from config
    return SREP_ERROR_CMDLINE;
}

srep_error_t srep_get_info(srep_ctx_t* ctx, const char* filename) {
    if (!ctx || !ctx->impl || !filename) {
        return SREP_ERROR_CMDLINE;
    }
    return ctx->impl->get_info(filename);
}

const srep_perf_counters_t* srep_get_perf_counters(const srep_ctx_t* ctx) {
    if (!ctx || !ctx->impl) {
        return nullptr;
    }
    return ctx->impl->get_perf_counters();
}

void srep_reset_perf_counters(srep_ctx_t* ctx) {
    if (ctx && ctx->impl) {
        ctx->impl->reset_perf_counters();
    }
}

const srep_hash_descriptor_t* srep_get_hash_descriptors(size_t* count) {
    if (count) {
        *count = sizeof(hash_descriptors) / sizeof(hash_descriptors[0]);
    }
    return hash_descriptors;
}

const char* srep_error_string(srep_error_t error) {
    switch (error) {
        case SREP_NO_ERRORS: return "No errors";
        case SREP_WARNINGS: return "Warnings";
        case SREP_ERROR_CMDLINE: return "Command line error";
        case SREP_ERROR_IO: return "I/O error";
        case SREP_ERROR_COMPRESSION: return "Compression error";
        case SREP_ERROR_MEMORY: return "Memory error";
        default: return "Unknown error";
    }
}

const char* srep_get_last_error_msg(const srep_ctx_t* ctx) {
    if (!ctx || !ctx->impl) {
        return "";
    }
    return ctx->impl->error_msg_.c_str();
}

void srep_free(srep_ctx_t* ctx) {
    delete ctx;
}

const char* srep_version(void) {
    return "SREP 3.93 beta (library)";
}

const char* srep_date(void) {
    return "August 3, 2013 (refactored 2026)";
}

} // extern "C"
