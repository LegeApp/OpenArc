/*
 * FreeARC C-compatible wrapper for Rust FFI
 * This file provides a clean C interface to FreeARC's compression/decompression functions
 */

#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "Compression/Compression.h"
#include "Compression/GRZip/C_GRZip.h"
// Tornado header excluded - uses C++ types that can't be extern "C"
// We use the COMPRESSION_METHOD class approach instead
#include "Compression/PPMD/C_PPMD.h"
#include "Compression/LZP/C_LZP.h"
#include "Compression/Dict/C_Dict.h"
#include "Compression/Delta/C_Delta.h"
#include "Compression/LZ4/C_LZ4.h"
#include "TornadoImprovements.h"

// Additional forward declarations for missing functions
extern void* PPM_CreateDecompressContext(int order, int memory_size);
extern void PPM_FreeDecompressContext(void* ctx);
extern int GRZip_LZP_Decode(uint8_t* input, int input_size, uint8_t* output, int min_match_len, int hash_size);

// PPM_CONTEXT type definition (from PPMD/Model.cpp)
typedef struct PPM_CONTEXT PPM_CONTEXT;

// Forward declare parse functions with C linkage to match staged objects
extern "C" COMPRESSION_METHOD* parse_TORNADO(char** parameters);
extern "C" COMPRESSION_METHOD* parse_GRZIP(char** parameters);

// Callback structure for I/O operations (used internally)
typedef struct {
    const uint8_t* input_data;
    int32_t input_size;
    int32_t input_pos;

    uint8_t* output_data;
    int32_t output_size;
    int32_t output_pos;

    int32_t expected_output_size;
} CallbackData;

// I/O callback function for FreeARC
int callback_func(char* operation, void* buffer, int size, void* auxdata) {
    CallbackData* cb_data = (CallbackData*)auxdata;

    if (strcmp(operation, "read") == 0) {
        // Reading from input
        int bytes_available = cb_data->input_size - cb_data->input_pos;
        int bytes_to_read = (size < bytes_available) ? size : bytes_available;

        if (bytes_to_read <= 0) {
            return 0; // EOF
        }

        memcpy(buffer, cb_data->input_data + cb_data->input_pos, bytes_to_read);
        cb_data->input_pos += bytes_to_read;
        return bytes_to_read;
    }
    else if (strcmp(operation, "write") == 0) {
        // Writing to output
        int bytes_available = cb_data->output_size - cb_data->output_pos;
        if (size > bytes_available) {
            // Must not do partial writes: FreeArc expects either full write or a negative error code
            return FREEARC_ERRCODE_OUTBLOCK_TOO_SMALL;
        }

        memcpy(cb_data->output_data + cb_data->output_pos, buffer, size);
        cb_data->output_pos += size;
        return size;
    }
    
    // Ignore other operations (progress, etc.)
    return 0;
}

// Wrapper functions to fix callback signature mismatches
static int callback_func_wrapper(const char* operation, void* buffer, int size, void* auxdata) {
    return callback_func((char*)operation, buffer, size, auxdata);
}

// Export wrapper functions with C linkage for Rust FFI
#ifdef __cplusplus
extern "C" {
#endif

// LZMA2 functions
int32_t freearc_lzma2_decompress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    uint32_t dict_size,
    uint32_t lc,
    uint32_t lp,
    uint32_t pb
) {
    char s_dict[32], s_lc[32], s_lp[32], s_pb[32];
    snprintf(s_dict, sizeof(s_dict), "d%ub", dict_size);
    snprintf(s_lc, sizeof(s_lc), "lc%d", lc);
    snprintf(s_lp, sizeof(s_lp), "lp%d", lp);
    snprintf(s_pb, sizeof(s_pb), "pb%d", pb);

    char *args[] = { (char*)"lzma", s_dict, s_lc, s_lp, s_pb, NULL };
    COMPRESSION_METHOD *c = parse_LZMA(args);
    if (!c) {
        printf("DEBUG: freearc_lzma2_decompress: parse_LZMA returned NULL\n");
        return FREEARC_ERRCODE_INVALID_COMPRESSOR;
    }
    
    // DeCompressMem expects int* for outputSize
    int result = c->DeCompressMem(DECOMPRESS, input, input_size, output, &output_size, callback_func_wrapper, NULL, NULL);
    
    delete c;
    return (result == FREEARC_OK) ? output_size : result;
}

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
) {
    char s_dict[32], s_lc[32], s_lp[32], s_pb[32];
    snprintf(s_dict, sizeof(s_dict), "d%ub", dict_size);
    snprintf(s_lc, sizeof(s_lc), "lc%d", lc);
    snprintf(s_lp, sizeof(s_lp), "lp%d", lp);
    snprintf(s_pb, sizeof(s_pb), "pb%d", pb);

    char *args[] = { (char*)"lzma", s_dict, s_lc, s_lp, s_pb, NULL };
    COMPRESSION_METHOD *c = parse_LZMA(args);
    if (!c) {
        printf("DEBUG: freearc_lzma2_compress: parse_LZMA returned NULL\n");
        return FREEARC_ERRCODE_INVALID_COMPRESSOR;
    }

    // DeCompressMem expects int* for outputSize
    int result = c->DeCompressMem(COMPRESS, input, input_size, output, &output_size, callback_func_wrapper, NULL, NULL);
    
    delete c;
    return (result == FREEARC_OK) ? output_size : result;
}

