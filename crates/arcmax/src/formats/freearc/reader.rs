use std::io::{Read, Seek, SeekFrom, Cursor};
use std::sync::Mutex;
use std::path::Path;
use anyhow::{Result, anyhow, Context};
use crate::core::archive::{ArchiveReader, FileEntry};
use crate::formats::freearc::constants::{ARC_SIGNATURE, SCAN_MAX, BlockType};
use crate::formats::freearc::block::BlockDescriptor;
use crate::formats::freearc::footer::FooterBlock;
use crate::formats::freearc::directory::DirectoryBlock;
use crate::formats::freearc::utils::{read_varint, split_compressor_encryption};
use crate::core::crypto::{EncryptionInfo, CascadedDecryptor};
use crate::codecs::lzma2::decompress_lzma_default;

pub struct FreeArcReader<R: Read + Seek> {
    reader: Mutex<R>,
    pub footer: FooterBlock,
    pub directory: DirectoryBlock,
    password: Option<String>,
}

impl<R: Read + Seek> FreeArcReader<R> {
    pub fn new(mut reader: R, password: Option<String>) -> Result<Self> {
        let file_size = reader.seek(SeekFrom::End(0))?;
        
        // 1. Find Footer Descriptor
        let (footer_desc, footer_desc_pos) = Self::find_footer_descriptor(&mut reader, file_size)?;
        
        // 2. Read Footer Block
        let footer_block = Self::read_control_block(&mut reader, &footer_desc, password.as_deref())?;
        
        // 3. Parse Footer
        let mut cursor = Cursor::new(footer_block);
        let footer = FooterBlock::read(&mut cursor, footer_desc_pos)?;
        
        // 4. Find Directory Block Descriptor in Footer
        let dir_desc = footer.control_blocks.iter()
            .find(|b| b.block_type == BlockType::Directory)
            .ok_or_else(|| anyhow!("Directory block not found in footer"))?;
            
        // 5. Read Directory Block
        // Note: Directory block position in descriptor is relative to footer_desc_pos if calculated in footer logic,
        // but BlockDescriptor.pos is an Option<u64>. In FooterBlock::read, we calculated absolute pos.
        let dir_block_data = Self::read_control_block(&mut reader, dir_desc, password.as_deref())?;
        
        // 6. Parse Directory
        let mut dir_cursor = Cursor::new(dir_block_data);
        // We pass footer_desc_pos because offsets in directory might be relative to it? 
        // Actually, directory offsets are relative to "start of directory block".
        // But the parse logic in directory.rs just reads them. The converting to absolute happens in the reader logic usually.
        // Let's check directory.rs. It reads offsets. We need to interpret them.
        let directory = DirectoryBlock::read(&mut dir_cursor, footer_desc_pos)?;
        
        Ok(FreeArcReader {
            reader: Mutex::new(reader),
            footer,
            directory,
            password,
        })
    }

    fn find_footer_descriptor(reader: &mut R, file_size: u64) -> Result<(BlockDescriptor, u64)> {
        let scan_size = std::cmp::min(file_size, SCAN_MAX);
        reader.seek(SeekFrom::End(-(scan_size as i64)))?;
        
        let mut buffer = vec![0u8; scan_size as usize];
        reader.read_exact(&mut buffer)?;
        
        // Search backwards for signature
        for i in (0..buffer.len().saturating_sub(4)).rev() {
            if buffer[i..i+4] == ARC_SIGNATURE {
                let pos = (file_size - scan_size) + i as u64;
                
                // Try to read descriptor at this position
                reader.seek(SeekFrom::Start(pos))?;
                if let Ok(desc) = BlockDescriptor::read(reader) {
                    return Ok((desc, pos));
                }
            }
        }
        
        Err(anyhow!("Could not find valid footer descriptor"))
    }

    fn read_control_block(reader: &mut R, desc: &BlockDescriptor, password: Option<&str>) -> Result<Vec<u8>> {
        let pos = desc.pos.ok_or_else(|| anyhow!("Block position missing"))?;
        
        reader.seek(SeekFrom::Start(pos))?;
        let mut compressed_data = vec![0u8; desc.comp_size as usize];
        reader.read_exact(&mut compressed_data)?;
        
        // Handle Encryption/Compression
        Self::decompress_data(&desc.compressor, &compressed_data, desc.orig_size as usize, password)
    }

