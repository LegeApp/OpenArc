use crate::core::archive::{ArchiveReader, FileEntry};
use crate::core::crypto::{CascadedDecryptor, EncryptionInfo};
use crate::formats::freearc::block::BlockDescriptor;
use crate::formats::freearc::constants::{BlockType, ARC_SIGNATURE, SCAN_MAX};
use crate::formats::freearc::directory::DirectoryBlock;
use crate::formats::freearc::footer::FooterBlock;
use crate::formats::freearc::utils::parse_codec_chain;
use anyhow::{anyhow, Result};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

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
        let footer_block =
            Self::read_control_block(&mut reader, &footer_desc, password.as_deref())?;

        // 3. Parse Footer
        let mut cursor = Cursor::new(footer_block);
        let footer = FooterBlock::read(&mut cursor, footer_desc_pos)?;

        // 4. Find Directory Block Descriptor in Footer
        let dir_desc = footer
            .control_blocks
            .iter()
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
            if buffer[i..i + 4] == ARC_SIGNATURE {
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

    fn read_control_block(
        reader: &mut R,
        desc: &BlockDescriptor,
        password: Option<&str>,
    ) -> Result<Vec<u8>> {
        let pos = desc.pos.ok_or_else(|| anyhow!("Block position missing"))?;

        reader.seek(SeekFrom::Start(pos))?;
        let mut compressed_data = vec![0u8; desc.comp_size as usize];
        reader.read_exact(&mut compressed_data)?;

        // Handle Encryption/Compression
        Self::decompress_data(
            &desc.compressor,
            &compressed_data,
            desc.orig_size as usize,
            password,
        )
    }

    fn decompress_data(
        method: &str,
        data: &[u8],
        orig_size: usize,
        password: Option<&str>,
    ) -> Result<Vec<u8>> {
        let chain = parse_codec_chain(method);
        let first_encryption = chain
            .iter()
            .position(|spec| is_encryption_codec(&spec.name))
            .unwrap_or(chain.len());
        let (compressors, encryption) = chain.split_at(first_encryption);

        // 1. Decrypt if needed
        let processed_data = if !encryption.is_empty() {
            let pwd = password.ok_or_else(|| anyhow!("Password required for encrypted block"))?;

            // Parse encryption info
            // Format usually: aes-256/ctr:k...:i... or similar
            // We reuse existing logic for this if possible, or parse here.
            let encryption = encryption
                .iter()
                .map(|spec| {
                    if spec.params.is_empty() {
                        spec.name.clone()
                    } else {
                        format!("{}:{}", spec.name, spec.params.join(":"))
                    }
                })
                .collect::<Vec<_>>()
                .join("+");
            let enc_info = EncryptionInfo::from_method_string(&encryption, None)?;
            let decryptor = CascadedDecryptor::new(&enc_info, pwd)?;

            decryptor.decrypt(data)?
        } else {
            data.to_vec() // Cow?
        };

        // 2. Decompress
        let mut processed_data = processed_data;
        for codec in compressors.iter().rev() {
            match codec.name.as_str() {
                "" | "store" | "storing" => {}
                name if name.starts_with("lzma") => {
                    processed_data = decompress_lzma_compat(&processed_data, orig_size)?;
                }
                "grzip" => {
                    processed_data = crate::codec::grzip_native::decompress_stream(&processed_data)
                        .map_err(|err| anyhow!(err))?;
                }
                "mm" => {
                    processed_data = decompress_mm_native(&processed_data, orig_size)?;
                }
                other => return Err(anyhow!("Unsupported compressor: {}", other)),
            }
        }

        if processed_data.len() != orig_size {
            Err(anyhow!(
                "decompressed {} bytes but block expected {}",
                processed_data.len(),
                orig_size
            ))
        } else {
            Ok(processed_data)
        }
    }

    pub fn extract_file(&self, file_index: usize) -> Result<Vec<u8>> {
        let file_info = self
            .directory
            .files
            .get(file_index)
            .ok_or_else(|| anyhow!("Invalid file index"))?;

        if file_info.is_dir {
            return Ok(Vec::new());
        }

        let block_idx = file_info
            .data_block_index
            .ok_or_else(|| anyhow!("File has no data block"))?;
        let block_info = self
            .directory
            .data_blocks
            .get(block_idx)
            .ok_or_else(|| anyhow!("Invalid data block index"))?;

        // Calculate absolute position of the data block
        // Block offset is relative to the start of directory block (which we know?)
        // Wait, spec says "initial block offset in archive, relative to start of the directory block".
        // But we don't store "start of directory block" in `DirectoryBlock` struct directly.
        // We have `footer.control_blocks` which has the directory block descriptor.

        let dir_desc = self
            .footer
            .control_blocks
            .iter()
            .find(|b| b.block_type == BlockType::Directory)
            .ok_or_else(|| anyhow!("Directory block descriptor missing"))?;

        let dir_pos = dir_desc
            .pos
            .ok_or_else(|| anyhow!("Directory position missing"))?;

        // The offset in block_info is relative to dir_pos?
        // Let's verify interpretation.
        // Haskell: `blDecodePosRelativeTo arcpos offset = arcpos - offset`.
        // Wait, `arcpos` is the position of the Directory Block Descriptor? No, usually the current block position.
        // In `ArhiveDirectory.hs`: `writeList$ map (blEncodePosRelativeTo arcpos) blocks`.
        // `blEncodePosRelativeTo arcpos arcblock = arcpos - blPos arcblock`.
        // So stored_offset = dir_pos - block_pos.
        // => block_pos = dir_pos - stored_offset.

        let block_pos = dir_pos
            .checked_sub(block_info.offset)
            .ok_or_else(|| anyhow!("Invalid block offset calculation"))?;

        // Read and decompress block
        let mut reader = self.reader.lock().unwrap();
        reader.seek(SeekFrom::Start(block_pos))?;

        let mut compressed_data = vec![0u8; block_info.compressed_size as usize];
        reader.read_exact(&mut compressed_data)?;

        let decompressed = Self::decompress_data(
            &block_info.compressor,
            &compressed_data,
            block_info.original_size as usize,
            self.password.as_deref(),
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

fn is_encryption_codec(name: &str) -> bool {
    name.starts_with("aes") || name.starts_with("blowfish") || name == "encryption"
}

fn decompress_mm_native(data: &[u8], orig_size: usize) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Err(anyhow!("MM stream is empty"));
    }

    let flags = data[0];
    if flags == 0 {
        return Ok(data[1..].to_vec());
    }

    // FreeArc's mm_decompress currently rejects reorder flags too.
    if flags & !1 != 0 {
        return Err(anyhow!("Unsupported MM reorder flags: {flags:#x}"));
    }
    if data.len() < 7 {
        return Err(anyhow!("MM stream header is truncated"));
    }

    let num_chan = data[1] as usize;
    let word_size = data[2] as usize;
    let offset = u32::from_le_bytes(data[3..7].try_into().unwrap()) as usize;
    let byte_size = (word_size + 7) / 8;
    let sample_size = num_chan
        .checked_mul(byte_size)
        .ok_or_else(|| anyhow!("MM sample size overflows"))?;
    if !(1..=4).contains(&byte_size) || sample_size == 0 {
        return Err(anyhow!(
            "Unsupported MM sample layout: channels={num_chan}, word_size={word_size}"
        ));
    }

    let mut pos = 7usize;
    if pos + offset > data.len() {
        return Err(anyhow!("MM original header exceeds stream size"));
    }

    let mut output = Vec::with_capacity(orig_size);
    output.extend_from_slice(&data[pos..pos + offset]);
    pos += offset;

    let align = round_up(7 + offset, sample_size) - (7 + offset);
    if pos + align > data.len() {
        return Err(anyhow!("MM alignment padding exceeds stream size"));
    }
    pos += align;

    let mut base = vec![0u32; num_chan];
    let chunk_size = if sample_size > 1 {
        round_down(256 * 1024, sample_size)
    } else {
        256 * 1024
    };

    while pos < data.len() {
        let len = (data.len() - pos).min(chunk_size);
        let full_len = round_down(len, sample_size);
        if full_len == 0 {
            output.extend_from_slice(&data[pos..]);
            break;
        }

        let chunk = &data[pos..pos + full_len];
        match byte_size {
            1 => undiff1(chunk, num_chan, &mut base, &mut output),
            2 => undiff2(chunk, num_chan, &mut base, &mut output),
            3 => undiff3(chunk, num_chan, &mut base, &mut output),
            4 => undiff4(chunk, num_chan, &mut base, &mut output),
            _ => unreachable!(),
        }
        pos += full_len;

        if full_len < len {
            output.extend_from_slice(&data[pos..pos + (len - full_len)]);
            pos += len - full_len;
        }
    }

    Ok(output)
}

fn undiff1(input: &[u8], num_chan: usize, base: &mut [u32], output: &mut Vec<u8>) {
    for sample in input.chunks_exact(num_chan) {
        for i in 0..num_chan {
            let value = (base[i] as u8).wrapping_add(sample[i]);
            base[i] = value as u32;
            output.push(value);
        }
    }
}

fn undiff2(input: &[u8], num_chan: usize, base: &mut [u32], output: &mut Vec<u8>) {
    for sample in input.chunks_exact(num_chan * 2) {
        for i in 0..num_chan {
            let off = i * 2;
            let delta = u16::from_le_bytes(sample[off..off + 2].try_into().unwrap());
            let value = (base[i] as u16).wrapping_add(delta);
            base[i] = value as u32;
            output.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn undiff3(input: &[u8], num_chan: usize, base: &mut [u32], output: &mut Vec<u8>) {
    for sample in input.chunks_exact(num_chan * 3) {
        for i in 0..num_chan {
            let off = i * 3;
            let delta = read_u24_le(&sample[off..off + 3]);
            let value = base[i].wrapping_add(delta) & 0x00ff_ffff;
            base[i] = value;
            output.extend_from_slice(&value.to_le_bytes()[..3]);
        }
    }
}

fn undiff4(input: &[u8], num_chan: usize, base: &mut [u32], output: &mut Vec<u8>) {
    for sample in input.chunks_exact(num_chan * 4) {
        for i in 0..num_chan {
            let off = i * 4;
            let delta = u32::from_le_bytes(sample[off..off + 4].try_into().unwrap());
            let value = base[i].wrapping_add(delta);
            base[i] = value;
            output.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn read_u24_le(bytes: &[u8]) -> u32 {
    bytes[0] as u32 | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16)
}

fn round_down(value: usize, factor: usize) -> usize {
    if factor <= 1 {
        value
    } else {
        value - (value % factor)
    }
}

fn round_up(value: usize, factor: usize) -> usize {
    if value != 0 && factor > 1 {
        round_down(value - 1, factor) + factor
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored_grzip_block(data: &[u8]) -> Vec<u8> {
        let mut block = Vec::with_capacity(28 + data.len());
        block.extend_from_slice(&(data.len() as i32).to_le_bytes());
        block.extend_from_slice(&(-1i32).to_le_bytes());
        block.extend_from_slice(&0i32.to_le_bytes());
        block.extend_from_slice(&0i32.to_le_bytes());
        block.extend_from_slice(&(data.len() as i32).to_le_bytes());
        block.extend_from_slice(&0i32.to_le_bytes());
        block.extend_from_slice(&0i32.to_le_bytes());
        block.extend_from_slice(data);
        block
    }

    #[test]
    fn mm_native_undiffs_interleaved_samples() {
        let mut encoded = vec![1, 2, 8];
        encoded.extend_from_slice(&4u32.to_le_bytes());
        encoded.extend_from_slice(b"RIFF");
        encoded.push(0); // alignment to 2-byte sample size
        encoded.extend_from_slice(&[10, 20, 3, 5, 255, 5]);

        let decoded = decompress_mm_native(&encoded, 10).unwrap();
        assert_eq!(decoded, b"RIFF\x0a\x14\x0d\x19\x0c\x1e");
    }

    #[test]
    fn freearc_data_block_decodes_mm_plus_grzip() {
        let mut mm_encoded = vec![1, 2, 8];
        mm_encoded.extend_from_slice(&4u32.to_le_bytes());
        mm_encoded.extend_from_slice(b"RIFF");
        mm_encoded.push(0);
        mm_encoded.extend_from_slice(&[10, 20, 3, 5, 255, 5]);

        let grzip_stream = stored_grzip_block(&mm_encoded);
        let decoded =
            FreeArcReader::<Cursor<Vec<u8>>>::decompress_data("mm+grzip", &grzip_stream, 10, None)
                .unwrap();

        assert_eq!(decoded, b"RIFF\x0a\x14\x0d\x19\x0c\x1e");
    }
}

#[cfg(feature = "ffi-codecs")]
fn decompress_lzma_compat(data: &[u8], orig_size: usize) -> Result<Vec<u8>> {
    crate::codecs::lzma2::decompress_lzma_default(data, orig_size)
}

#[cfg(not(feature = "ffi-codecs"))]
fn decompress_lzma_compat(_data: &[u8], _orig_size: usize) -> Result<Vec<u8>> {
    Err(anyhow!(
        "LZMA FreeArc archive blocks require the temporary ffi-codecs feature until native LZMA wiring is implemented"
    ))
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
        let index = self
            .directory
            .files
            .iter()
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