// PPMD functions
// Note: memory_size is size_t (MemSize) which is 64-bit on x64
int32_t freearc_ppmd_decompress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t order,
    size_t memory_size
) {
    fprintf(stderr, "WRAP DEBUG: freearc_ppmd_decompress enter input_size=%d output_size=%d order=%d memory_size=%zu\n",
            input_size, output_size, order, memory_size);
    fflush(stderr);
    CallbackData cb_data;
    cb_data.input_data = input;
    cb_data.input_size = input_size;
    cb_data.input_pos = 0;
    cb_data.output_data = output;
    cb_data.output_size = output_size;
    cb_data.output_pos = 0;
    cb_data.expected_output_size = output_size;

    // Use ppmd_decompress2 with order and memory parameters
    // ENCODE=FALSE for decompression (DecodeFile)
    fprintf(stderr, "WRAP DEBUG: calling ppmd_decompress2\n");
    fflush(stderr);
    int result = ppmd_decompress2(FALSE, order, memory_size, 0, 0, callback_func_wrapper, &cb_data);
    fprintf(stderr, "WRAP DEBUG: ppmd_decompress2 returned %d (out_pos=%d)\n", result, cb_data.output_pos);
    fflush(stderr);
    return (result == FREEARC_OK) ? cb_data.output_pos : result;
}

int32_t freearc_ppmd_compress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t order,
    size_t memory_size
) {
    fprintf(stderr, "WRAP DEBUG: freearc_ppmd_compress enter input_size=%d output_size=%d order=%d memory_size=%zu\n",
            input_size, output_size, order, memory_size);
    fflush(stderr);
    CallbackData cb_data;
    cb_data.input_data = input;
    cb_data.input_size = input_size;
    cb_data.input_pos = 0;
    cb_data.output_data = output;
    cb_data.output_size = output_size;
    cb_data.output_pos = 0;
    cb_data.expected_output_size = output_size;

    // Use ppmd_compress2 with order and memory parameters
    // ENCODE=TRUE for compression (EncodeFile), FALSE for decompression (DecodeFile)
    fprintf(stderr, "WRAP DEBUG: calling ppmd_compress2\n");
    fflush(stderr);
    int result = ppmd_compress2(TRUE, order, memory_size, 0, 0, callback_func_wrapper, &cb_data);
    fprintf(stderr, "WRAP DEBUG: ppmd_compress2 returned %d (out_pos=%d)\n", result, cb_data.output_pos);
    fflush(stderr);
    return (result == FREEARC_OK) ? cb_data.output_pos : result;
}

// LZP functions
int32_t freearc_lzp_decompress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t min_match_len,
    int32_t hash_size
) {
    CallbackData cb_data;
    cb_data.input_data = input;
    cb_data.input_size = input_size;
    cb_data.input_pos = 0;
    cb_data.output_data = output;
    cb_data.output_size = output_size;
    cb_data.output_pos = 0;
    cb_data.expected_output_size = output_size;

    // Default block size 8MB, min compression 100%
    int result = lzp_decompress(8*1024*1024, 100, min_match_len, hash_size, INT32_MAX, 2, callback_func_wrapper, &cb_data);
    return (result == FREEARC_OK) ? cb_data.output_pos : result;
}

