//! FreeARC Variable-Length Integer Encoding/Decoding
//!
//! FreeARC uses a custom variable-length integer format where the number of
//! trailing zero bits in the first byte determines the format:
//! - Low bit = 0: 1 byte, value = x >> 1 (7 bits, 0-127)
//! - Low 2 bits = 01: 2 bytes, value = x >> 2 (14 bits)
//! - Low 3 bits = 011: 3 bytes, value = x >> 3 (21 bits)
//! - Low 4 bits = 0111: 4 bytes, value = x >> 4 (28 bits)
//! - Low 5 bits = 01111: 5 bytes, value = x >> 5 (35 bits)
//! - Low 6 bits = 011111: 6 bytes, value = x >> 6 (42 bits)
//! - Low 7 bits = 0111111: 7 bytes, value = x >> 7 (49 bits)
//! - Low 8 bits = 01111111: 8 bytes, value = x >> 8 (56 bits)
//! - First byte = 0xFF: 9 bytes, following 8 bytes are the value (64 bits)

use anyhow::Result;

/// Encode a value as a FreeARC variable-length integer
pub fn encode_varint(value: u64) -> Vec<u8> {
    if value < (1 << 7) {
        // 1 byte: value << 1 with low bit = 0
        vec![(value << 1) as u8]
    } else if value < (1 << 14) {
        // 2 bytes: value << 2 with low 2 bits = 01
        let v = (value << 2) | 0b01;
        v.to_le_bytes()[..2].to_vec()
    } else if value < (1 << 21) {
        // 3 bytes: value << 3 with low 3 bits = 011
        let v = (value << 3) | 0b011;
        v.to_le_bytes()[..3].to_vec()
    } else if value < (1 << 28) {
        // 4 bytes: value << 4 with low 4 bits = 0111
        let v = (value << 4) | 0b0111;
        v.to_le_bytes()[..4].to_vec()
    } else if value < (1 << 35) {
        // 5 bytes: value << 5 with low 5 bits = 01111
        let v = (value << 5) | 0b01111;
        v.to_le_bytes()[..5].to_vec()
    } else if value < (1 << 42) {
        // 6 bytes: value << 6 with low 6 bits = 011111
        let v = (value << 6) | 0b011111;
        v.to_le_bytes()[..6].to_vec()
    } else if value < (1 << 49) {
        // 7 bytes: value << 7 with low 7 bits = 0111111
        let v = (value << 7) | 0b0111111;
        v.to_le_bytes()[..7].to_vec()
    } else if value < (1 << 56) {
        // 8 bytes: value << 8 with low 8 bits = 01111111
        let v = (value << 8) | 0b01111111;
        v.to_le_bytes().to_vec()
    } else {
        // 9 bytes: 0xFF marker followed by 8 bytes
        let mut result = vec![0xFF];
        result.extend_from_slice(&value.to_le_bytes());
        result
    }
}

/// Decode a FreeARC variable-length integer from a byte slice
/// Returns (value, bytes_consumed)
pub fn decode_varint(data: &[u8]) -> Result<(u64, usize)> {
    if data.is_empty() {
        anyhow::bail!("Cannot decode varint from empty slice");
    }

    // Read up to 8 bytes (pad with zeros if needed)
    let mut buf = [0u8; 8];
    let available = std::cmp::min(8, data.len());
    buf[..available].copy_from_slice(&data[..available]);

    let x32 = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let x64 = u64::from_le_bytes(buf);

    let (value, consumed) = if (x32 & 1) == 0 {
        // 1 byte format
        ((buf[0] >> 1) as u64, 1)
    } else if (x32 & 3) == 1 {
        // 2 byte format
        let v = u16::from_le_bytes([buf[0], buf[1]]);
        ((v >> 2) as u64, 2)
    } else if (x32 & 7) == 3 {
        // 3 byte format
        let v = u32::from_le_bytes([buf[0], buf[1], buf[2], 0]);
        ((v >> 3) as u64, 3)
    } else if (x32 & 15) == 7 {
        // 4 byte format
        ((x32 >> 4) as u64, 4)
    } else if (x32 & 31) == 15 {
        // 5 byte format
        ((x64 >> 5) & ((1u64 << 40) - 1), 5)
    } else if (x32 & 63) == 31 {
        // 6 byte format
        ((x64 >> 6) & ((1u64 << 48) - 1), 6)
    } else if (x32 & 127) == 63 {
        // 7 byte format
        ((x64 >> 7) & ((1u64 << 56) - 1), 7)
    } else if (x32 & 255) == 127 {
        // 8 byte format
        (x64 >> 8, 8)
    } else {
        // 9 byte format: first byte is 0xFF, followed by 8 bytes of value
        if data.len() < 9 {
            anyhow::bail!("Not enough data for 9-byte varint");
        }
        let v = u64::from_le_bytes([
            data[1], data[2], data[3], data[4],
            data[5], data[6], data[7], data[8],
        ]);
        (v, 9)
    };

    Ok((value, consumed))
}

/// Write a varint to a writer
pub fn write_varint<W: std::io::Write>(writer: &mut W, value: u64) -> std::io::Result<usize> {
    let encoded = encode_varint(value);
    writer.write_all(&encoded)?;
    Ok(encoded.len())
}

/// Write a null-terminated string
pub fn write_cstring<W: std::io::Write>(writer: &mut W, s: &str) -> std::io::Result<usize> {
    writer.write_all(s.as_bytes())?;
    writer.write_all(&[0])?;
    Ok(s.len() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_roundtrip_small() {
        for value in 0..128u64 {
            let encoded = encode_varint(value);
            assert_eq!(encoded.len(), 1, "Value {} should encode to 1 byte", value);
            let (decoded, consumed) = decode_varint(&encoded).unwrap();
            assert_eq!(decoded, value, "Roundtrip failed for value {}", value);
            assert_eq!(consumed, 1);
        }
    }

    #[test]
    fn test_varint_roundtrip_medium() {
        let test_values = [128, 255, 1000, 16383, 16384, 100000, 1_000_000];
        for &value in &test_values {
            let encoded = encode_varint(value);
            let (decoded, consumed) = decode_varint(&encoded).unwrap();
            assert_eq!(decoded, value, "Roundtrip failed for value {}", value);
            assert_eq!(consumed, encoded.len());
        }
    }

    #[test]
    fn test_varint_roundtrip_large() {
        let test_values = [
            1u64 << 30,
            1u64 << 40,
            1u64 << 50,
            1u64 << 60,
            u64::MAX,
        ];
        for &value in &test_values {
            let encoded = encode_varint(value);
            let (decoded, consumed) = decode_varint(&encoded).unwrap();
            assert_eq!(decoded, value, "Roundtrip failed for value {}", value);
            assert_eq!(consumed, encoded.len());
        }
    }

    #[test]
    fn test_varint_encoding_sizes() {
        // 1 byte: 0-127
        assert_eq!(encode_varint(0).len(), 1);
        assert_eq!(encode_varint(127).len(), 1);

        // 2 bytes: 128-16383
        assert_eq!(encode_varint(128).len(), 2);
        assert_eq!(encode_varint(16383).len(), 2);

        // 3 bytes: 16384-2097151
        assert_eq!(encode_varint(16384).len(), 3);
        assert_eq!(encode_varint(2097151).len(), 3);

        // 4 bytes: 2097152-268435455
        assert_eq!(encode_varint(2097152).len(), 4);
        assert_eq!(encode_varint(268435455).len(), 4);

        // 9 bytes for very large values
        assert_eq!(encode_varint(u64::MAX).len(), 9);
    }
}
