mod internal;
mod model;
pub(crate) mod tagged_offset;

use std::io::{Read, Write};

use super::{PpmdOptions, PpmdVariant};
use crate::error::{ArcError, Result as ArcResult};
use model::{PPMd7, RangeDecoder, RangeEncoder};

pub(crate) const SYM_END: i32 = -1;
pub(crate) const SYM_ERROR: i32 = -2;

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    InvalidParameter,
    MemoryAllocation,
    IoError(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidParameter => write!(f, "invalid PPMd-H parameter"),
            Error::MemoryAllocation => write!(f, "PPMd-H memory allocation failed"),
            Error::IoError(e) => write!(f, "PPMd-H I/O error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IoError(e) => Some(e),
            _ => None,
        }
    }
}

fn validate_h_options(options: PpmdOptions) -> ArcResult<()> {
    options.validate()?;
    if options.variant != PpmdVariant::H {
        return Err(ArcError::InvalidMethod(
            "PPMd-H codec requires PpmdVariant::H options".to_string(),
        ));
    }
    Ok(())
}

fn model_error(context: &'static str, err: Error) -> ArcError {
    ArcError::Codec {
        codec: "ppmd",
        message: format!("{context}: {err}"),
    }
}

/// PPMd variant H encoder using the Subbotin carryless range coder.
///
/// Compatible with FreeArc `.arc` archives and the original PPMdH C implementation.
/// Order and memory size correspond to the FreeArc `-m` and `-mem` parameters.
pub struct PpmdHEncoder<W: Write> {
    ppmd: PPMd7<RangeEncoder<W>>,
}

unsafe impl<W: Write + Send> Send for PpmdHEncoder<W> {}

impl<W: Write> PpmdHEncoder<W> {
    pub fn new(writer: W, options: PpmdOptions) -> ArcResult<Self> {
        validate_h_options(options)?;
        let ppmd = PPMd7::new_encoder(writer, options.order as u32, options.memory_size as u32)
            .map_err(|err| model_error("encoder init failed", err))?;
        Ok(Self { ppmd })
    }

    pub fn get_ref(&self) -> &W {
        self.ppmd.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut W {
        self.ppmd.get_mut()
    }

    pub fn into_inner(self) -> W {
        self.ppmd.into_inner()
    }

    /// Finish encoding. Pass `with_end_marker = true` to allow the decoder to
    /// stop without needing an external length (FreeArc does NOT use an end marker;
    /// it passes `expected_len` to the decoder instead).
    pub fn finish(mut self, with_end_marker: bool) -> std::io::Result<W> {
        if with_end_marker {
            self.ppmd.encode_symbol(SYM_END)?;
        }
        self.flush()?;
        Ok(self.into_inner())
    }
}

impl<W: Write> Write for PpmdHEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for &byte in buf {
            self.ppmd.encode_symbol(byte as i32)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.ppmd.flush_range_encoder()
    }
}

/// PPMd variant H decoder using the Subbotin carryless range coder.
pub struct PpmdHDecoder<R: Read> {
    ppmd: PPMd7<RangeDecoder<R>>,
    finished: bool,
}

unsafe impl<R: Read + Send> Send for PpmdHDecoder<R> {}

impl<R: Read> PpmdHDecoder<R> {
    pub fn new(reader: R, options: PpmdOptions) -> ArcResult<Self> {
        validate_h_options(options)?;
        let ppmd = PPMd7::new_decoder(reader, options.order as u32, options.memory_size as u32)
            .map_err(|err| model_error("decoder init failed", err))?;
        Ok(Self {
            ppmd,
            finished: false,
        })
    }

    pub fn get_ref(&self) -> &R {
        self.ppmd.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut R {
        self.ppmd.get_mut()
    }

    pub fn into_inner(self) -> R {
        self.ppmd.into_inner()
    }
}

impl<R: Read> Read for PpmdHDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.finished || buf.is_empty() {
            return Ok(0);
        }

        let mut decoded = 0;
        let mut sym = 0;

        for byte in buf.iter_mut() {
            match self.ppmd.decode_symbol() {
                Ok(s) => sym = s,
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::UnexpectedEof {
                        self.finished = true;
                        return Ok(decoded);
                    }
                    return Err(err);
                }
            }

            if sym < 0 {
                break;
            }
            *byte = sym as u8;
            decoded += 1;
        }

        let code = self.ppmd.range_decoder_code();

        if sym >= 0 {
            return Ok(decoded);
        }

        self.finished = true;

        if sym != SYM_END || code != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "PPMd-H decoding error",
            ));
        }

        Ok(decoded)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;

    fn roundtrip(input: &[u8], order: u8, memory_size: usize) {
        let options = PpmdOptions {
            order,
            memory_size,
            variant: PpmdVariant::H,
        };
        let mut compressed = Vec::new();
        {
            let mut enc = PpmdHEncoder::new(&mut compressed, options).unwrap();
            enc.write_all(input).unwrap();
            enc.finish(false).unwrap();
        }

        let mut dec = PpmdHDecoder::new(compressed.as_slice(), options).unwrap();
        let mut decoded = vec![0u8; input.len()];
        dec.read_exact(&mut decoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn roundtrip_repetitive_text() {
        let input = b"hello world hello world hello world ".repeat(256);
        roundtrip(&input, 8, 1 << 20);
    }

    #[test]
    fn roundtrip_binary_cycle() {
        let input: Vec<u8> = (0u8..=255).cycle().take(8192).collect();
        roundtrip(&input, 6, 1 << 19);
    }

    #[test]
    fn roundtrip_small_order2() {
        let input = b"PPMdH carryless range coder roundtrip ".repeat(64);
        roundtrip(&input, 2, PpmdOptions::MIN_MEMORY_SIZE);
    }

    #[test]
    fn roundtrip_high_order() {
        let input = b"abcdefghijklmnopqrstuvwxyz".repeat(512);
        roundtrip(&input, 16, 1 << 22);
    }

    #[test]
    fn compressed_smaller_than_input() {
        let input = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".repeat(256);
        let options = PpmdOptions {
            order: 8,
            memory_size: 1 << 20,
            variant: PpmdVariant::H,
        };
        let mut compressed = Vec::new();
        {
            let mut enc = PpmdHEncoder::new(&mut compressed, options).unwrap();
            enc.write_all(&input).unwrap();
            enc.finish(false).unwrap();
        }
        assert!(compressed.len() < input.len(), "expected compression");
    }
}
