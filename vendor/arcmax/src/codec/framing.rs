/// Size-prepend framing used by FFI-backed codecs.
///
/// All FFI decompressors need the original (uncompressed) byte count before
/// they can allocate an output buffer. We prepend it as an 8-byte LE u64 so
/// the `Codec` trait's generic `Read → Write` interface can work without a
/// separate out-of-band size channel.
///
/// Format: `[u64 LE uncompressed_len][compressed bytes…]`
use std::io::{self, Read, Write};

pub const SIZE_HEADER_LEN: usize = 8;

/// Read the leading u64 LE uncompressed-size field.
pub fn read_size_header<R: Read + ?Sized>(input: &mut R) -> io::Result<usize> {
    let mut buf = [0u8; SIZE_HEADER_LEN];
    input.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf) as usize)
}

/// Write the u64 LE uncompressed-size field.
pub fn write_size_header<W: Write + ?Sized>(
    uncompressed_len: usize,
    output: &mut W,
) -> io::Result<()> {
    output.write_all(&(uncompressed_len as u64).to_le_bytes())
}
