use std::io::{Read, Write};

use crate::error::{ArcError, Result};

fn codec_err(msg: impl Into<String>) -> ArcError {
    ArcError::Codec {
        codec: "rep",
        message: msg.into(),
    }
}

fn read_i32_le_from(buf: &[u8], pos: &mut usize) -> Result<i32> {
    if *pos + 4 > buf.len() {
        return Err(codec_err("truncated REP block"));
    }
    let v = i32::from_le_bytes(buf[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    Ok(v)
}

pub fn rep_decompress(input: &mut dyn Read, output: &mut dyn Write) -> Result<u64> {
    // The actual block_size is stored in the first 4 bytes of the stream.
    let block_size = match try_read_u32_le(input)? {
        Some(v) => v as usize,
        None => return Ok(0), // empty stream
    };
    if block_size == 0 {
        return Ok(0);
    }

    let mut ring = vec![0u8; block_size];
    let mut write_pos: usize = 0;
    let mut bytes_out: u64 = 0;

    loop {
        // Read compressed-block size (0 == EOF).
        let compr_size = match try_read_u32_le(input)? {
            Some(v) => v as usize,
            None => break,
        };
        if compr_size == 0 {
            break;
        }

        let mut blk = vec![0u8; compr_size];
        input
            .read_exact(&mut blk)
            .map_err(|e| codec_err(e.to_string()))?;
        let mut cur = 0usize;

        let num = read_i32_le_from(&blk, &mut cur)? as usize;

        if 4 + num * 8 + (num + 1) * 4 > blk.len() {
            return Err(codec_err("REP block header overruns buffer"));
        }

        let mut lens = vec![0i32; num];
        let mut offsets = vec![0i32; num];
        let mut datalens = vec![0i32; num + 1];

        for l in &mut lens {
            *l = read_i32_le_from(&blk, &mut cur)?;
        }
        for o in &mut offsets {
            *o = read_i32_le_from(&blk, &mut cur)?;
        }
        for d in &mut datalens {
            *d = read_i32_le_from(&blk, &mut cur)?;
        }

        for i in 0..=num {
            let datalen = datalens[i] as usize;

            // Literal bytes
            if cur + datalen > blk.len() {
                return Err(codec_err("REP literal data overruns block"));
            }
            output.write_all(&blk[cur..cur + datalen])?;
            for &b in &blk[cur..cur + datalen] {
                ring[write_pos] = b;
                write_pos = (write_pos + 1) % block_size;
            }
            cur += datalen;
            bytes_out += datalen as u64;

            if i == num {
                break;
            }

            // LZ match: copy from ring buffer
            let offset = offsets[i] as usize;
            let match_len = lens[i] as usize;

            if offset == 0 || offset > block_size {
                return Err(codec_err(format!(
                    "REP invalid match offset {offset} (block_size={block_size})"
                )));
            }

            for _ in 0..match_len {
                let src = (write_pos + block_size - offset) % block_size;
                let b = ring[src];
                output.write_all(&[b])?;
                ring[write_pos] = b;
                write_pos = (write_pos + 1) % block_size;
            }
            bytes_out += match_len as u64;
        }
    }

    Ok(bytes_out)
}

fn try_read_u32_le(r: &mut dyn Read) -> Result<Option<u32>> {
    let mut buf = [0u8; 4];
    match r.read(&mut buf[..1])? {
        0 => return Ok(None),
        _ => {}
    }
    r.read_exact(&mut buf[1..])
        .map_err(|e| codec_err(e.to_string()))?;
    Ok(Some(u32::from_le_bytes(buf)))
}
