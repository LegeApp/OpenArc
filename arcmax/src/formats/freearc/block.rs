use std::io::{Read, Write};
use anyhow::{Result, bail};
use crate::formats::freearc::constants::{BlockType, ARC_SIGNATURE};
use crate::formats::freearc::utils::{read_varint, read_stringz};

#[derive(Debug, Clone)]
pub struct BlockDescriptor {
    pub block_type: BlockType,
    pub compressor: String,
    pub orig_size: u64,
    pub comp_size: u64,
    pub crc: u32,
    // The position is not part of the descriptor on disk, but useful context
    pub pos: Option<u64>, 
}

impl BlockDescriptor {
    pub fn read<R: Read>(reader: &mut R) -> Result<Self> {
        // 1. Signature
        let mut sig = [0u8; 4];
        reader.read_exact(&mut sig)?;
        if sig != ARC_SIGNATURE {
            bail!("Invalid block signature: {:02x?}", sig);
        }

        // 2. Block Type (VarInt)
        let (block_type_val, _) = read_varint(reader)?;
        let block_type = BlockType::from(block_type_val as u8);

        // 3. Compressor (StringZ)
        let compressor = read_stringz(reader)?;

        // 4. Orig Size (VarInt)
        let (orig_size, _) = read_varint(reader)?;

        // 5. Comp Size (VarInt)
        let (comp_size, _) = read_varint(reader)?;

        // 6. Data CRC (4 bytes)
        let mut crc_buf = [0u8; 4];
        reader.read_exact(&mut crc_buf)?;
        let crc = u32::from_le_bytes(crc_buf);

        // 7. Descriptor CRC (4 bytes)
        let mut desc_crc_buf = [0u8; 4];
        reader.read_exact(&mut desc_crc_buf)?;
        let _desc_crc = u32::from_le_bytes(desc_crc_buf);

        // TODO: Verify descriptor CRC.

        Ok(BlockDescriptor {
            block_type,
            compressor,
            orig_size,
            comp_size,
            crc,
            pos: None,
        })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        let mut buf = Vec::new();
        
        // 1. Signature
        buf.extend_from_slice(&ARC_SIGNATURE);
        
        // 2. Block Type
        buf.extend_from_slice(&crate::core::varint::encode_varint(u8::from(self.block_type) as u64));
        
        // 3. Compressor
        buf.extend_from_slice(self.compressor.as_bytes());
        buf.push(0);
        
        // 4. Orig Size
        buf.extend_from_slice(&crate::core::varint::encode_varint(self.orig_size));
        
        // 5. Comp Size
        buf.extend_from_slice(&crate::core::varint::encode_varint(self.comp_size));
        
        // 6. Data CRC
        buf.extend_from_slice(&self.crc.to_le_bytes());
        
        // Calculate Descriptor CRC (CRC32 of fields 1-6)
        let desc_crc = crc32fast::hash(&buf);
        
        // Write fields 1-6
        writer.write_all(&buf)?;
        
        // 7. Descriptor CRC
        writer.write_all(&desc_crc.to_le_bytes())?;
        
        Ok(())
    }
}
