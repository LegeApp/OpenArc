pub(crate) mod h;

/// Which PPMd variant to use for encoding/decoding.
///
/// `H` uses Shkarin's PPMdH model with Subbotin's carryless range coder —
/// this is the format produced and consumed by FreeArc.
/// `Seven` uses the 7-zip adaptation (PPMd7) which differs only in the range
/// coder and is used by 7-Zip `.7z` archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PpmdVariant {
    /// FreeArc-compatible PPMdH (carryless range coder). Default.
    #[default]
    H,
    /// 7-zip PPMd7 (7z range coder with sentinel byte).
    Seven,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmdOptions {
    pub order: u8,
    pub memory_size: usize,
    pub variant: PpmdVariant,
}

impl PpmdOptions {
    pub const MIN_ORDER: u8 = 2;
    pub const MAX_ORDER: u8 = 16;
    pub const MIN_MEMORY_SIZE: usize = 1 << 11;
    pub const MAX_MEMORY_SIZE: usize = u32::MAX as usize - 12;

    pub fn validate(&self) -> crate::error::Result<()> {
        if !(Self::MIN_ORDER..=Self::MAX_ORDER).contains(&self.order) {
            return Err(crate::error::ArcError::InvalidMethod(format!(
                "PPMd order must be {}..={}, got {}",
                Self::MIN_ORDER,
                Self::MAX_ORDER,
                self.order
            )));
        }
        if !(Self::MIN_MEMORY_SIZE..=Self::MAX_MEMORY_SIZE).contains(&self.memory_size) {
            return Err(crate::error::ArcError::InvalidMethod(format!(
                "PPMd memory must be {}..={} bytes, got {}",
                Self::MIN_MEMORY_SIZE,
                Self::MAX_MEMORY_SIZE,
                self.memory_size
            )));
        }
        Ok(())
    }
}

impl Default for PpmdOptions {
    fn default() -> Self {
        Self {
            order: 6,
            memory_size: 16 * 1024 * 1024,
            variant: PpmdVariant::H,
        }
    }
}
