use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};

use crate::codec::dict::options::DictOptions;
use crate::error::{ArcError, Result};

const USE_DICT2: u8 = 1;
const MAX_WORD_LEN: usize = 32;
const MIN_WORD_LEN: usize = 3;

#[derive(Debug, Clone)]
struct DictWord {
    code: u8,
    bytes: Vec<u8>,
}

fn codec_err(msg: impl Into<String>) -> ArcError {
    ArcError::Codec {
        codec: "dict",
        message: msg.into(),
    }
}

fn write_i32_le(w: &mut dyn Write, v: i32) -> Result<()> {
    w.write_all(&v.to_le_bytes()).map_err(ArcError::Io)
}

fn checked_i32(v: usize, what: &str) -> Result<i32> {
    i32::try_from(v).map_err(|_| codec_err(format!("DICT {what} exceeds i32 range: {v}")))
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn count_words(block: &[u8]) -> HashMap<Vec<u8>, usize> {
    let mut counts = HashMap::new();
    let mut pos = 0usize;
    while pos < block.len() {
        if !is_word_byte(block[pos]) {
            pos += 1;
            continue;
        }

        let start = pos;
        while pos < block.len() && is_word_byte(block[pos]) {
            pos += 1;
        }
        let token = &block[start..pos];
        if token.len() >= MIN_WORD_LEN {
            let len = token.len().min(MAX_WORD_LEN);
            *counts.entry(token[..len].to_vec()).or_insert(0) += 1;
        }
    }
    counts
}

fn choose_words(block: &[u8]) -> (u8, Vec<DictWord>) {
    let mut byte_counts = [0usize; 256];
    for &b in block {
        byte_counts[b as usize] += 1;
    }

    let mut rare_bytes = (0u8..=255).collect::<Vec<_>>();
    rare_bytes.sort_by_key(|&b| (byte_counts[b as usize], b));

    let prefix = rare_bytes[0];
    let mut available_codes = rare_bytes
        .into_iter()
        .filter(|&b| b != prefix)
        .collect::<Vec<_>>();

    let mut candidates = count_words(block)
        .into_iter()
        .filter(|(word, count)| *count >= 2 && word.len() >= MIN_WORD_LEN)
        .map(|(word, count)| {
            let gross_gain = count * (word.len() - 1);
            (word, count, gross_gain)
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut selected = Vec::new();
    for (word, count, gross_gain) in candidates {
        if selected.len() >= 255 || available_codes.is_empty() {
            break;
        }

        let code = available_codes.remove(0);
        let escape_cost = byte_counts[code as usize];
        let header_cost = word.len();
        if gross_gain <= escape_cost + header_cost || count < 2 {
            continue;
        }

        selected.push(DictWord { code, bytes: word });
    }

    (prefix, selected)
}

fn encode_text(block: &[u8], prefix: u8, words: &[DictWord]) -> Vec<u8> {
    let mut by_first: HashMap<u8, Vec<&DictWord>> = HashMap::new();
    let stolen = words.iter().map(|w| w.code).collect::<HashSet<_>>();
    for word in words {
        by_first.entry(word.bytes[0]).or_default().push(word);
    }
    for bucket in by_first.values_mut() {
        bucket.sort_by(|a, b| b.bytes.len().cmp(&a.bytes.len()));
    }

    let mut encoded = Vec::with_capacity(block.len());
    let mut pos = 0usize;
    while pos < block.len() {
        let mut matched = None;
        if let Some(bucket) = by_first.get(&block[pos]) {
            for word in bucket {
                if block[pos..].starts_with(&word.bytes) {
                    matched = Some(*word);
                    break;
                }
            }
        }

        if let Some(word) = matched {
            encoded.push(word.code);
            pos += word.bytes.len();
        } else {
            let b = block[pos];
            if b == prefix || stolen.contains(&b) {
                encoded.push(prefix);
            }
            encoded.push(b);
            pos += 1;
        }
    }

    encoded
}

fn dict_encode_block(block: &[u8]) -> Option<Vec<u8>> {
    let (prefix, words) = choose_words(block);
    if words.is_empty() {
        return None;
    }

    let mut dict1_len = [0u8; 256];
    for word in &words {
        dict1_len[word.code as usize] = word.bytes.len() as u8;
    }

    let mut out = Vec::new();
    out.extend_from_slice(&dict1_len);
    for i in 0..256usize {
        if dict1_len[i] != 0 && dict1_len[i] != USE_DICT2 {
            let word = words
                .iter()
                .find(|word| word.code as usize == i)
                .expect("word for non-zero dict1 length");
            out.extend_from_slice(&word.bytes);
        }
    }

    // No 2-byte dictionary rows are emitted by this encoder, but the wire
    // format still carries the row separator and the escape-prefix byte.
    out.push(0);
    out.push(prefix);
    out.extend_from_slice(&encode_text(block, prefix, &words));

    Some(out)
}

/// Dict encoder.
///
/// This emits fully compatible Dict streams. The current native encoder uses
/// one-byte dictionary substitutions and relies on the original passthrough
/// block form when the encoded block does not satisfy `min_compression`.
pub fn dict_compress(
    input: &mut dyn Read,
    output: &mut dyn Write,
    opts: &DictOptions,
) -> Result<u64> {
    if opts.block_size == 0 {
        return Err(codec_err("DICT block size must be non-zero"));
    }

    let mut bytes_out = 0u64;
    let mut block = vec![0u8; opts.block_size];

    loop {
        let mut filled = 0usize;
        while filled < opts.block_size {
            let n = input.read(&mut block[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        if filled == 0 {
            break;
        }

        let raw = &block[..filled];
        let encoded = dict_encode_block(raw);
        let use_encoded = encoded.as_ref().is_some_and(|encoded| {
            opts.min_compression <= 0
                || encoded.len() < (filled * opts.min_compression as usize) / 100
        });

        if use_encoded {
            let encoded = encoded.unwrap();
            write_i32_le(output, checked_i32(encoded.len(), "encoded block size")?)?;
            output.write_all(&encoded)?;
            bytes_out += 4 + encoded.len() as u64;
        } else {
            write_i32_le(output, -checked_i32(filled, "raw block size")?)?;
            output.write_all(raw)?;
            bytes_out += 4 + filled as u64;
        }

        if filled < opts.block_size {
            break;
        }
    }

    Ok(bytes_out)
}