    fn decompress_data(method: &str, data: &[u8], orig_size: usize, password: Option<&str>) -> Result<Vec<u8>> {
        let (compressor, encryption) = split_compressor_encryption(method);
        
        // 1. Decrypt if needed
        let processed_data = if !encryption.is_empty() {
             let pwd = password.ok_or_else(|| anyhow!("Password required for encrypted block"))?;
             
             // Parse encryption info
             // Format usually: aes-256/ctr:k...:i... or similar
             // We reuse existing logic for this if possible, or parse here.
             let enc_info = EncryptionInfo::from_method_string(&encryption, None)?;
             let decryptor = CascadedDecryptor::new(&enc_info, pwd)?;
             
             decryptor.decrypt(data)?
        } else {
             data.to_vec() // Cow?
        };
        
        // 2. Decompress
        if compressor == "storing" || compressor.is_empty() {
            return Ok(processed_data);
        }
        
        if compressor.starts_with("lzma") {
             decompress_lzma_default(&processed_data, orig_size)
        } else {
             Err(anyhow!("Unsupported compressor: {}", compressor))
        }
    }
    
    pub fn extract_file(&self, file_index: usize) -> Result<Vec<u8>> {
        let file_info = self.directory.files.get(file_index).ok_or_else(|| anyhow!("Invalid file index"))?;
        
        if file_info.is_dir {
            return Ok(Vec::new());
        }
        
        let block_idx = file_info.data_block_index.ok_or_else(|| anyhow!("File has no data block"))?;
        let block_info = self.directory.data_blocks.get(block_idx).ok_or_else(|| anyhow!("Invalid data block index"))?;
        
        // Calculate absolute position of the data block
        // Block offset is relative to the start of directory block (which we know?)
        // Wait, spec says "initial block offset in archive, relative to start of the directory block".
        // But we don't store "start of directory block" in `DirectoryBlock` struct directly.
        // We have `footer.control_blocks` which has the directory block descriptor.
        
        let dir_desc = self.footer.control_blocks.iter()
            .find(|b| b.block_type == BlockType::Directory)
            .ok_or_else(|| anyhow!("Directory block descriptor missing"))?;
            
        let dir_pos = dir_desc.pos.ok_or_else(|| anyhow!("Directory position missing"))?;
        
        // The offset in block_info is relative to dir_pos?
        // Let's verify interpretation.
        // Haskell: `blDecodePosRelativeTo arcpos offset = arcpos - offset`.
        // Wait, `arcpos` is the position of the Directory Block Descriptor? No, usually the current block position.
        // In `ArhiveDirectory.hs`: `writeList$ map (blEncodePosRelativeTo arcpos) blocks`.
        // `blEncodePosRelativeTo arcpos arcblock = arcpos - blPos arcblock`.
        // So stored_offset = dir_pos - block_pos.
        // => block_pos = dir_pos - stored_offset.
        
        let block_pos = dir_pos.checked_sub(block_info.offset).ok_or_else(|| anyhow!("Invalid block offset calculation"))?;
        
        // Read and decompress block
        let mut reader = self.reader.lock().unwrap();
        reader.seek(SeekFrom::Start(block_pos))?;
        
        let mut compressed_data = vec![0u8; block_info.compressed_size as usize];
        reader.read_exact(&mut compressed_data)?;
        
        let decompressed = Self::decompress_data(
            &block_info.compressor, 
            &compressed_data, 
            block_info.original_size as usize, 
            self.password.as_deref()
        )?;
        
        // Extract file slice
        let start = file_info.offset_in_block as usize;
        let end = start + file_info.size as usize;
        
        if end > decompressed.len() {
             return Err(anyhow!("File data outside of decompressed block bounds"));
        }
        
        Ok(decompressed[start..end].to_vec())
    }
}

impl<R: Read + Seek> ArchiveReader for FreeArcReader<R> {
    fn list(&mut self) -> Result<Vec<FileEntry>> {
        let mut entries = Vec::with_capacity(self.directory.files.len());
        
        for file in &self.directory.files {
            entries.push(FileEntry {
                name: file.name.clone(),
                size: file.size,
                compressed_size: 0, // Difficult to calculate per-file without detailed analysis
                mtime: Some(file.time as u64),
                is_dir: file.is_dir,
            });
        }
        
        Ok(entries)
    }
    
    fn extract(&mut self, entry: &FileEntry, writer: &mut dyn std::io::Write) -> Result<()> {
        // Find file index by name
        let index = self.directory.files.iter()
            .position(|f| f.name == entry.name)
            .ok_or_else(|| anyhow!("File not found: {}", entry.name))?;
            
        let data = self.extract_file(index)?;
        writer.write_all(&data)?;
        
        Ok(())
    }
    
    fn extract_all(&mut self, output_dir: &Path) -> Result<()> {
        for (i, file) in self.directory.files.iter().enumerate() {
            let path = output_dir.join(&file.name);
            
            if file.is_dir {
                std::fs::create_dir_all(&path)?;
            } else {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                
                let data = self.extract_file(i)?;
                std::fs::write(&path, &data)?;
            }
        }
        
        Ok(())
    }
}
