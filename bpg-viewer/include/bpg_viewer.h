/*
 * BPG Viewer and Thumbnail Library - C API
 *
 * This header provides a C-compatible FFI interface for embedding
 * the BPG viewer library in other languages and applications.
 */

#ifndef BPG_VIEWER_H
#define BPG_VIEWER_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stddef.h>

/* Error codes */
typedef enum {
    BPG_VIEWER_SUCCESS = 0,
    BPG_VIEWER_INVALID_PARAM = -1,
    BPG_VIEWER_DECODE_FAILED = -2,
    BPG_VIEWER_ENCODE_FAILED = -3,
    BPG_VIEWER_OUT_OF_MEMORY = -4,
    BPG_VIEWER_IO_ERROR = -5
} BPGViewerError;

/* Opaque handles */
typedef struct BPGImageHandle BPGImageHandle;
typedef struct BPGThumbnailHandle BPGThumbnailHandle;
typedef struct UniversalThumbnailHandle UniversalThumbnailHandle;
typedef struct UniversalImageHandle UniversalImageHandle;

/*
 * Image Decoding Functions
 */

/* Decode a BPG file and return a handle to the decoded image
 * Returns NULL on failure
 */
BPGImageHandle* bpg_viewer_decode_file(const char* path);

/* Get image dimensions from handle
 * Returns BPG_VIEWER_SUCCESS on success
 */
int bpg_viewer_get_dimensions(
    const BPGImageHandle* handle,
    uint32_t* width,
    uint32_t* height
);

/* Get image data pointer and size
 * The returned pointer is valid as long as the handle exists
 * Returns BPG_VIEWER_SUCCESS on success
 */
int bpg_viewer_get_data(
    const BPGImageHandle* handle,
    const uint8_t** data,
    size_t* size
);

/* Get RGBA32 data from image (performs conversion if needed)
 * Caller must free the returned pointer with bpg_viewer_free_buffer
 * Returns BPG_VIEWER_SUCCESS on success
 */
int bpg_viewer_get_rgba32(
    const BPGImageHandle* handle,
    uint8_t** data,
    size_t* size
);

/* Free buffer allocated by bpg_viewer_get_rgba32 */
void bpg_viewer_free_buffer(uint8_t* ptr, size_t size);

/* Free decoded image handle */
void bpg_viewer_free_image(BPGImageHandle* handle);

/*
 * Thumbnail Generation Functions
 */

/* Create a thumbnail generator with default settings (256x256) */
BPGThumbnailHandle* bpg_thumbnail_create(void);

/* Create a thumbnail generator with specific dimensions */
BPGThumbnailHandle* bpg_thumbnail_create_with_size(
    uint32_t max_width,
    uint32_t max_height
);

/* Generate thumbnail and save as PNG
 * Returns BPG_VIEWER_SUCCESS on success
 */
int bpg_thumbnail_generate_png(
    const BPGThumbnailHandle* handle,
    const char* input_path,
    const char* output_path
);

/* Free thumbnail generator handle */
void bpg_thumbnail_free(BPGThumbnailHandle* handle);

/*
 * Utility Functions
 */

/* Get library version string */
const char* bpg_viewer_version(void);

/*
 * Universal Thumbnail Generation Functions
 * Supports BPG, HEIC, RAW, DNG, JPEG2000, and standard image formats
 */

/* Create a universal thumbnail generator with default settings (256x256) */
UniversalThumbnailHandle* universal_thumbnail_create(void);

/* Create a universal thumbnail generator with specific dimensions */
UniversalThumbnailHandle* universal_thumbnail_create_with_size(
    uint32_t max_width,
    uint32_t max_height
);

/* Generate thumbnail for any supported format and save as PNG
 * Returns BPG_VIEWER_SUCCESS on success
 */
int universal_thumbnail_generate_png(
    const UniversalThumbnailHandle* handle,
    const char* input_path,
    const char* output_path
);

/* Check if a file format is supported by the universal thumbnail generator
 * Returns 1 if supported, 0 otherwise
 */
int universal_thumbnail_is_supported(const char* file_path);

/* Free universal thumbnail generator handle */
void universal_thumbnail_free(UniversalThumbnailHandle* handle);

/*
 * Universal Image Decoding Functions (Full Resolution BGRA)
 * Supports BPG, HEIC, RAW, DNG, JPEG2000, and standard image formats
 */

/* Decode any supported image file to full resolution BGRA
 * Returns NULL on failure
 */
UniversalImageHandle* universal_image_decode_file(const char* path);

/* Get image dimensions from universal image handle
 * Returns BPG_VIEWER_SUCCESS on success
 */
int universal_image_get_dimensions(
    const UniversalImageHandle* handle,
    uint32_t* width,
    uint32_t* height
);

/* Copy BGRA data to a provided buffer (e.g. WPF WriteableBitmap)
 * Buffer must be at least stride * height bytes
 * Returns BPG_VIEWER_SUCCESS on success
 */
int universal_image_copy_to_buffer(
    const UniversalImageHandle* handle,
    uint8_t* buffer,
    size_t buffer_size,
    size_t stride
);

/* Get BGRA data pointer and size from universal image handle
 * The returned pointer is valid as long as the handle exists
 * Returns BPG_VIEWER_SUCCESS on success
 */
int universal_image_get_data(
    const UniversalImageHandle* handle,
    const uint8_t** data,
    size_t* size
);

/* Check if a file format is supported by the universal image decoder
 * Returns 1 if supported, 0 otherwise
 */
int universal_image_is_supported(const char* file_path);

/* Free universal image handle */
void universal_image_free(UniversalImageHandle* handle);

#ifdef __cplusplus
}
#endif

#endif /* BPG_VIEWER_H */
