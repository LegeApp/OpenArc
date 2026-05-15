/// Absolute byte position or file size. Never subtract without checked_sub.
pub type Offset = u64;

/// One element of an encoded STAT stream (4 bytes, little-endian in the archive).
pub type Stat = u32;

/// Index of an L-byte chunk within the global hash table.
pub type ChunkId = u32;

/// Raw rolling hash value used as a hash-table key (machine-word sized).
pub type HashValue = usize;

/// Compact hash stored per-chunk slot for quick rejection before digest compare.
pub type StoredHashValue = u32;

/// Sentinel used only at format boundaries — never as a valid ChunkId.
/// Internal code uses Option<ChunkId> instead.
pub const NOT_FOUND: ChunkId = ChunkId::MAX;
