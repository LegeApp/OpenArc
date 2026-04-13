use std::io::{Write, Seek, SeekFrom};
use anyhow::{Result, anyhow};
use crate::formats::freearc::constants::{BlockType, ARC_SIGNATURE};
use crate::formats::freearc::block::BlockDescriptor;
use crate::formats::freearc::footer::FooterBlock;
use crate::formats::freearc::directory::{DirectoryBlock, DataBlockInfo, FileInfo};
use crate::core::crypto::{EncryptionInfo, create_encryptor, CascadedDecryptor};
use crate::formats::freearc::utils::split_compressor_encryption;
use crate::codecs::lzma2::{compress_lzma_default, compress_lzma};

pub struct ArchiveOptions {
    pub compression: String, // e.g. "lzma"
    pub compression_level: i32,
    pub encryption: Option<String>, // e.g. "aes-256"
    pub password: Option<String>,
}

pub struct FreeArcWriter<W: Write + Seek> {
    writer: W,
    options: ArchiveOptions,
    
    // State
    files: Vec<FileInfo>,
    data_blocks: Vec<DataBlockInfo>,
    directories: Vec<String>,
    
    current_offset: u64,
    
    // Pending data for solid block
    pending_data: Vec<u8>,
    pending_files: Vec<FileInfo>, // Files in current pending block
}

impl<W: Write + Seek> FreeArcWriter<W> {
    pub fn new(mut writer: W, options: ArchiveOptions) -> Result<Self> {
        let current_offset = writer.stream_position()?;
        
        // Write Header Block (Signature + Version) if at start?
        // Spec says: "HEADER block is the first block of any archive. It starts with FreeArc arhive signature..."
        // But usually we just write the signature bytes `ArC\x01` at the very beginning.
        // `free_arc_writer.rs` does not seem to write a full Header Block struct, just signature.
        // Let's verify spec: "HEADER block... starts with FreeArc arhive signature, plus contains info about archiver version."
        // And it is a control block, so it has a descriptor?
        // "Each control block is immediately followed by it's LOCAL DESCRIPTOR".
        // If we write a Header Block, we need a descriptor for it.
        // However, standard archives often just start with signature.
        // `ArhiveStructure.hs`: `archiveWriteHeaderBlock` writes `aARCHIVE_SIGNATURE`.
        // `aARCHIVE_SIGNATURE` is `(aSIGNATURE, aARCHIVE_VERSION)`.
        // `aSIGNATURE` is `ArC\x01`.
        // It seems it just writes bytes, not a full block with descriptor.
        // Let's just write signature for now.
        
        if current_offset == 0 {
             writer.write_all(&ARC_SIGNATURE)?;
             // Write version? Haskell writes `aARCHIVE_VERSION`.
             // `aARCHIVE_VERSION` is a Word16?
             // Let's skip for now or write a simple header if needed.
             // For compatibility, just the signature might be enough or the signature IS the header.
        }
        
        let current_offset = writer.stream_position()?; // Update after signature
        
        Ok(FreeArcWriter {
            writer,
            options,
            files: Vec::new(),
            data_blocks: Vec::new(),
            directories: vec![String::new()], // Root dir
            current_offset,
            pending_data: Vec::new(),
            pending_files: Vec::new(),
        })
    }
    
    pub fn add_file(&mut self, path: &str, data: &[u8]) -> Result<()> {
        // Simple implementation: 1 file = 1 block for now, or accumulation.
        // Let's accumulate until some size?
        // For simplicity: Accumulate.
        
        let dir_index = 0; // TODO: Directory management
        
        let file_info = FileInfo {
            name: path.to_string(),
            dir_index,
            size: data.len() as u64,
            time: 0, // TODO: Time
            is_dir: false,
            crc: crc32fast::hash(data),
            data_block_index: None, // Set when flushing
            offset_in_block: self.pending_data.len() as u64,
        };
        
        self.pending_data.extend_from_slice(data);
        self.pending_files.push(file_info);
        
        // Auto-flush if > 16MB
        if self.pending_data.len() > 16 * 1024 * 1024 {
            self.flush_block()?;
        }
        
        Ok(())
    }
    
    pub fn flush_block(&mut self) -> Result<()> {
        if self.pending_data.is_empty() {
            return Ok(());
        }
        
        let original_size = self.pending_data.len() as u64;
        
        // Compress/Encrypt
        let (compressed_data, method_string) = self.compress_and_encrypt(&self.pending_data)?;
        
        let compressed_size = compressed_data.len() as u64;
        let offset = self.current_offset;
        
        // Write data
        self.writer.write_all(&compressed_data)?;
        self.current_offset += compressed_size;
        
        // Record block info
        let block_idx = self.data_blocks.len();
        self.data_blocks.push(DataBlockInfo {
            compressor: method_string,
            original_size,
            compressed_size,
            offset, // Absolute for now, converted to relative in DirectoryBlock::write
            num_files: self.pending_files.len() as u32,
        });
        
        // Update files with block index
        for mut file in self.pending_files.drain(..) {
            file.data_block_index = Some(block_idx);
            self.files.push(file);
        }
        
        self.pending_data.clear();
        
        Ok(())
    }
    
