//! XZ container format codec.
//!
//! Produces standards-compliant `.xz` files using a single-block stream with
//! LZMA2 compression and CRC32 integrity check.  The format is defined at
//! <https://tukaani.org/xz/xz-file-format-1.0.4.txt>.
//!
//! Framing: unlike the internal `LzmaCodec`, this codec produces self-contained
//! XZ bytes with no extra size header.  The XZ stream footer lets the decoder
//! locate the index without knowing the uncompressed size in advance.

use std::io::{Cursor, Read, Write};

use lzma_rust2::{CountingWriter, LZMA2Options, LZMA2Reader, LZMA2Writer};

use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::codec::xz::XzOptions;
use crate::error::{ArcError, Result};

// ── Format constants ──────────────────────────────────────────────────────────

/// XZ stream header magic: `\xfd7zXZ\x00`
const STREAM_HEADER_MAGIC: [u8; 6] = [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00];
/// XZ stream footer magic: `YZ`
const STREAM_FOOTER_MAGIC: [u8; 2] = [0x59, 0x5A];
/// Stream flags: CRC32 block check.
const CHECK_CRC32: u16 = 0x0001;
/// LZMA2 filter identifier.
const FILTER_LZMA2: u64 = 0x21;

// ── VLI helpers ───────────────────────────────────────────────────────────────

/// Write an unsigned LEB128 VLI into `buf`.
fn write_vli(buf: &mut Vec<u8>, mut n: u64) {
    loop {
        let byte = (n & 0x7F) as u8;
        n >>= 7;
        if n == 0 {
            buf.push(byte);
            break;
        }
        buf.push(byte | 0x80);
    }
}

/// Read one unsigned LEB128 VLI from `data` starting at `*pos`.
fn read_vli(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut result = 0u64;
    let mut shift = 0u32;
    loop {
        if *pos >= data.len() || shift > 63 {
            return None;
        }
        let byte = data[*pos];
        *pos += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Some(result);
        }
    }
}

// ── CRC32 helper ──────────────────────────────────────────────────────────────

fn crc32(data: &[u8]) -> u32 {
    let mut h = crc32fast::Hasher::new();
    h.update(data);
    h.finalize()
}

// ── LZMA2 dict-size encoding ──────────────────────────────────────────────────

/// Return the XZ LZMA2 property byte for the given `dict_size`.
///
/// The encoding formula (from the XZ spec §5.3.1):
///   dict_size(d) = (2 | (d & 1)) << (d / 2 + 11)
///
/// We pick the smallest `d` whose resulting size is ≥ `dict_size`.
fn dict_size_to_prop(dict_size: u32) -> u8 {
    if dict_size >= u32::MAX {
        return 40;
    }
    for d in 0u8..40 {
        let sz = (2u32 | (d as u32 & 1)) << (d as u32 / 2 + 11);
        if sz >= dict_size {
            return d;
        }
    }
    40
}

/// Decode an XZ LZMA2 property byte back to a dict size in bytes.
fn prop_to_dict_size(prop: u8) -> u32 {
    if prop == 40 {
        u32::MAX
    } else {
        (2u32 | (prop as u32 & 1)) << (prop as u32 / 2 + 11)
    }
}

// ── XzCodec ───────────────────────────────────────────────────────────────────

pub struct XzCodec {
    options: XzOptions,
}

impl XzCodec {
    pub fn new(options: XzOptions) -> Self {
        Self { options }
    }

    fn lzma2_opts(&self) -> LZMA2Options {
        let preset = self.options.level.unwrap_or(6).min(9) as u32;
        let mut opts = LZMA2Options::with_preset(preset);
        if self.options.dict_size > 0 {
            opts.dict_size = self.options.dict_size;
        }
        opts
    }
}

impl Codec for XzCodec {
    fn name(&self) -> &'static str {
        "xz"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut source = Vec::new();
        input.read_to_end(&mut source)?;
        let uncompressed_len = source.len() as u64;

        // ── Compress with LZMA2 ───────────────────────────────────────────────
        let opts = self.lzma2_opts();
        let dict_size = opts.dict_size;
        let mut lzma2_data = Vec::new();
        {
            let cw = CountingWriter::new(&mut lzma2_data);
            let mut writer = LZMA2Writer::new(cw, &opts);
            writer.write_all(&source).map_err(|e| ArcError::Codec {
                codec: "xz",
                message: format!("LZMA2 encode failed: {e}"),
            })?;
            writer.finish().map_err(|e| ArcError::Codec {
                codec: "xz",
                message: format!("LZMA2 finish failed: {e}"),
            })?;
        }
        let compressed_len = lzma2_data.len() as u64;

