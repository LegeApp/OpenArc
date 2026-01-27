/*
 * FreeARC C-compatible wrapper header for Rust FFI
 * This header defines the interface to FreeARC's compression/decompression functions
 */

#ifndef FREEARC_WRAPPER_H
#define FREEARC_WRAPPER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// GRZip functions
int32_t freearc_grzip_decompress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size
);

int32_t freearc_grzip_compress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t mode
);

// Tornado functions
int32_t freearc_tornado_decompress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size
);

int32_t freearc_tornado_compress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t method_number
);

// PPMD functions
int32_t freearc_ppmd_decompress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t order,
    int32_t memory_size
);

int32_t freearc_ppmd_compress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t order,
    int32_t memory_size
);

// LZP functions
int32_t freearc_lzp_decompress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t min_match_len,
    int32_t hash_size
);

int32_t freearc_lzp_compress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t min_match_len,
    int32_t hash_size
);

// LZMA2 functions
int32_t freearc_lzma2_decompress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size
);

int32_t freearc_lzma2_compress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t compression_level,
    uint32_t dict_size,
    uint32_t lc,
    uint32_t lp,
    uint32_t pb
);

// Utility functions for memory management
void* freearc_big_alloc(int32_t size);
void freearc_big_free(void* ptr);

// Set global parameters
void freearc_set_threads(int32_t num_threads);
int32_t freearc_get_threads();

#ifdef __cplusplus
}
#endif

#endif // FREEARC_WRAPPER_H