use std::io::{Read, Write};
use anyhow::{Result, bail};
use crate::formats::freearc::constants::BlockType;
use crate::formats::freearc::utils::*;
use crate::formats::freearc::block::BlockDescriptor;

// Data Block Structure (Internal to Directory)
#[derive(Debug, Clone)]
pub struct DataBlockInfo {
    pub compressor: String,
    pub original_size: u64,
    pub compressed_size: u64,
    pub offset: u64, // Relative offset in archive? Or absolute? Spec says relative to start of directory block, but implementation converts to absolute.
    pub num_files: u32,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub dir_index: usize,
    pub size: u64,
    pub time: u32, // DOS or Unix? Haskell says CTime/Word32
    pub is_dir: bool,
    pub crc: u32,
    
    // Calculated fields
    pub data_block_index: Option<usize>,
    pub offset_in_block: u64,
}

#[derive(Debug, Clone)]
pub struct DirectoryBlock {
    pub data_blocks: Vec<DataBlockInfo>,
    pub directories: Vec<String>,
    pub files: Vec<FileInfo>,
}

impl DirectoryBlock {
    pub fn read<R: Read>(reader: &mut R, footer_pos: u64) -> Result<Self> {
        // 1. Number of data blocks (VarInt)
        let (num_blocks, _) = read_varint(reader)?;
        let num_blocks = num_blocks as usize;
        
        // 2. Number of files in each block (VarInt List)
        let files_per_block = read_varint_list(reader, num_blocks)?;
        
        // 3. Compressor strings for each block (StringZ List)
        let compressors = read_string_list(reader, num_blocks)?;
        
        // 4. Relative offsets (VarInt List)
        // "initial block offset in archive, relative to start of the directory block"
        // Wait, spec says "relative to start of the directory block".
        // Haskell implementation: `blEncodePosRelativeTo arcpos blocks` -> `arcpos - p`.
        // So offset = directory_start - block_start.
        // => block_start = directory_start - offset.
        // We know `footer_pos` which is the start of the footer descriptor? No, we need directory position.
        // Actually, the caller should pass the base position if we want to resolve absolute offsets.
        // But here we just read the values.
        let offsets = read_varint_list(reader, num_blocks)?;
        
        // 5. Compressed sizes (VarInt List)
        let comp_sizes = read_varint_list(reader, num_blocks)?;
        
        // 6. Number of directories (VarInt)
        let (num_dirs, _) = read_varint(reader)?;
        
        // 7. Directory names (StringZ List)
        let directories = read_string_list(reader, num_dirs as usize)?;
        
        // 8. Files Metadata
        let total_files: u64 = files_per_block.iter().sum();
        let total_files = total_files as usize;
        
        let names = read_string_list(reader, total_files)?;
        let dir_indices = read_varint_list(reader, total_files)?;
        let sizes = read_varint_list(reader, total_files)?;
        let times = read_fixed_list::<R, u32>(reader, total_files)?;
        let is_dirs = read_fixed_list::<R, bool>(reader, total_files)?;
        let crcs = read_fixed_list::<R, u32>(reader, total_files)?;
        
        // Optional fields end with TAG_END=0
        // Currently just read until TAG_END? Or assume none for now as per minimal implementation?
        // Haskell: `repeat_while (read) (/=aTAG_END) ...`
        // We should check if we can read a byte. If it's not TAG_END(0), we might have issues if we don't know how to skip.
        // But minimal implementation often writes TAG_END immediately.
        
        // Let's try to read one byte.
        // Note: Buffer reader might be needed to peek.
        // If we assume strict format adherence by our writer, we expect 0.
        // If reading from real archives, we should handle tags.
        // For now, let's assume we consume the TAG_END if present, or stop if EOF (though block should be self-contained).
        
        let mut tag_buf = [0u8; 1];
        if reader.read(&mut tag_buf).is_ok() {
             if tag_buf[0] != 0 {
                 // TODO: Handle optional fields
                 eprintln!("Warning: Non-zero optional field tag encountered: {}", tag_buf[0]);
             }
        }
        
        // Reconstruct Data Blocks
        let mut data_blocks = Vec::with_capacity(num_blocks);
        for i in 0..num_blocks {
            data_blocks.push(DataBlockInfo {
                compressor: compressors[i].clone(),
                original_size: 0, // Calculated later
                compressed_size: comp_sizes[i],
                offset: offsets[i],
                num_files: files_per_block[i] as u32,
            });
        }
        
        // Reconstruct Files and calculate sizes/offsets
        let mut files = Vec::with_capacity(total_files);
        let mut current_block_idx = 0;
        let mut files_in_current_block_remaining = if num_blocks > 0 { files_per_block[0] } else { 0 };
        let mut current_offset_in_block = 0;
        
        // We need to calculate original sizes for data blocks by summing file sizes
        
        for i in 0..total_files {
            // Determine which block this file belongs to
            while files_in_current_block_remaining == 0 && current_block_idx < num_blocks - 1 {
                current_block_idx += 1;
                files_in_current_block_remaining = files_per_block[current_block_idx];
                current_offset_in_block = 0;
            }
            
            let file_size = sizes[i];
            
            // Update block original size
            if current_block_idx < data_blocks.len() {
                data_blocks[current_block_idx].original_size += file_size;
            }
            
            files.push(FileInfo {
                name: names[i].clone(),
                dir_index: dir_indices[i] as usize,
                size: file_size,
                time: times[i],
                is_dir: is_dirs[i],
                crc: crcs[i],
                data_block_index: if is_dirs[i] { None } else { Some(current_block_idx) },
                offset_in_block: current_offset_in_block,
            });
            
            if !is_dirs[i] {
                current_offset_in_block += file_size;
                if files_in_current_block_remaining > 0 {
                    files_in_current_block_remaining -= 1;
                }
            }
        }
        
        Ok(DirectoryBlock {
            data_blocks,
            directories,
            files,
        })
    }
    
    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        let num_blocks = self.data_blocks.len();
        
