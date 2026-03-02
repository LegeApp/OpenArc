// Example usage of SREP library
#include "srep_lib.h"
#include <stdio.h>
#include <stdlib.h>

// Example log callback
static void log_callback(void* user_data, int level, const char* message) {
    FILE* log_file = (FILE*)user_data;
    const char* level_str;
    
    switch (level) {
        case 0: level_str = "ERROR"; break;
        case 1: level_str = "WARN"; break;
        case 2: level_str = "INFO"; break;
        default: level_str = "DEBUG"; break;
    }
    
    fprintf(log_file, "[%s] %s\n", level_str, message);
    fflush(log_file);
}

int main(int argc, char** argv) {
    if (argc < 3) {
        fprintf(stderr, "Usage: %s <input> <output>\n", argv[0]);
        fprintf(stderr, "Example compression using SREP library\n");
        return 1;
    }
    
    printf("SREP Library Example\n");
    printf("Version: %s\n", srep_version());
    printf("Date: %s\n\n", srep_date());
    
    // Initialize configuration with defaults
    srep_config_t config;
    srep_config_init(&config);
    
    // Customize configuration
    config.method = SREP_METHOD3;           // Precompute digests method
    config.min_match = 32;                  // Minimum match length
    config.chunk_size = 32;                 // Chunk size for hashing
    config.buf_size = 8 * 1024 * 1024;      // 8MB buffer
    config.large_pages = SREP_LP_TRY;       // Try to use large pages
    config.hash_name = "vmac";              // Use VMAC hash
    config.verbosity = 2;                   // Verbose output
    config.print_counters = 1;              // Print performance counters
    config.stats_interval = 0.5;            // Update stats every 0.5 seconds
    config.num_threads = 4;                 // Use 4 threads
    
    // Set up logging
    config.log_cb = log_callback;
    config.log_user_data = stderr;
    
    // Create SREP context
    srep_ctx_t* ctx = nullptr;
    srep_error_t err = srep_init(&ctx, &config);
    if (err != SREP_NO_ERRORS) {
        fprintf(stderr, "Failed to initialize SREP: %s\n", srep_error_string(err));
        return 1;
    }
    
    printf("Compressing %s -> %s\n", argv[1], argv[2]);
    
    // Perform compression
    err = srep_compress_file(ctx, argv[1], argv[2]);
    if (err != SREP_NO_ERRORS) {
        fprintf(stderr, "Compression failed: %s\n", srep_error_string(err));
        fprintf(stderr, "Details: %s\n", srep_get_last_error_msg(ctx));
        srep_free(ctx);
        return 1;
    }
    
    // Print performance counters if enabled
    if (config.print_counters) {
        const srep_perf_counters_t* pc = srep_get_perf_counters(ctx);
        printf("\nPerformance Counters:\n");
        printf("  Matches found: %llu\n", (unsigned long long)pc->find_match);
        printf("  Hash array checks: %llu\n", (unsigned long long)pc->check_hasharr);
        printf("  Hash hits: %llu\n", (unsigned long long)pc->hash_found);
        printf("  Length checks: %llu\n", (unsigned long long)pc->check_len);
        printf("  Matches recorded: %llu\n", (unsigned long long)pc->record_match);
        printf("  Total match length: %llu\n", (unsigned long long)pc->total_match_len);
        printf("  Max offset: %llu\n", (unsigned long long)pc->max_offset);
    }
    
    // Clean up
    srep_free(ctx);
    
    printf("\nCompression completed successfully!\n");
    return 0;
}

// Example of using the library with multiple concurrent contexts (thread-safe)
#ifdef EXAMPLE_MULTITHREADED
#include <thread>
#include <vector>

void compress_file_in_thread(const char* input, const char* output, int thread_id) {
    srep_config_t config;
    srep_config_init(&config);
    config.verbosity = 1;
    config.method = SREP_METHOD3;
    
    srep_ctx_t* ctx = nullptr;
    srep_error_t err = srep_init(&ctx, &config);
    if (err != SREP_NO_ERRORS) {
        fprintf(stderr, "Thread %d: Failed to init: %s\n", thread_id, srep_error_string(err));
        return;
    }
    
    err = srep_compress_file(ctx, input, output);
    if (err != SREP_NO_ERRORS) {
        fprintf(stderr, "Thread %d: Compression failed: %s\n", 
                thread_id, srep_error_string(err));
    } else {
        printf("Thread %d: Compression successful\n", thread_id);
    }
    
    srep_free(ctx);
}

void example_multithreaded() {
    const int NUM_FILES = 4;
    std::vector<std::thread> threads;
    
    for (int i = 0; i < NUM_FILES; i++) {
        char input[256], output[256];
        snprintf(input, sizeof(input), "input%d.bin", i);
        snprintf(output, sizeof(output), "output%d.srep", i);
        
        threads.emplace_back(compress_file_in_thread, input, output, i);
    }
    
    for (auto& t : threads) {
        t.join();
    }
    
    printf("All compressions completed\n");
}
#endif

// Example of memory-based compression/decompression
#ifdef EXAMPLE_MEMORY
void example_memory_compression() {
    // Prepare input data
    const size_t input_size = 1024 * 1024; // 1MB
    void* input = malloc(input_size);
    memset(input, 'A', input_size); // Fill with repeated data
    
    // Prepare output buffer
    size_t output_size = input_size * 2; // Allocate enough space
    void* output = malloc(output_size);
    
    // Configure and initialize
    srep_config_t config;
    srep_config_init(&config);
    config.method = SREP_METHOD0; // In-memory compression
    
    srep_ctx_t* ctx = nullptr;
    srep_error_t err = srep_init(&ctx, &config);
    if (err != SREP_NO_ERRORS) {
        fprintf(stderr, "Init failed: %s\n", srep_error_string(err));
        goto cleanup;
    }
    
    // Compress
    err = srep_compress_memory(ctx, input, input_size, output, &output_size);
    if (err != SREP_NO_ERRORS) {
        fprintf(stderr, "Compression failed: %s\n", srep_error_string(err));
        goto cleanup;
    }
    
    printf("Compressed %zu bytes to %zu bytes (%.2f%%)\n",
           input_size, output_size, 
           (double)output_size * 100.0 / input_size);
    
cleanup:
    free(input);
    free(output);
    srep_free(ctx);
}
#endif
