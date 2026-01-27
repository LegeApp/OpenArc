/*
 * Example C program using the BPG Viewer FFI
 *
 * Compile with:
 *   gcc -o ffi_example ffi_usage.c -L../target/release -lbpg_viewer -lm -lpthread
 */

#include "../include/bpg_viewer.h"
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <image.bpg>\n", argv[0]);
        return 1;
    }

    const char* input_path = argv[1];

    printf("BPG Viewer C FFI Example\n");
    printf("Library version: %s\n\n", bpg_viewer_version());

    /* Decode the image */
    printf("Decoding: %s\n", input_path);
    BPGImageHandle* img = bpg_viewer_decode_file(input_path);

    if (!img) {
        fprintf(stderr, "Error: Failed to decode image\n");
        return 1;
    }

    /* Get dimensions */
    uint32_t width, height;
    int result = bpg_viewer_get_dimensions(img, &width, &height);

    if (result != BPG_VIEWER_SUCCESS) {
        fprintf(stderr, "Error: Failed to get dimensions\n");
        bpg_viewer_free_image(img);
        return 1;
    }

    printf("Image dimensions: %ux%u\n", width, height);

    /* Get RGBA data */
    uint8_t* rgba_data = NULL;
    size_t rgba_size = 0;

    result = bpg_viewer_get_rgba32(img, &rgba_data, &rgba_size);

    if (result != BPG_VIEWER_SUCCESS) {
        fprintf(stderr, "Error: Failed to get RGBA data\n");
        bpg_viewer_free_image(img);
        return 1;
    }

    printf("RGBA data size: %zu bytes\n", rgba_size);
    printf("Expected size: %zu bytes\n", (size_t)(width * height * 4));

    /* Sample first pixel */
    if (rgba_size >= 4) {
        printf("First pixel (RGBA): %u, %u, %u, %u\n",
               rgba_data[0], rgba_data[1], rgba_data[2], rgba_data[3]);
    }

    /* Cleanup */
    bpg_viewer_free_buffer(rgba_data, rgba_size);
    bpg_viewer_free_image(img);

    /* Thumbnail example */
    printf("\nGenerating thumbnail...\n");

    BPGThumbnailHandle* thumb = bpg_thumbnail_create_with_size(256, 256);

    if (!thumb) {
        fprintf(stderr, "Error: Failed to create thumbnail generator\n");
        return 1;
    }

    result = bpg_thumbnail_generate_png(thumb, input_path, "thumb_output.png");

    if (result == BPG_VIEWER_SUCCESS) {
        printf("Thumbnail saved to: thumb_output.png\n");
    } else {
        fprintf(stderr, "Error: Failed to generate thumbnail\n");
    }

    bpg_thumbnail_free(thumb);

    printf("\nDone!\n");
    return 0;
}
