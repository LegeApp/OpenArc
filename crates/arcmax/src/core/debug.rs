//! Debugging utilities for archive format reverse-engineering

use std::io::{Read, Seek, SeekFrom};
use anyhow::Result;

pub struct ArchiveDebugger;

impl ArchiveDebugger {
    /// Dump the raw footer block to understand structure
    pub fn dump_footer_region<R: Read + Seek>(reader: &mut R, window_size: u64) -> Result<()> {
        let file_size = reader.seek(SeekFrom::End(0))?;
        let scan_start = if file_size > window_size {
            file_size - window_size
        } else {
            0
        };

        reader.seek(SeekFrom::Start(scan_start))?;
        let mut buffer = vec![0u8; (file_size - scan_start) as usize];
        reader.read_exact(&mut buffer)?;

        println!("\n=== FOOTER REGION DUMP (last {} bytes) ===", file_size - scan_start);
        println!("File size: {} bytes", file_size);
        println!("Scan window: {} to {} (size: {})\n", scan_start, file_size, buffer.len());

        // Find all ArC\x01 signatures
        let signature = [0x41, 0x72, 0x43, 0x01];
        let mut positions = Vec::new();
        for i in 0..buffer.len().saturating_sub(3) {
            if buffer[i..i+4] == signature {
                positions.push(scan_start + i as u64);
            }
        }

        println!("Found {} FreeArc signature(s) at offset(s):", positions.len());
        for pos in &positions {
            println!("  - {}", pos);
        }

        // Hex dump around each signature
        for &sig_pos in &positions {
            let offset = (sig_pos - scan_start) as usize;
            println!("\n--- Signature at file offset {} ---", sig_pos);
            Self::hex_dump(&buffer, offset, 256);
        }

        Ok(())
    }

    /// Hex dump with ASCII representation
    pub fn hex_dump(data: &[u8], start_offset: usize, length: usize) {
        let end = std::cmp::min(start_offset + length, data.len());
        let mut offset = start_offset;

        while offset < end {
            print!("{:08x}: ", offset);

            // Hex bytes
            for i in 0..16 {
                if offset + i < end {
                    print!("{:02x} ", data[offset + i]);
                } else {
                    print!("   ");
                }
            }

            print!(" | ");

            // ASCII
            for i in 0..16 {
                if offset + i < end {
                    let b = data[offset + i];
                    if b >= 32 && b <= 126 {
                        print!("{}", b as char);
                    } else {
                        print!(".");
                    }
                } else {
                    print!(" ");
                }
            }

            println!();
            offset += 16;
        }
    }

    /// Parse and display footer block structure byte-by-byte
    pub fn analyze_footer_descriptor<R: Read + Seek>(
        reader: &mut R,
        footer_offset: u64,
    ) -> Result<()> {
        reader.seek(SeekFrom::Start(footer_offset))?;

        let mut buf = [0u8; 1024];
        let bytes_read = reader.read(&mut buf)?;

        println!("\n=== FOOTER DESCRIPTOR ANALYSIS ===");
        println!("Offset: {} (0x{:x})", footer_offset, footer_offset);
        println!("Bytes read: {}\n", bytes_read);

        let mut pos = 0;

        // Signature
        println!("Bytes 0-3: Signature: {:?}", &buf[0..4]);
        pos = 4;

        // Block type
        println!("Byte 4: Block type: {} (0x{:02x})", buf[4], buf[4]);
        pos = 5;

        // Compressor string (null-terminated)
        let comp_start = pos;
        while pos < bytes_read && buf[pos] != 0 {
            pos += 1;
        }
        if pos < bytes_read {
            let comp_str = String::from_utf8_lossy(&buf[comp_start..pos]);
            println!("Bytes {}-{}: Compressor string: \"{}\"", comp_start, pos, comp_str);
            pos += 1; // Skip null terminator
        }

        // Variable ints
        println!("\nRemaining bytes as variables:");
        for i in 0..4 {
            if pos + 4 <= bytes_read {
                let val_bytes = &buf[pos..pos+4];
                let val = u32::from_le_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]]);
                println!("  Bytes {}-{}: {:08x} (le_u32) / {}", pos, pos+3, val, val);
                pos += 4;
            }
        }

        println!("\n--- Hex dump of entire descriptor ---");
        Self::hex_dump(&buf, 0, bytes_read);

        Ok(())
    }
}