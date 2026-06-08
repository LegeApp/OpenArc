#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepOptions {
    pub block_size: usize,
    pub min_compression: i32,
    pub chunk_size: usize,
    pub min_match_len: i32,
    pub barrier: i32,
    pub smallest_len: i32,
    pub hash_size_log: u32,
    pub amplifier: i32,
}

impl Default for RepOptions {
    fn default() -> Self {
        Self {
            block_size: 64 * 1024 * 1024,
            min_compression: 100,
            chunk_size: 0,
            min_match_len: 512,
            barrier: i32::MAX,
            smallest_len: 512,
            hash_size_log: 0,
            amplifier: 1,
        }
    }
}
