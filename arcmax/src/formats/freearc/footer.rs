use std::io::{Read, Write};
use anyhow::Result;
use crate::formats::freearc::constants::BlockType;
use crate::formats::freearc::utils::{read_varint, write_varint, read_stringz, write_stringz};
use crate::formats::freearc::block::BlockDescriptor;

#[derive(Debug, Clone)]
pub struct FooterBlock {
    pub control_blocks: Vec<BlockDescriptor>,
    pub locked: bool,
    pub comment: String,
    pub recovery: String,
    // Calculated/Internal fields
    pub sfx_size: Option<u64>, 
}

impl FooterBlock {
    // Note: This reads the CONTENT of the footer block (decompressed), not the descriptor.
    pub fn read<R: Read>(reader: &mut R, footer_desc_pos: u64) -> Result<Self> {
        // 1. Number of control blocks (VarInt)
        let (num_blocks, _) = read_varint(reader)?;
        
        let mut control_blocks = Vec::with_capacity(num_blocks as usize);
        
        for _ in 0..num_blocks {
            // Each block tuple: (t, c, rel_offset, o, s, crc)
            let (type_val, _) = read_varint(reader)?;
            let block_type = BlockType::from(type_val as u8);
            
            let compressor = read_stringz(reader)?;
            
            let (rel_offset, _) = read_varint(reader)?;
            let pos = footer_desc_pos.checked_sub(rel_offset).unwrap_or(0);
            
            let (orig_size, _) = read_varint(reader)?;
            let (comp_size, _) = read_varint(reader)?;
            
            let mut crc_buf = [0u8; 4];
            reader.read_exact(&mut crc_buf)?;
            let crc = u32::from_le_bytes(crc_buf);
            
            control_blocks.push(BlockDescriptor {
                block_type,
                compressor,
                orig_size,
                comp_size,
                crc,
                pos: Some(pos),
            });
        }
        
        // 2. Locked (Bool/1 byte)
        let mut locked_buf = [0u8; 1];
        reader.read_exact(&mut locked_buf)?;
        let locked = locked_buf[0] != 0;
        
        // 3. Old Comment (VarInt length + bytes, but mostly 0 length)
        let (old_comment_len, _) = read_varint(reader)?;
        if old_comment_len > 0 {
             let mut skip = vec![0u8; old_comment_len as usize];
             reader.read_exact(&mut skip)?;
        }
        
        // 4. Recovery Info (StringZ - optional check EOF?)
        let recovery = match read_stringz(reader) {
            Ok(s) => s,
            Err(_) => String::new(), // Assume EOF or empty
        };
        
        // 5. Comment (VarInt Length + Bytes)
        let comment = match read_varint(reader) {
            Ok((len, _)) => {
                let mut bytes = vec![0u8; len as usize];
                reader.read_exact(&mut bytes)?;
                String::from_utf8_lossy(&bytes).to_string()
            }
            Err(_) => String::new(),
        };
        
        Ok(FooterBlock {
            control_blocks,
            locked,
            comment,
            recovery,
            sfx_size: None, // Need to calculate from blocks
        })
    }
    
    pub fn write<W: Write>(&self, writer: &mut W, footer_desc_pos: u64) -> Result<()> {
        // 1. Number of blocks
        write_varint(writer, self.control_blocks.len() as u64)?;
        
        // Write blocks
        for block in &self.control_blocks {
            write_varint(writer, u8::from(block.block_type) as u64)?;
            write_stringz(writer, &block.compressor)?;
            
            let pos = block.pos.expect("Block position must be set for writing footer");
            let rel_offset = footer_desc_pos.checked_sub(pos).expect("Block position is after footer descriptor?");
            write_varint(writer, rel_offset)?;
            
            write_varint(writer, block.orig_size)?;
            write_varint(writer, block.comp_size)?;
            writer.write_all(&block.crc.to_le_bytes())?;
        }
        
        // 2. Locked
        writer.write_all(&[if self.locked { 1 } else { 0 }])?;
        
        // 3. Old Comment (0 length)
        write_varint(writer, 0)?;
        
        // 4. Recovery
        write_stringz(writer, &self.recovery)?;
        
        // 5. Comment
        let comment_bytes = self.comment.as_bytes();
        write_varint(writer, comment_bytes.len() as u64)?;
        writer.write_all(comment_bytes)?;
        
        Ok(())
    }
}