        // CRC32 of the uncompressed data (block integrity check).
        let block_check = crc32(&source);

        // ── Stream Header (12 bytes) ──────────────────────────────────────────
        let stream_flags: u16 = CHECK_CRC32;
        let flags_bytes = stream_flags.to_le_bytes();
        let header_crc = crc32(&flags_bytes);
        output.write_all(&STREAM_HEADER_MAGIC)?;
        output.write_all(&flags_bytes)?;
        output.write_all(&header_crc.to_le_bytes())?;

        // ── Block Header ─────────────────────────────────────────────────────
        //
        //  [Header Size byte][Block Flags][Comp Size VLI][Uncomp Size VLI]
        //  [Filter ID VLI][Prop Size VLI][Properties][Padding][CRC32]
        //
        //  Total block header size must be a multiple of 4.
        //  Header Size byte = (total / 4) - 1.

        // Block Flags: 0 extra filters (1 total), both sizes present.
        let block_flags: u8 = 0b1100_0000; // bit7=uncomp present, bit6=comp present
        let dict_prop = dict_size_to_prop(dict_size);

        let mut inner = Vec::new();
        inner.push(block_flags);
        write_vli(&mut inner, compressed_len);
        write_vli(&mut inner, uncompressed_len);
        write_vli(&mut inner, FILTER_LZMA2);
        write_vli(&mut inner, 1u64); // 1 properties byte
        inner.push(dict_prop);

        // Pad so that 1 (size byte) + inner.len() + padding + 4 (CRC) is a
        // multiple of 4.
        let without_crc = 1 + inner.len();
        let padding = (4 - ((without_crc + 4) % 4)) % 4;
        let total_header_size = without_crc + padding + 4;
        debug_assert_eq!(total_header_size % 4, 0);

        let header_size_byte = (total_header_size / 4 - 1) as u8;

        let mut header_for_crc = Vec::with_capacity(total_header_size - 4);
        header_for_crc.push(header_size_byte);
        header_for_crc.extend_from_slice(&inner);
        header_for_crc.extend(std::iter::repeat(0u8).take(padding));

        let block_header_crc = crc32(&header_for_crc);
        output.write_all(&header_for_crc)?;
        output.write_all(&block_header_crc.to_le_bytes())?;

        // ── Compressed Data ───────────────────────────────────────────────────
        output.write_all(&lzma2_data)?;

        // ── Block Padding (align compressed data to 4 bytes) ─────────────────
        let data_padding = (4 - (compressed_len as usize % 4)) % 4;
        let zeros = [0u8; 3];
        output.write_all(&zeros[..data_padding])?;

        // ── Block Check (CRC32 of uncompressed data) ──────────────────────────
        output.write_all(&block_check.to_le_bytes())?;

        // ── Index ─────────────────────────────────────────────────────────────
        //
        //  [0x00 indicator][num_records VLI]
        //  [unpadded_size VLI][uncomp_size VLI] × num_records
        //  [Padding][CRC32]
        //
        //  Unpadded size = block_header_size + compressed_data_size
        //  (no padding, no check).

        let unpadded_size = total_header_size as u64 + compressed_len;

        let mut index_body = Vec::new();
        index_body.push(0x00u8); // index indicator
        write_vli(&mut index_body, 1); // 1 block
        write_vli(&mut index_body, unpadded_size);
        write_vli(&mut index_body, uncompressed_len);

        let index_padding = (4 - (index_body.len() % 4)) % 4;
        index_body.extend(std::iter::repeat(0u8).take(index_padding));

        let index_crc = crc32(&index_body);
        let index_total = index_body.len() as u64 + 4; // +4 for CRC

        output.write_all(&index_body)?;
        output.write_all(&index_crc.to_le_bytes())?;

        // ── Stream Footer (12 bytes) ──────────────────────────────────────────
        //
        //  [CRC32][Backward Size][Stream Flags][59 5A]
        //
        //  Backward Size = (index_total_bytes / 4) - 1.

        let backward_size = (index_total / 4 - 1) as u32;
        let mut footer_data = Vec::with_capacity(8);
        footer_data.extend_from_slice(&backward_size.to_le_bytes());
        footer_data.extend_from_slice(&stream_flags.to_le_bytes());
        let footer_crc = crc32(&footer_data);

        output.write_all(&footer_crc.to_le_bytes())?;
        output.write_all(&backward_size.to_le_bytes())?;
        output.write_all(&stream_flags.to_le_bytes())?;
        output.write_all(&STREAM_FOOTER_MAGIC)?;

        let bytes_out = 12
            + total_header_size as u64
            + compressed_len
            + data_padding as u64
            + 4 // block check
            + index_total
            + 12; // stream footer

