#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZstdOptions {
    pub level: i32,
}

impl Default for ZstdOptions {
    fn default() -> Self {
        Self { level: 3 }
    }
}