        // 1. Number of blocks
        write_varint(writer, num_blocks as u64)?;
        
        // 2. Files per block
        let files_per_block: Vec<u64> = self.data_blocks.iter().map(|b| b.num_files as u64).collect();
        write_varint_list(writer, &files_per_block)?;
        
        // 3. Compressors
        let compressors: Vec<String> = self.data_blocks.iter().map(|b| b.compressor.clone()).collect();
        write_string_list(writer, &compressors)?;
        
        // 4. Offsets
        let offsets: Vec<u64> = self.data_blocks.iter().map(|b| b.offset).collect();
        write_varint_list(writer, &offsets)?;
        
        // 5. Compressed Sizes
        let comp_sizes: Vec<u64> = self.data_blocks.iter().map(|b| b.compressed_size).collect();
        write_varint_list(writer, &comp_sizes)?;
        
        // 6. Number of directories
        write_varint(writer, self.directories.len() as u64)?;
        
        // 7. Directory names
        write_string_list(writer, &self.directories)?;
        
        // 8. Files Metadata
        let names: Vec<String> = self.files.iter().map(|f| f.name.clone()).collect();
        write_string_list(writer, &names)?;
        
        let dir_indices: Vec<u64> = self.files.iter().map(|f| f.dir_index as u64).collect();
        write_varint_list(writer, &dir_indices)?;
        
        let sizes: Vec<u64> = self.files.iter().map(|f| f.size).collect();
        write_varint_list(writer, &sizes)?;
        
        let times: Vec<u32> = self.files.iter().map(|f| f.time).collect();
        write_fixed_list(writer, &times)?;
        
        let is_dirs: Vec<bool> = self.files.iter().map(|f| f.is_dir).collect();
        write_fixed_list(writer, &is_dirs)?;
        
        let crcs: Vec<u32> = self.files.iter().map(|f| f.crc).collect();
        write_fixed_list(writer, &crcs)?;
        
        // 9. TAG_END
        writer.write_all(&[0])?;
        
        Ok(())
    }
}
