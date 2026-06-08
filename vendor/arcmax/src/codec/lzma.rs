#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LzmaOptions {
    pub dict_size: u32,
    pub lc: u32,
    pub lp: u32,
    pub pb: u32,
    pub level: Option<u8>,
    /// LZMA "fast bytes" / nice match length. 7-Zip exposes this as
    /// "Word size"; preset 9 in `lzma-rust2` defaults to 64, while 7-Zip
    /// Ultra commonly uses 273.
    pub nice_len: Option<u32>,
    pub lzma2: bool,
}

impl Default for LzmaOptions {
    fn default() -> Self {
        Self {
            dict_size: 32 * 1024 * 1024,
            lc: 3,
            lp: 0,
            pb: 2,
            level: Some(5),
            nice_len: None,
            lzma2: true,
        }
    }
}