        Ok(CodecReport {
            bytes_in: uncompressed_len,
            bytes_out,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut data = Vec::new();
        input.read_to_end(&mut data)?;
        let bytes_in = data.len() as u64;

        // ── Stream Header ─────────────────────────────────────────────────────
        if data.len() < 12 {
            return Err(bad("stream header truncated"));
        }
        if &data[..6] != &STREAM_HEADER_MAGIC {
            return Err(bad("bad stream header magic"));
        }
        let stream_flags = u16::from_le_bytes([data[6], data[7]]);
        let expected_hcrc = u32::from_le_bytes(data[8..12].try_into().unwrap());
        if crc32(&data[6..8]) != expected_hcrc {
            return Err(bad("stream header CRC32 mismatch"));
        }
        let check_type = stream_flags & 0x0F;

        let mut pos = 12usize;

        // ── Block Header ─────────────────────────────────────────────────────
        if pos >= data.len() {
            return Err(bad("missing block header"));
        }
        let header_size_byte = data[pos] as usize;
        if header_size_byte == 0 {
            // Index indicator — stream has no blocks.
            output.write_all(&[])?;
            return Ok(CodecReport {
                bytes_in,
                bytes_out: 0,
            });
        }
        let total_header_size = (header_size_byte + 1) * 4;
        if pos + total_header_size > data.len() {
            return Err(bad("block header truncated"));
        }

        let crc_pos = pos + total_header_size - 4;
        let expected_bcrc = u32::from_le_bytes(data[crc_pos..crc_pos + 4].try_into().unwrap());
        if crc32(&data[pos..crc_pos]) != expected_bcrc {
            return Err(bad("block header CRC32 mismatch"));
        }

        let mut hpos = pos + 1; // skip Header Size byte
        let block_flags = data[hpos];
        hpos += 1;

        let has_compressed_size = (block_flags >> 6) & 1 == 1;
        let has_uncompressed_size = (block_flags >> 7) & 1 == 1;

        let compressed_size = if has_compressed_size {
            read_vli(&data, &mut hpos).ok_or_else(|| bad("VLI decode error (compressed size)"))?
        } else {
            return Err(bad(
                "block without Compressed Size field is not supported; use xz tool default",
            ));
        };

        let uncompressed_size = if has_uncompressed_size {
            read_vli(&data, &mut hpos).ok_or_else(|| bad("VLI decode error (uncompressed size)"))?
        } else {
            0
        };

        // Filter flags: read Filter ID, prop size, and dict prop byte.
        let _filter_id = read_vli(&data, &mut hpos).unwrap_or(0);
        let prop_size = read_vli(&data, &mut hpos).unwrap_or(0) as usize;
        let dict_prop = if prop_size > 0 && hpos < crc_pos {
            data[hpos]
        } else {
            22
        };
        // hpos += prop_size; — we only need the prop byte; advance past block header below
        let dict_size = prop_to_dict_size(dict_prop.min(40));

        pos += total_header_size;

        // ── Compressed Data ───────────────────────────────────────────────────
        let comp_end = pos + compressed_size as usize;
        if comp_end > data.len() {
            return Err(bad("compressed data truncated"));
        }

        let mut reader = LZMA2Reader::new(Cursor::new(&data[pos..comp_end]), dict_size, None);
        let capacity = if uncompressed_size > 0 {
            uncompressed_size as usize
        } else {
            0
        };
        let mut decompressed = Vec::with_capacity(capacity);
        reader
            .read_to_end(&mut decompressed)
            .map_err(|e| ArcError::Codec {
                codec: "xz",
                message: format!("LZMA2 decode failed: {e}"),
            })?;

        // ── Block Check ───────────────────────────────────────────────────────
        if check_type == 1 {
            // CRC32 of uncompressed data sits after block padding.
            let data_padding = (4 - (compressed_size as usize % 4)) % 4;
            let check_pos = comp_end + data_padding;
            if check_pos + 4 <= data.len() {
                let file_check =
                    u32::from_le_bytes(data[check_pos..check_pos + 4].try_into().unwrap());
                if crc32(&decompressed) != file_check {
                    return Err(bad("block CRC32 mismatch — data corrupt"));
                }
            }
        }

        let bytes_out = decompressed.len() as u64;
        output.write_all(&decompressed)?;
        Ok(CodecReport {
            bytes_in,
            bytes_out,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        let dict = self.options.dict_size.max(4096) as u64;
        MemoryUsage {
            working_bytes: dict * 2,
            ..MemoryUsage::default()
        }
    }
}

fn bad(msg: &'static str) -> ArcError {
    ArcError::Codec {
        codec: "xz",
        message: msg.to_string(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
