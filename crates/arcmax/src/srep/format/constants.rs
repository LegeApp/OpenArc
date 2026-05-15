// Archive-level signatures (from srep.cpp and Compression.h)
pub const BULAT_ZIGANSHIN_SIGNATURE: u32 = 0x2635_1817;
pub const SREP_SIGNATURE: u32 = 0x5045_5253; // b"SREP" little-endian

// Format version bytes (stored in bits 0-7 of archive header word 2)
pub const FORMAT_VERSION_IO_LZ_ROUND: u8 = 1;     // -m3 without dict (ROUND_MATCHES)
pub const FORMAT_VERSION_IO_LZ_NONROUND: u8 = 2;  // IO_LZ with 4-stat matches
pub const FORMAT_VERSION_FUTURE_LZ: u8 = 3;
pub const FORMAT_VERSION_INDEX_LZ: u8 = 4;        // default for -m3

pub const FOOTER_VERSION: u8 = 1; // SREP_FOOTER_VERSION1

// Fixed field counts
pub const ARCHIVE_HEADER_WORDS: usize = 4;  // STAT words before the seed
pub const BLOCK_HEADER_WORDS: usize = 3;    // STAT words before the digest
pub const INDEX_LZ_FOOTER_WORDS: usize = 6; // STAT words in the INDEX_LZ footer

// Compression limits
pub const MINIMAL_MIN_MATCH: usize = 16;
pub const DEFAULT_MIN_MATCH: usize = 32;
pub const MAX_HASH_SIZE_BYTES: usize = 256;

// Acceleration variants (ACCELERATOR template parameter in C++ compress<ACCEL>)
pub const ACCEL_VALUES: &[usize] = &[0, 1, 2, 4, 8, 16, 32, 64];
pub const DEFAULT_ACCEL: usize = 1;
pub const MAX_HASH_CHAIN: usize = 12; // MAX_HASH_CHAIN in C++ HashTable
