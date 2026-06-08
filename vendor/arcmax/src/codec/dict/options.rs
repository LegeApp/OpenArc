#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictOptions {
    pub block_size: usize,
    pub min_compression: i32,
    pub min_weak_chars: i32,
    pub min_large_cnt: i32,
    pub min_medium_cnt: i32,
    pub min_small_cnt: i32,
    pub min_ratio: i32,
}

impl Default for DictOptions {
    fn default() -> Self {
        Self {
            block_size: 64 * 1024 * 1024,
            min_compression: 100,
            min_weak_chars: 20,
            min_large_cnt: 2048,
            min_medium_cnt: 100,
            min_small_cnt: 50,
            min_ratio: 4,
        }
    }
}
