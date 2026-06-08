use std::io::{Read, Write};

use crate::error::{ArcError, Result};

const USE_DICT2: u8 = 1;

fn codec_err(msg: impl Into<String>) -> ArcError {
    ArcError::Codec {
        codec: "dict",
        message: msg.into(),
    }
}

/// Decode a single DictEncode block (mirrors `DictDecode()` in dict.cpp).
///
/// `input` is the raw encoded bytes for this block (not including the outer
/// i32 length header). Decoded bytes are written to `output`.
fn dict_decode_block(data: &[u8], output: &mut Vec<u8>) -> Result<()> {
    let mut pos = 0usize;

    let gb = |pos: &mut usize| -> Result<u8> {
        if *pos >= data.len() {
            return Err(codec_err("unexpected EOF in dict block"));
        }
        let b = data[*pos];
        *pos += 1;
        Ok(b)
    };

    // Phase 1: read 256 one-byte code lengths.
    // len == 0: literal passthrough; len == USE_DICT2 (1): 2-byte code prefix;
    // len >= 2: 1-byte code for a word of that length.
    let mut dict1_len = [0u8; 256];
    for i in 0..256 {
        dict1_len[i] = gb(&mut pos)?;
    }

    // Phase 2: for each i where dict1_len[i] == USE_DICT2, read 256 prefix
    // lengths for the 2-byte table row dict2[i][0..=255].
    let mut dict2_prefix_len = vec![[0u8; 256]; 256];
    for i in 0..256usize {
        if dict1_len[i] == USE_DICT2 {
            for j in 0..256usize {
                dict2_prefix_len[i][j] = gb(&mut pos)?;
            }
        }
    }

    // Phase 3: read word text for 1-byte codes (len != USE_DICT2, len > 0).
    let mut dict1_word: Vec<Vec<u8>> = vec![vec![]; 256];
    for i in 0..256usize {
        if dict1_len[i] != USE_DICT2 {
            let len = dict1_len[i] as usize;
            let mut word = vec![0u8; len];
            for k in 0..len {
                word[k] = gb(&mut pos)?;
            }
            dict1_word[i] = word;
        }
    }

    // Phase 4: read the separator byte used to delimit 2-byte words.
    let word_sep = gb(&mut pos)?;

    // Phase 5: build the 2-byte word table. Each entry (i,j) is built by
    // copying the first prefix_len bytes of the previous (i,j-1) word, then
    // reading bytes until word_sep.
    let mut dict2_word: Vec<Vec<Vec<u8>>> = vec![vec![vec![]; 256]; 256];
    for i in 0..256usize {
        if dict1_len[i] == USE_DICT2 {
            let mut prev: Option<Vec<u8>> = None;
            for j in 0..256usize {
                let prefix_len = dict2_prefix_len[i][j] as usize;
                let mut word: Vec<u8> = Vec::new();

                // Copy prefix_len bytes from the previous word.
                if prefix_len > 0 {
                    let pw = prev.as_ref().ok_or_else(|| {
                        codec_err("dict2: prefix copy from non-existent previous word")
                    })?;
                    let copy_len = prefix_len.min(pw.len());
                    word.extend_from_slice(&pw[..copy_len]);
                }

                // Read bytes until word_sep.
                loop {
                    let c = gb(&mut pos)?;
                    if c == word_sep {
                        break;
                    }
                    word.push(c);
                }

                prev = Some(word.clone());
                dict2_word[i][j] = word;
            }
        }
    }

    // Phase 6: read the prefix byte used to escape "stolen" characters.
    // The prefix byte is a 2-byte code starter; dict2(prefix, j) = [j].
    let prefix = gb(&mut pos)? as usize;
    dict1_len[prefix] = USE_DICT2;
    for j in 0..256usize {
        dict2_word[prefix][j] = vec![j as u8];
    }

    // Phase 7: decode the encoded text.
    while pos < data.len() {
        let c = data[pos] as usize;
        pos += 1;
        match dict1_len[c] {
            0 => output.push(c as u8),
            USE_DICT2 => {
                if pos >= data.len() {
                    return Err(codec_err("unexpected EOF reading 2-byte code"));
                }
                let c2 = data[pos] as usize;
                pos += 1;
                output.extend_from_slice(&dict2_word[c][c2]);
            }
            _ => {
                output.extend_from_slice(&dict1_word[c]);
            }
        }
    }

    Ok(())
}

/// Outer dict decompressor (mirrors `dict_decompress()` in C_Dict.cpp).
///
/// Wire format: loop of `i32 block_size`
///   - negative: passthrough block of `-block_size` raw bytes
///   - non-negative: DictDecode block of `block_size` bytes
/// Loop exits on EOF when reading `block_size`.
pub fn dict_decompress(input: &mut dyn Read, output: &mut dyn Write) -> Result<u64> {
    let mut bytes_out: u64 = 0;
    let mut len_buf = [0u8; 4];

    loop {
        // Try to read the next i32 block-size header.
        match input.read(&mut len_buf[..1])? {
            0 => break, // EOF
            _ => {}
        }
        if let Err(e) = input.read_exact(&mut len_buf[1..]) {
            return Err(ArcError::Codec {
                codec: "dict",
                message: format!("truncated block-size header: {e}"),
            });
        }
        let in_size = i32::from_le_bytes(len_buf);

        if in_size < 0 {
            // Passthrough block: copy -in_size raw bytes.
            let n = (-in_size) as usize;
            let mut buf = vec![0u8; n];
            input
                .read_exact(&mut buf)
                .map_err(|e| codec_err(e.to_string()))?;
            output.write_all(&buf)?;
            bytes_out += n as u64;
        } else {
            // Dict-encoded block of in_size bytes.
            let n = in_size as usize;
            let mut buf = vec![0u8; n];
            input
                .read_exact(&mut buf)
                .map_err(|e| codec_err(e.to_string()))?;
            let mut decoded: Vec<u8> = Vec::new();
            dict_decode_block(&buf, &mut decoded)?;
            output.write_all(&decoded)?;
            bytes_out += decoded.len() as u64;
        }
    }

    Ok(bytes_out)
}
