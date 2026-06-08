//! SQL row-template pre-compressor.
//!
//! `pg_dump` / `mysqldump` output is dominated by millions of lines like:
//!
//! ```text
//! INSERT INTO users (id, name, email) VALUES (1, 'alice', 'alice@example.com');
//! INSERT INTO users (id, name, email) VALUES (2, 'bob', 'bob@example.com');
//! ```
//!
//! The `INSERT INTO … VALUES` prefix repeats verbatim for every row. This
//! filter extracts those prefixes into a per-table template dictionary and
//! replaces each prefix with a 2-byte `ESCAPE TABLE_ID` code, leaving only
//! the VALUES tuple for the downstream compressor.
//!
//! ## Detection
//!
//! A template is any line prefix up to and including `VALUES (` (case-insensitive)
//! that appears at least twice. Templates are scoped per dump table: two different
//! `INSERT INTO` targets never share a template ID.
//!
//! ## Wire format (output of `compress`)
//!
//! ```text
//! [magic: b"SQLT"][version: u8 = 1]
//! [dict_len: u8]  — number of template entries (0..=254)
//! For each template (index = position):
//!   [tmpl_len: u16 LE][tmpl_bytes: tmpl_len bytes]
//! [body: input bytes with template prefixes replaced]
//! ```
//!
//! In the body:
//! * `0xFD N` where `N < dict_len` → template[N] (expands to the prefix text)
//! * `0xFD 0xFF`                   → literal `0xFD`
//! * any other byte                → unchanged

use std::collections::HashMap;
use std::io::{Read, Write};

use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

const MAGIC: &[u8; 4] = b"SQLT";
const VERSION: u8 = 1;
const ESCAPE: u8 = 0xFD;
const ESCAPE_SELF: u8 = 0xFF;
const MAX_TEMPLATES: usize = 254;

pub struct SqlTemplateCodec;

impl Codec for SqlTemplateCodec {
    fn name(&self) -> &'static str {
        "sqltemplate"
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage::default()
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        let mut buf = Vec::new();
        let bytes_in = input.read_to_end(&mut buf)? as u64;

        let dict = build_template_dict(&buf);
        let body = encode_body(&buf, &dict);

        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(dict.len() as u8);
        for tmpl in &dict {
            let len = tmpl.len() as u16;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(tmpl);
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

        if buf.len() < 6 {
            return Err(sqlt_err("stream too short".into()));
        }
        if &buf[..4] != MAGIC {
            return Err(sqlt_err(format!(
                "invalid magic: expected SQLT, got {:02x?}",
                &buf[..4]
            )));
        }
        if buf[4] != VERSION {
            return Err(sqlt_err(format!("unsupported version {}", buf[4])));
        }

        let dict_len = buf[5] as usize;
        let mut pos = 6usize;

        let mut dict: Vec<Vec<u8>> = Vec::with_capacity(dict_len);
        for _ in 0..dict_len {
            if pos + 2 > buf.len() {
                return Err(sqlt_err("truncated template header".into()));
            }
            let tmpl_len = u16::from_le_bytes([buf[pos], buf[pos + 1]]) as usize;
            pos += 2;
            if pos + tmpl_len > buf.len() {
                return Err(sqlt_err("truncated template entry".into()));
            }
            dict.push(buf[pos..pos + tmpl_len].to_vec());
            pos += tmpl_len;
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

/// Find the `VALUES (` boundary in a line (case-insensitive), returning the
/// byte offset just after the `(`. Returns `None` if not found.
fn find_values_end(line: &[u8]) -> Option<usize> {
    // Scan for "values" (case-insensitive) followed by optional whitespace and `(`
    let lower: Vec<u8> = line.iter().map(|b| b.to_ascii_lowercase()).collect();
    let needle = b"values";
    let mut i = 0;
    while i + needle.len() <= lower.len() {
        if &lower[i..i + needle.len()] == needle {
            // Skip whitespace after "values"
            let mut j = i + needle.len();
            while j < lower.len() && lower[j] == b' ' {
                j += 1;
            }
            if j < lower.len() && lower[j] == b'(' {
                return Some(j + 1); // just after the '('
            }
        }
        i += 1;
    }
    None
}

/// Extract per-line templates (the prefix up to and including `VALUES (`),
/// count frequencies, and return the top-254 by bytes-saved.
fn build_template_dict(input: &[u8]) -> Vec<Vec<u8>> {
    let mut freq: HashMap<&[u8], usize> = HashMap::new();

    for line in input.split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        // Only process INSERT statements.
        let trimmed = line.strip_prefix(b"\r").unwrap_or(line);
        let lower_prefix: Vec<u8> = trimmed
            .iter()
            .take(7)
            .map(|b| b.to_ascii_lowercase())
            .collect();
        if !lower_prefix.starts_with(b"insert") {
            continue;
        }
        if let Some(end) = find_values_end(trimmed) {
            let prefix = &trimmed[..end];
            if prefix.len() > 4 {
                *freq.entry(prefix).or_default() += 1;
            }
        }
    }

    let mut scored: Vec<(&[u8], usize)> = freq
        .into_iter()
        .filter(|&(_, f)| f >= 2)
        .map(|(tmpl, f)| {
            // Each substitution saves (tmpl_len - 2) bytes.
            let saving = (f - 1) * tmpl.len().saturating_sub(2);
            (tmpl, saving)
        })
        .filter(|&(_, s)| s > 0)
        .collect();

    scored.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    scored
        .into_iter()
        .take(MAX_TEMPLATES)
        .map(|(t, _)| t.to_vec())
        .collect()
}

fn encode_body(input: &[u8], dict: &[Vec<u8>]) -> Vec<u8> {
    if dict.is_empty() {
        // Still need to escape any literal 0xFD bytes.
        let mut out = Vec::with_capacity(input.len());
        for &b in input {
            if b == ESCAPE {
                out.push(ESCAPE);
                out.push(ESCAPE_SELF);
            } else {
                out.push(b);
            }
        }
        return out;
    }

    // Build a lookup: prefix bytes → template index.
    // Sort templates longest-first so we match greedily.
    let mut indexed: Vec<(u8, &[u8])> = dict
        .iter()
        .enumerate()
        .map(|(i, t)| (i as u8, t.as_slice()))
        .collect();
    indexed.sort_unstable_by(|a, b| b.1.len().cmp(&a.1.len()));

    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;

    while i < input.len() {
        let b = input[i];
        if b == ESCAPE {
            out.push(ESCAPE);
            out.push(ESCAPE_SELF);
            i += 1;
            continue;
        }

        // Try to match a template at position i.
        let mut matched = false;
        for &(idx, tmpl) in &indexed {
            if input[i..].starts_with(tmpl) {
                out.push(ESCAPE);
                out.push(idx);
                i += tmpl.len();
                matched = true;
                break;
            }
        }
        if !matched {
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
                return Err(sqlt_err("truncated escape sequence in body".into()));
            }
            let n = body[i];
            if n == ESCAPE_SELF {
                out.push(ESCAPE);
            } else if (n as usize) < dict.len() {
                out.extend_from_slice(&dict[n as usize]);
            } else {
                return Err(sqlt_err(format!(
                    "template index {n} out of range (dict has {} entries)",
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

fn sqlt_err(message: String) -> ArcError {
    ArcError::Codec {
        codec: "sqltemplate",
        message,
    }
}
