//! JSON/XML token-dictionary pre-compressor.
//!
//! JSON and XML are repetition-heavy at the *token* level: property names,
//! element names, namespace prefixes, and attribute names repeat thousands of
//! times. PPMd/LZMA model them well but still pay several context bits per
//! occurrence. This filter replaces the top-255 highest-frequency word tokens
//! with 2-byte escape sequences, reducing entropy for the downstream coder.
//!
//! ## What counts as a token?
//!
//! Any maximal sequence of *word characters* (`A-Za-z0-9_:.-`) that is at
//! least 4 bytes long. This captures JSON property names (the quotes are
//! non-word chars and stay in place), XML element names, attribute names,
//! namespace prefixes, and MIME-type strings.
//!
//! ## Wire format (output of `compress`)
//!
//! ```text
//! [magic: b"TKND"][version: u8 = 1]
//! [dict_len: u8]  — number of dictionary entries (0..=255)
//! For each entry (in order, index = position):
//!   [entry_len: u8][entry_bytes: entry_len bytes]
//! [body: input bytes with substitutions applied]
//! ```
//!
//! In the body:
//! * `0xFE N` where `N < dict_len` → token at index N
//! * `0xFE 0xFF`                   → literal byte `0xFE`
//! * any other byte                → unchanged

use std::collections::HashMap;
use std::io::{Read, Write};

use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

const MAGIC: &[u8; 4] = b"TKND";
const VERSION: u8 = 1;
const ESCAPE: u8 = 0xFE;
const ESCAPE_SELF: u8 = 0xFF;
/// Minimum word length to be eligible for the dictionary.
const MIN_TOKEN_LEN: usize = 4;
/// Maximum number of dictionary entries.
const MAX_DICT_ENTRIES: usize = 255;

pub struct TokenDictCodec;

impl Codec for TokenDictCodec {
    fn name(&self) -> &'static str {
        "tokendict"
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage::default()
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut buf = Vec::new();
        let bytes_in = input.read_to_end(&mut buf)? as u64;

        let dict = build_dict(&buf);
        let body = encode_body(&buf, &dict);

        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(dict.len() as u8);
        for entry in &dict {
            out.push(entry.len() as u8);
            out.extend_from_slice(entry);
        }
        out.extend_from_slice(&body);

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

        if buf.len() < MAGIC.len() + 2 {
            return Err(tknd_err("stream too short".into()));
        }
        if &buf[..4] != MAGIC {
            return Err(tknd_err(format!(
                "invalid magic: expected TKND, got {:02x?}",
                &buf[..4]
            )));
        }
        if buf[4] != VERSION {
            return Err(tknd_err(format!("unsupported version {}", buf[4])));
        }

        let dict_len = buf[5] as usize;
        let mut pos = 6usize;

        let mut dict: Vec<Vec<u8>> = Vec::with_capacity(dict_len);
        for _ in 0..dict_len {
            if pos >= buf.len() {
                return Err(tknd_err("truncated dictionary".into()));
            }
            let entry_len = buf[pos] as usize;
            pos += 1;
            if pos + entry_len > buf.len() {
                return Err(tknd_err("truncated dictionary entry".into()));
            }
            dict.push(buf[pos..pos + entry_len].to_vec());
            pos += entry_len;
        }

        let body = &buf[pos..];
        let plain = decode_body(body, &dict)?;
        let bytes_out = plain.len() as u64;
        output.write_all(&plain)?;
        Ok(CodecReport {
            bytes_in,
            bytes_out,
        })
    }
}

// --- helpers -----------------------------------------------------------------

#[inline]
fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b':' || b == b'.' || b == b'-'
}

/// Build a dictionary of the top tokens ranked by `(freq - 1) * len`,
/// which estimates the bytes saved by substituting each token.
fn build_dict(input: &[u8]) -> Vec<Vec<u8>> {
    let mut freq: HashMap<&[u8], usize> = HashMap::new();
    let mut i = 0;
    while i < input.len() {
        if is_word_char(input[i]) {
            let start = i;
            while i < input.len() && is_word_char(input[i]) {
                i += 1;
            }
            let word = &input[start..i];
            if word.len() >= MIN_TOKEN_LEN {
                *freq.entry(word).or_default() += 1;
            }
        } else {
            i += 1;
        }
    }

    let mut scored: Vec<(&[u8], usize)> = freq
        .into_iter()
        .filter(|&(_, f)| f >= 2)
        .map(|(w, f)| {
            // Each substitution saves (word_len - 2) bytes; first occurrence
            // adds overhead (word_len + 1 bytes in the header), so net savings
            // only materialise at frequency >= 2.
            let saving = (f - 1) * w.len().saturating_sub(2);
            (w, saving)
        })
        .filter(|&(_, s)| s > 0)
        .collect();

    // Sort by savings descending; break ties by token (deterministic output).
    scored.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    scored
        .into_iter()
        .take(MAX_DICT_ENTRIES)
        .map(|(w, _)| w.to_vec())
        .collect()
}

fn encode_body(input: &[u8], dict: &[Vec<u8>]) -> Vec<u8> {
    let token_map: HashMap<&[u8], u8> = dict
        .iter()
        .enumerate()
        .map(|(i, t)| (t.as_slice(), i as u8))
        .collect();

    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;

    while i < input.len() {
        let b = input[i];
        if b == ESCAPE {
            // Escape the escape byte.
            out.push(ESCAPE);
            out.push(ESCAPE_SELF);
            i += 1;
        } else if is_word_char(b) {
            // Find the extent of this word token.
            let start = i;
            while i < input.len() && is_word_char(input[i]) {
                i += 1;
            }
            let word = &input[start..i];
            if let Some(&idx) = token_map.get(word) {
                out.push(ESCAPE);
                out.push(idx);
            } else {
                out.extend_from_slice(word);
            }
        } else {
            out.push(b);
            i += 1;
        }
    }

    out
}

fn decode_body(body: &[u8], dict: &[Vec<u8>]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(body.len());
    let mut i = 0;

    while i < body.len() {
        let b = body[i];
        if b == ESCAPE {
            i += 1;
            if i >= body.len() {
                return Err(tknd_err("truncated escape sequence in body".into()));
            }
            let n = body[i];
            if n == ESCAPE_SELF {
                out.push(ESCAPE);
            } else if (n as usize) < dict.len() {
                out.extend_from_slice(&dict[n as usize]);
            } else {
                return Err(tknd_err(format!(
                    "token index {n} out of range (dict has {} entries)",
                    dict.len()
                )));
            }
            i += 1;
        } else {
            out.push(b);
            i += 1;
        }
    }

    Ok(out)
}

fn tknd_err(message: String) -> ArcError {
    ArcError::Codec {
        codec: "tokendict",
        message,
    }
}
