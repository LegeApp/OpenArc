#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XzOptions {
    /// LZMA2 preset level (1–9). None = 5.
    pub level: Option<u8>,
    /// LZMA2 dictionary size in bytes. 0 = derive from preset.
    pub dict_size: u32,
}

impl XzOptions {
    pub fn with_level(level: u8) -> Self {
        Self {
            level: Some(level),
            dict_size: 0,
        }
    }
}

impl Default for XzOptions {
    fn default() -> Self {
        Self {
            level: Some(6),
            dict_size: 0,
        }
    }
}
