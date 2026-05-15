use std::io::{Read, Write};
use anyhow::{Result, bail};
use crate::core::varint::{decode_varint, encode_varint};

pub fn read_varint<R: Read>(reader: &mut R) -> Result<(u64, usize)> {
    let mut buf = [0u8; 9];
    
    // Read first byte
    reader.read_exact(&mut buf[0..1])?;
    
    let first = buf[0];
    let extra_bytes = if first == 0xFF {
        8
    } else {
        let zeros = first.trailing_zeros();
        zeros as usize
    };
    
    if extra_bytes > 0 {
        reader.read_exact(&mut buf[1..1 + extra_bytes])?;
    }
    
    decode_varint(&buf[0..1+extra_bytes])
}

pub fn write_varint<W: Write>(writer: &mut W, value: u64) -> Result<usize> {
    let encoded = encode_varint(value);
    writer.write_all(&encoded)?;
    Ok(encoded.len())
}

pub fn read_stringz<R: Read>(reader: &mut R) -> Result<String> {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 1];
    loop {
        reader.read_exact(&mut buf)?;
        if buf[0] == 0 {
            break;
        }
        bytes.push(buf[0]);
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

pub fn write_stringz<W: Write>(writer: &mut W, s: &str) -> Result<usize> {
    let bytes = s.as_bytes();
    writer.write_all(bytes)?;
    writer.write_all(&[0])?;
    Ok(bytes.len() + 1)
}

pub fn read_varint_list<R: Read>(reader: &mut R, count: usize) -> Result<Vec<u64>> {
    let mut list = Vec::with_capacity(count);
    for _ in 0..count {
        let (val, _) = read_varint(reader)?;
        list.push(val);
    }
    Ok(list)
}

pub fn write_varint_list<W: Write>(writer: &mut W, list: &[u64]) -> Result<()> {
    for &val in list {
        write_varint(writer, val)?;
    }
    Ok(())
}

pub fn read_string_list<R: Read>(reader: &mut R, count: usize) -> Result<Vec<String>> {
    let mut list = Vec::with_capacity(count);
    for _ in 0..count {
        list.push(read_stringz(reader)?);
    }
    Ok(list)
}

pub fn write_string_list<W: Write>(writer: &mut W, list: &[String]) -> Result<()> {
    for s in list {
        write_stringz(writer, s)?;
    }
    Ok(())
}

// For fixed size types (u32, u8, etc)
pub trait FixedSize: Sized {
    fn read<R: Read>(reader: &mut R) -> Result<Self>;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()>;
}

impl FixedSize for u32 {
    fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl FixedSize for u8 {
    fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&[*self])?;
        Ok(())
    }
}

pub fn read_fixed_list<R: Read, T: FixedSize>(reader: &mut R, count: usize) -> Result<Vec<T>> {
    let mut list = Vec::with_capacity(count);
    for _ in 0..count {
        list.push(T::read(reader)?);
    }
    Ok(list)
}

pub fn write_fixed_list<W: Write, T: FixedSize>(writer: &mut W, list: &[T]) -> Result<()> {
    for val in list {
        val.write(writer)?;
    }
    Ok(())
}

// Special case for bool which is 1 byte
impl FixedSize for bool {
    fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        Ok(buf[0] != 0)
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&[if *self { 1 } else { 0 }])?;
        Ok(())
    }
}

// Parsing utilities moved from free_arc_utils.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecSpec {
    pub name: String,
    pub params: Vec<String>,
}

pub fn find_param_value(params: &[String], prefix: char) -> Option<String> {
    params
        .iter()
        .find_map(|param| param.strip_prefix(prefix).map(|value| value.to_string()))
}

pub fn format_codec_chain(chain: &[CodecSpec]) -> String {
    chain
        .iter()
        .map(format_codec_spec)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("+")
}

pub fn format_codec_spec(spec: &CodecSpec) -> String {
    if spec.name.is_empty() {
        return String::new();
    }

    if spec.params.is_empty() {
        spec.name.clone()
    } else {
        format!("{}:{}", spec.name, spec.params.join(":"))
    }
}

pub fn parse_codec_chain(method: &str) -> Vec<CodecSpec> {
    if method.is_empty() {
        return Vec::new();
    }

    method
        .split('+')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut pieces = part.split(':');
            let name = pieces.next().unwrap_or("").to_lowercase();
            let params = pieces.map(|param| param.to_string()).collect();
            CodecSpec { name, params }
        })
        .collect()
}

pub fn parse_size(input: &str) -> Result<usize> {
    let s = input.to_lowercase();
    if let Some(num) = s.strip_suffix("gb") {
        Ok(num.parse::<usize>()? * 1024 * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix("mb") {
        Ok(num.parse::<usize>()? * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix("kb") {
        Ok(num.parse::<usize>()? * 1024)
    } else if let Some(num) = s.strip_suffix('b') {
        Ok(num.parse::<usize>()?)
    } else {
        Ok(s.parse::<usize>()?)
    }
}

pub fn split_compressor_encryption(method: &str) -> (String, String) {
    if method.contains('+') {
        let parts: Vec<&str> = method.split('+').collect();
        let compression = parts[0].to_string();
        let encryption = parts[1..].join("+");
        (compression, encryption)
    } else {
        (method.to_string(), String::new())
    }
}
