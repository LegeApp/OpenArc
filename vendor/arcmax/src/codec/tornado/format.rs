use std::io::{Read, Write};

use crate::error::{ArcError, Result};

/// On-disk entropy coding identifier stored in the Tornado stream header.
///
/// Numeric values match the C++ constants `STORING=0 .. ARICODER=4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EncodingMethod {
    Storing = 0,
    ByteCoder = 1,
    BitCoder = 2,
    HuffmanCoder = 3,
    ArithmeticCoder = 4,
}

impl EncodingMethod {
    pub fn from_byte(b: u8) -> Result<Self> {
        match b {
            0 => Ok(Self::Storing),
            1 => Ok(Self::ByteCoder),
            2 => Ok(Self::BitCoder),
            3 => Ok(Self::HuffmanCoder),
            4 => Ok(Self::ArithmeticCoder),
            _ => Err(ArcError::Codec {
                codec: "tornado",
                message: format!("unknown encoding method byte: {b}"),
            }),
        }
    }
}

/// The 6-byte header that precedes every Tornado compressed stream.
///
/// Written by `coder.put8(encoding_method) + coder.put8(minlen) + coder.put32(buffer)`.
/// Parsed by the decompressor before choosing the decoder variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TornadoHeader {
    /// Entropy coder used for this stream.
    pub encoding: EncodingMethod,
    /// Minimum match length the compressor searched for.
    pub min_match_len: u8,
    /// Dictionary/buffer size in bytes.
    pub buf_size: u32,
}

impl TornadoHeader {
    /// Wire size: 1 + 1 + 4 bytes.
    pub const SIZE: usize = 6;

    /// Parse from the first 6 bytes of a compressed Tornado stream.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(ArcError::Codec {
                codec: "tornado",
                message: format!(
                    "header too short: need {} bytes, got {}",
                    Self::SIZE,
                    data.len()
                ),
            });
        }
        let encoding = EncodingMethod::from_byte(data[0])?;
        let min_match_len = data[1];
        let buf_size = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
        Ok(Self {
            encoding,
            min_match_len,
            buf_size,
        })
    }

    /// Parse by consuming exactly [`SIZE`] bytes from a reader.
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; Self::SIZE];
        reader.read_exact(&mut buf)?;
        Self::parse(&buf)
    }

    /// Serialize to the canonical 6-byte wire encoding.
    pub fn to_bytes(self) -> [u8; Self::SIZE] {
        let [b2, b3, b4, b5] = self.buf_size.to_le_bytes();
        [self.encoding as u8, self.min_match_len, b2, b3, b4, b5]
    }

    /// Write the 6-byte header to a writer.
    pub fn write_to<W: Write>(self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_bytes())?;
        Ok(())
    }
}

// ---- table kinds (used in LZ stream commands) ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableKind {
    Diff2,
    Diff4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TornadoFrameKind {
    Raw,
    Lz,
}