    fn compress_and_encrypt(&self, data: &[u8]) -> Result<(Vec<u8>, String)> {
        let mut method = self.options.compression.clone();
        if method.is_empty() {
            method = "storing".to_string();
        }
        
        let mut processed = data.to_vec();
        
        // Compress
        if method.starts_with("lzma") {
             let level = self.options.compression_level;
             processed = if level > 0 {
                 compress_lzma(&processed, level, 32 * 1024 * 1024, 3, 0, 2)?
             } else {
                 compress_lzma_default(&processed)?
             };
             // We keep the method string as is, assuming defaults or that header contains info
             // Ideally we would update method string with exact parameters if needed
        }
        
        // Encrypt
        if let Some(enc_method) = &self.options.encryption {
            if let Some(pwd) = &self.options.password {
                let (full_method, encryptor) = create_encryptor(enc_method, pwd)?;
                processed = encryptor.encrypt(&processed)?;
                method = format!("{}+{}", method, full_method); // Fix method string
            }
        }
        
        Ok((processed, method))
    }
    
    pub fn finish(mut self) -> Result<W> {
        self.flush_block()?;
        
        let dir_start_pos = self.current_offset;
        
        // Convert absolute offsets to relative
        // offset = dir_start_pos - block_pos
        for block in &mut self.data_blocks {
             block.offset = dir_start_pos.checked_sub(block.offset).expect("Block pos > Dir pos?");
        }
        
        // Take ownership of data to construct DirectoryBlock, leaving empty vecs in self
        let data_blocks = std::mem::take(&mut self.data_blocks);
        let directories = std::mem::take(&mut self.directories);
        let files = std::mem::take(&mut self.files);
        
        let dir_block = DirectoryBlock {
            data_blocks,
            directories,
            files,
        };
        
        // Serialize Directory
        let mut dir_content = Vec::new();
        dir_block.write(&mut dir_content)?;
        
        let dir_orig_size = dir_content.len() as u64;
        
        // Compress Directory
        let (dir_compressed, dir_method) = self.compress_and_encrypt(&dir_content)?;
        let dir_comp_size = dir_compressed.len() as u64;
        let _dir_crc = crc32fast::hash(&dir_compressed); // CRC of COMPRESSED data? 
        // Spec: "CRC of original data" in descriptor.
        // Wait, BlockDescriptor says "CRC of original data".
        let dir_orig_crc = crc32fast::hash(&dir_content);
        
        self.writer.write_all(&dir_compressed)?;
        self.current_offset += dir_comp_size;
        
        // Create Directory Descriptor
        let dir_desc = BlockDescriptor {
            block_type: BlockType::Directory,
            compressor: dir_method,
            orig_size: dir_orig_size,
            comp_size: dir_comp_size,
            crc: dir_orig_crc,
            pos: Some(dir_start_pos),
        };
        
        // Prepare Footer
        let footer_start_pos = self.current_offset;
        
        // Estimate footer descriptor position (it will be at end of file)
        // Footer Content + Footer Descriptor
        // We iterate to find stable size.
        
        let mut footer_desc_pos = footer_start_pos + 1024; // Initial guess
        
        for _ in 0..3 { // Retry loop
            let footer = FooterBlock {
                control_blocks: vec![dir_desc.clone()], // Add other control blocks if any
                locked: false,
                comment: String::new(),
                recovery: String::new(),
                sfx_size: None,
            };
            
            let mut footer_content = Vec::new();
            footer.write(&mut footer_content, footer_desc_pos)?;
            
            let footer_orig_size = footer_content.len() as u64;
            let footer_orig_crc = crc32fast::hash(&footer_content);
            
            let (footer_compressed, footer_method) = self.compress_and_encrypt(&footer_content)?;
            let footer_comp_size = footer_compressed.len() as u64;
            
            let new_footer_desc_pos = footer_start_pos + footer_comp_size;
            
            if new_footer_desc_pos == footer_desc_pos {
                // Converged
                self.writer.write_all(&footer_compressed)?;
                
                let footer_desc = BlockDescriptor {
                    block_type: BlockType::Footer,
                    compressor: footer_method,
                    orig_size: footer_orig_size,
                    comp_size: footer_comp_size,
                    crc: footer_orig_crc,
                    pos: Some(footer_start_pos), // Point to data
                };
                
                footer_desc.write(&mut self.writer)?;
                return Ok(self.writer);
            }
            
            footer_desc_pos = new_footer_desc_pos;
        }
        
        Ok(self.writer)
    }
}
