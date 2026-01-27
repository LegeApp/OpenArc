#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct Option_ProgressCallback Option_ProgressCallback;

/**
 * Compression settings matching CLI options from openarc-core OrchestratorSettings.
 */
typedef struct CompressionSettings {
  int bpg_quality;
  bool bpg_lossless;
  int bpg_bit_depth;
  int bpg_chroma_format;
  int bpg_encoder_type;
  int bpg_compression_level;
  int video_codec;
  int video_speed;
  int video_crf;
  int compression_level;
  bool enable_catalog;
  bool enable_dedup;
  bool skip_already_compressed_videos;
} CompressionSettings;

/**
 * Extraction settings for FFI
 */
typedef struct ExtractionSettings {
  /**
   * Decode BPG files back to original formats (using metadata)
   */
  bool decode_images;
  /**
   * HEIC quality (1-100) for re-encoding HEIC files
   */
  int heic_quality;
  /**
   * JPEG quality (1-100) for decoding to JPEG
   */
  int jpeg_quality;
} ExtractionSettings;

/**
 * Archive file information for listing
 */
typedef struct ArchiveFileInfo {
  const char *filename;
  uint64_t original_size;
  uint64_t compressed_size;
  int file_type;
} ArchiveFileInfo;

/**
 * Archive record information for FFI
 */
typedef struct ArchiveRecordInfo {
  int64_t id;
  const char *archive_path;
  uint64_t archive_size;
  uint64_t creation_date;
  const char *original_location;
  const char *destination_location;
  const char *description;
  uint32_t file_count;
} ArchiveRecordInfo;

int CreateArchive(const char *output_path,
                  const char *const *input_files,
                  int file_count,
                  const struct CompressionSettings *settings,
                  struct Option_ProgressCallback callback);

int VerifyArchive(const char *archive_path);

int ExtractArchive(const char *archive_path,
                   const char *output_dir,
                   struct Option_ProgressCallback callback);

/**
 * Extract archive with optional decoding of BPG/HEIC files
 */
int ExtractArchiveWithDecoding(const char *archive_path,
                               const char *output_dir,
                               const struct ExtractionSettings *settings,
                               struct Option_ProgressCallback callback);

const char *GetOpenArcError(void);

void FreeCString(char *ptr);

char *PhoneGetStatusJson(const char *phone_root);

int PhoneArchivePendingFiles(const char *phone_root,
                             const char *output_path,
                             const struct CompressionSettings *settings,
                             struct Option_ProgressCallback callback);

/**
 * List archive contents
 */
int ListArchive(const char *archive_path, int *file_count, struct ArchiveFileInfo **files);

/**
 * Update archive destination location
 */
int UpdateArchiveDestination(const char *catalog_db_path,
                             const char *archive_path,
                             const char *destination_path);

/**
 * Get all archives from the database
 */
int GetAllArchives(const char *catalog_db_path,
                   int *archive_count,
                   struct ArchiveRecordInfo **archives);

/**
 * Free the memory allocated by GetAllArchives
 */
void FreeArchivesArray(struct ArchiveRecordInfo *archives, int count);

/**
 * Encode a single image file to BPG
 */
int EncodeBpgFile(const char *input_path,
                  const char *output_path,
                  const struct CompressionSettings *settings);

/**
 * Encode a single video file with FFmpeg
 */
int EncodeVideoFile(const char *input_path,
                    const char *output_path,
                    const struct CompressionSettings *settings);
