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

int ExtractArchiveEntry(const char *archive_path, const char *entry_name, const char *output_path);

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

void FreeArchiveFileList(struct ArchiveFileInfo *files, int count);

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

/**
 * List all connected MTP devices
 */
char *MtpListDevices(void);

/**
 * List contents of a folder on an MTP device
 */
char *MtpListFolder(const char *device_id, const char *object_id);

/**
 * Get thumbnail for MTP file - tries WPD native thumbnail first, falls back to generating from full file
 */
char *MtpGetThumbnail(const char *device_id,
                      const char *object_id,
                      const char *original_name,
                      uint32_t target_width,
                      uint32_t target_height);

/**
 * Cache a file from MTP device to local temp directory and return the local path.
 * This is the key function - C# never sees MTP paths, only local temp paths.
 */
char *MtpCacheFileToTemp(const char *device_id, const char *object_id, const char *original_name);

/**
 * Get temp file path for an MTP object if already cached, without copying.
 */
char *MtpGetCachedPath(const char *device_id, const char *object_id);

/**
 * Clear the MTP temp cache (removes temp files)
 */
char *MtpClearCache(void);

/**
 * Read file to memory - note: THIS IS DANGEROUS FOR LARGE FILES
 * Included for compatibility but better to use cache_file_to_temp
 */
uint8_t *MtpReadFileToMemory(const char *device_id, const char *object_id, uint64_t *out_size);

void MtpFreeData(uint8_t *data, uintptr_t size);

void MtpFreeString(char *s);
