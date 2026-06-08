pub mod bitstream {}
pub mod huffman {}
pub mod mem {}
pub mod rangecoder {}

pub mod varint {
    pub use crate::core::varint::{decode_varint, encode_varint};
}
