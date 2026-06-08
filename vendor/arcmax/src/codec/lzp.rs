#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LzpOptions {
    pub min_match: usize,
    pub hash_size_log: u8,
}

impl Default for LzpOptions {
    fn default() -> Self {
        Self {
            min_match: 32,
            hash_size_log: 18,
        }
    }
}
