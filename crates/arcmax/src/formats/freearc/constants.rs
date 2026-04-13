
pub const ARC_SIGNATURE: [u8; 4] = [0x41, 0x72, 0x43, 0x01]; // "ArC\x01"
pub const MAX_FOOTER_DESCRIPTOR_SIZE: u64 = 4096;
pub const SCAN_MAX: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    Descriptor = 0,
    Header = 1,
    Data = 2,
    Directory = 3,
    Footer = 4,
    Recovery = 5,
    Unknown = 255,
}

impl From<u8> for BlockType {
    fn from(v: u8) -> Self {
        match v {
            0 => BlockType::Descriptor,
            1 => BlockType::Header,
            2 => BlockType::Data,
            3 => BlockType::Directory,
            4 => BlockType::Footer,
            5 => BlockType::Recovery,
            _ => BlockType::Unknown,
        }
    }
}

impl From<BlockType> for u8 {
    fn from(val: BlockType) -> Self {
        val as u8
    }
}
