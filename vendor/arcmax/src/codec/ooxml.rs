//! OOXML / ODF ZIP unwrapper codec.
//!
//! `.docx`, `.xlsx`, `.pptx` (OOXML) and `.odt`, `.ods`, `.odp` (ODF) are ZIP
//! containers whose XML parts are independently deflate-compressed. That
//! prevents cross-part dedup and severely limits the entropy coder's view.
//!
//! This codec strips the ZIP layer, exposing raw XML bytes to the downstream
//! compressor (typically PPMd or LZMA), and reconstructs a valid stored ZIP on
//! the other side. The result is semantically identical to the original (all
//! content preserved), but not bit-for-bit identical because entries are
//! re-stored without deflate (lossy mode).
//!
//! ## Wire format (output of `compress`)
//!
//! ```text
//! [magic: b"OOXML"][version: u8 = 1][entry_count: u32 LE]
//! For each entry:
//!   [name_len: u16 LE][name: UTF-8 bytes]
//!   [crc32: u32 LE]
//!   [data_len: u32 LE][data: raw uncompressed bytes]
//! ```

use std::io::{Cursor, Read, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

const MAGIC: &[u8; 5] = b"OOXML";
const VERSION: u8 = 1;

pub struct OoxmlCodec;

impl Codec for OoxmlCodec {
    fn name(&self) -> &'static str {
        "ooxml"
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage::default()
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut buf = Vec::new();
        let bytes_in = input.read_to_end(&mut buf)? as u64;

        let cursor = Cursor::new(&buf);
        let mut archive =
            ZipArchive::new(cursor).map_err(|e| ooxml_err(format!("not a ZIP: {e}")))?;

        if archive.len() == 0 {
            return Err(ooxml_err("empty ZIP archive".into()));
        }

        // Require the OOXML or ODF content-type marker.
        let is_ooxml = archive.by_name("[Content_Types].xml").is_ok();
        let is_odf = archive.by_name("mimetype").is_ok();
        if !is_ooxml && !is_odf {
            return Err(ooxml_err(
                "not an OOXML or ODF file (missing [Content_Types].xml / mimetype)".into(),
            ));
        }

        // Collect filenames first to avoid overlapping borrows.
        let names: Vec<String> = archive.file_names().map(str::to_owned).collect();

        let mut out: Vec<u8> = Vec::new();

        // Write stream header.
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&(names.len() as u32).to_le_bytes());

        for name in &names {
            let mut entry = archive
                .by_name(name)
                .map_err(|e| ooxml_err(format!("cannot open entry '{name}': {e}")))?;

            let crc32 = entry.crc32();

            // read_to_end transparently decompresses deflated entries.
            let mut data = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut data)
                .map_err(|e| ooxml_err(format!("cannot read entry '{name}': {e}")))?;

            let name_bytes = name.as_bytes();
            let name_len = u16::try_from(name_bytes.len())
                .map_err(|_| ooxml_err(format!("filename too long: {name}")))?;
            let data_len = u32::try_from(data.len())
                .map_err(|_| ooxml_err(format!("entry too large: {name}")))?;

            out.extend_from_slice(&name_len.to_le_bytes());
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(&crc32.to_le_bytes());
            out.extend_from_slice(&data_len.to_le_bytes());
            out.extend_from_slice(&data);
        }

        let bytes_out = out.len() as u64;
        output.write_all(&out)?;
        Ok(CodecReport {
            bytes_in,
            bytes_out,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut buf = Vec::new();
        let bytes_in = input.read_to_end(&mut buf)? as u64;
        let mut cur = Cursor::new(buf.as_slice());

        // Verify magic and version.
        let mut magic = [0u8; 5];
        cur.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(ooxml_err(format!(
                "invalid magic: expected OOXML, got {:02x?}",
                magic
            )));
        }
        let mut ver = [0u8; 1];
        cur.read_exact(&mut ver)?;
        if ver[0] != VERSION {
            return Err(ooxml_err(format!("unsupported version {}", ver[0])));
        }
        let mut count_buf = [0u8; 4];
        cur.read_exact(&mut count_buf)?;
        let n_entries = u32::from_le_bytes(count_buf) as usize;

        // Reconstruct as a STORED ZIP (no re-compression).
        let zip_cursor: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(zip_cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for _ in 0..n_entries {
            let mut name_len_buf = [0u8; 2];
            cur.read_exact(&mut name_len_buf)?;
            let name_len = u16::from_le_bytes(name_len_buf) as usize;

            let mut name_bytes = vec![0u8; name_len];
            cur.read_exact(&mut name_bytes)?;
            let name = String::from_utf8(name_bytes)
                .map_err(|_| ooxml_err("invalid UTF-8 in filename".into()))?;

            // crc32 field is preserved but ZipWriter recomputes it from data.
            let mut crc32_buf = [0u8; 4];
            cur.read_exact(&mut crc32_buf)?;

            let mut data_len_buf = [0u8; 4];
            cur.read_exact(&mut data_len_buf)?;
            let data_len = u32::from_le_bytes(data_len_buf) as usize;

            let mut data = vec![0u8; data_len];
            cur.read_exact(&mut data)?;

            writer
                .start_file(&name, options)
                .map_err(|e| ooxml_err(format!("cannot start entry '{name}': {e}")))?;
            writer
                .write_all(&data)
                .map_err(|e| ooxml_err(format!("cannot write entry '{name}': {e}")))?;
        }

        let zip_cursor = writer
            .finish()
            .map_err(|e| ooxml_err(format!("cannot finalise ZIP: {e}")))?;
        let zip_bytes = zip_cursor.into_inner();
        let bytes_out = zip_bytes.len() as u64;
        output.write_all(&zip_bytes)?;
        Ok(CodecReport {
            bytes_in,
            bytes_out,
        })
    }
}

fn ooxml_err(message: String) -> ArcError {
    ArcError::Codec {
        codec: "ooxml",
        message,
    }
}