int32_t freearc_lzp_compress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t min_match_len,
    int32_t hash_size
) {
    CallbackData cb_data;
    cb_data.input_data = input;
    cb_data.input_size = input_size;
    cb_data.input_pos = 0;
    cb_data.output_data = output;
    cb_data.output_size = output_size;
    cb_data.output_pos = 0;
    cb_data.expected_output_size = output_size;

    // Default block size 8MB, min compression 100%
    int result = lzp_compress(8*1024*1024, 100, min_match_len, hash_size, INT32_MAX, 2, callback_func_wrapper, &cb_data);
    return (result == FREEARC_OK) ? cb_data.output_pos : result;
}

// Tornado functions
int32_t freearc_tornado_decompress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size
) {
    CallbackData cb_data;
    cb_data.input_data = input;
    cb_data.input_size = input_size;
    cb_data.input_pos = 0;
    cb_data.output_data = output;
    cb_data.output_size = output_size;
    cb_data.output_pos = 0;
    cb_data.expected_output_size = output_size;

    // Use COMPRESSION_METHOD class approach
    char* params[] = { (char*)"tor", NULL };
    COMPRESSION_METHOD* method = parse_TORNADO(params);
    if (!method) {
        return FREEARC_ERRCODE_INVALID_COMPRESSOR;
    }

    int result = method->decompress(callback_func_wrapper, &cb_data);
    delete method;
    return (result == FREEARC_OK) ? cb_data.output_pos : result;
}

int32_t freearc_tornado_compress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t method_number
) {
    CallbackData cb_data;
    cb_data.input_data = input;
    cb_data.input_size = input_size;
    cb_data.input_pos = 0;
    cb_data.output_data = output;
    cb_data.output_size = output_size;
    cb_data.output_pos = 0;
    cb_data.expected_output_size = output_size;

    // Use COMPRESSION_METHOD class approach with method number
    // parse_TORNADO expects tokenized parameters: ["tor", "<num>", ...]
    char num_str[16];
    snprintf(num_str, sizeof(num_str), "%d", method_number);
    char* params[] = { (char*)"tor", num_str, NULL };
    COMPRESSION_METHOD* method = parse_TORNADO(params);
    if (!method) {
        return FREEARC_ERRCODE_INVALID_COMPRESSOR;
    }

    int result = method->compress(callback_func_wrapper, &cb_data);
    delete method;
    return (result == FREEARC_OK) ? cb_data.output_pos : result;
}

// GRZip functions
int32_t freearc_grzip_decompress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size
) {
    if (input_size < 28) {
        return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
    }

    // Input[0..3] stores original size in GRZip blocks
    int32_t original_size = *(int32_t*)input;
    if (original_size < 0) {
        return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
    }
    if (original_size > output_size) {
        return FREEARC_ERRCODE_OUTBLOCK_TOO_SMALL;
    }

    int result = GRZip_DecompressBlock((uint8*)input, input_size, (uint8*)output);
    if (result < 0) {
        return (result == GRZ_NOT_ENOUGH_MEMORY) ? FREEARC_ERRCODE_NOT_ENOUGH_MEMORY
                                                 : FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
    }
    return result;
}

int32_t freearc_grzip_compress(
    uint8_t* input,
    int32_t input_size,
    uint8_t* output,
    int32_t output_size,
    int32_t block_size
) {
    // GRZip output always includes 28-byte header, and for stored blocks it's input_size + 28.
    if (input_size < 0) {
        return FREEARC_ERRCODE_GENERAL;
    }
    if (output_size < input_size + 28) {
        return FREEARC_ERRCODE_OUTBLOCK_TOO_SMALL;
    }

    // Note: Rust passes 'mode' in this parameter.
    int result = GRZip_CompressBlock((uint8*)input, input_size, (uint8*)output, block_size);
    if (result < 0) {
        return (result == GRZ_NOT_ENOUGH_MEMORY) ? FREEARC_ERRCODE_NOT_ENOUGH_MEMORY
                                                 : FREEARC_ERRCODE_GENERAL;
    }
    return result;
}

// Utility functions for memory management
void* freearc_big_alloc(int32_t size) {
    return BigAlloc(size);
}

void freearc_big_free(void* ptr) {
    BigFree(ptr);
}

// Set global parameters
void freearc_set_threads(int32_t num_threads) {
    SetCompressionThreads(num_threads);
}

int32_t freearc_get_threads() {
    return GetCompressionThreads();
}

#ifdef __cplusplus
}
#endif