/*
 * BPG Native Library API
 * 
 * Provides a C API for BPG encoding/decoding without subprocess overhead.
 * Designed for FFI integration with Rust and other languages.
 */

#ifndef BPG_API_H
#define BPG_API_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque encoder context */
typedef struct BPGEncoderContext BPGEncoderContext;

/* Encoder configuration */
typedef struct {
    int quality;              /* 0-51, lower is better quality (default: 28) */
    int bit_depth;            /* 8, 10, or 12 bits per component */
    int lossless;             /* 1 for lossless, 0 for lossy */
    int chroma_format;        /* 0=grayscale, 1=4:2:0, 2=4:2:2, 3=4:4:4 */
    int encoder_type;         /* 0=x265, 1=JCTVC (if available) */
    int compress_level;       /* 1-9, compression effort (default: 8) */
} BPGEncoderConfig;

/* Error codes */
typedef enum {
    BPG_OK = 0,
    BPG_ERROR_INVALID_PARAM = -1,
    BPG_ERROR_OUT_OF_MEMORY = -2,
    BPG_ERROR_UNSUPPORTED_FORMAT = -3,
    BPG_ERROR_ENCODE_FAILED = -4,
    BPG_ERROR_DECODE_FAILED = -5,
    BPG_ERROR_FILE_IO = -6,
    BPG_ERROR_INVALID_IMAGE = -7,
} BPGError;

/* Image format for input data */
typedef enum {
    BPG_INPUT_FORMAT_GRAY = 0,
    BPG_INPUT_FORMAT_RGB24,
    BPG_INPUT_FORMAT_RGBA32,
    BPG_INPUT_FORMAT_BGR24,
    BPG_INPUT_FORMAT_BGRA32,
    BPG_INPUT_FORMAT_YCbCr_420P,  /* Planar YCbCr 4:2:0 (JPEG native) */
    BPG_INPUT_FORMAT_YCbCr_444P,  /* Planar YCbCr 4:4:4 */
} BPGImageFormat;

/*
 * Encoder Functions
 */

/* Create encoder with default configuration */
BPGEncoderContext* bpg_encoder_create(void);

/* Create encoder with custom configuration */
BPGEncoderContext* bpg_encoder_create_ex(const BPGEncoderConfig* config);

/* Set encoder configuration (can be called before encoding) */
int bpg_encoder_set_config(BPGEncoderContext* ctx, const BPGEncoderConfig* config);

/* Get default configuration */
void bpg_encoder_get_default_config(BPGEncoderConfig* config);

/* 
 * Encode image from file (PNG, JPEG, etc.)
 * Returns BPG_OK on success, error code on failure.
 * Output buffer is allocated by this function and must be freed with bpg_free().
 */
int bpg_encode_from_file(
    BPGEncoderContext* ctx,
    const char* input_path,
    uint8_t** output_data,
    size_t* output_size
);

/*
 * Encode image from memory buffer
 * Input data must be in the specified format.
 * Returns BPG_OK on success, error code on failure.
 * Output buffer is allocated by this function and must be freed with bpg_free().
 */
int bpg_encode_from_memory(
    BPGEncoderContext* ctx,
    const uint8_t* input_data,
    int width,
    int height,
    int stride,
    BPGImageFormat format,
    uint8_t** output_data,
    size_t* output_size
);

/*
 * Encode image to file
 * Returns BPG_OK on success, error code on failure.
 */
int bpg_encode_to_file(
    BPGEncoderContext* ctx,
    const char* input_path,
    const char* output_path
);

/* Get last error message */
const char* bpg_encoder_get_error(BPGEncoderContext* ctx);

/* Destroy encoder context */
void bpg_encoder_destroy(BPGEncoderContext* ctx);

/*
 * Decoder Functions (using existing libbpg)
 */

/* Decode BPG file to memory (existing libbpg function) */
int bpg_decode_file(
    const char* input_path,
    uint8_t** output_data,
    int* width,
    int* height,
    BPGImageFormat* format
);

/*
 * Memory Management
 */

/* Free memory allocated by BPG API */
void bpg_free(void* ptr);

/*
 * Version Information
 */

/* Get library version string */
const char* bpg_get_version(void);

/* Get supported encoders (bitmask: bit 0=x265, bit 1=JCTVC) */
int bpg_get_supported_encoders(void);

#ifdef __cplusplus
}
#endif

#endif /* BPG_API_H */
