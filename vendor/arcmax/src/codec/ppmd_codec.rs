use std::io::{Read, Write};

use ppmd_rust::{Ppmd7Decoder, Ppmd7Encoder};

use crate::codec::framing;
use crate::codec::ppmd::h::{PpmdHDecoder, PpmdHEncoder};
use crate::codec::ppmd::{PpmdOptions, PpmdVariant};
use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

/// PPMd codec supporting both PPMdH (FreeArc-compatible) and PPMd7 (7-zip) variants.
///
/// Uses size-prepend framing: 8-byte LE uncompressed-size header precedes the
/// entropy-coded stream so the decoder knows how much to read.
///
/// Default variant is `H` (carryless range coder), which is what FreeArc archives use.
pub struct PpmdCodec {
    options: PpmdOptions,
}

impl PpmdCodec {
    pub fn new(options: PpmdOptions) -> Self {
        Self { options }
    }
}

struct CountingWriter<'a, W: Write + ?Sized> {
    inner: &'a mut W,
    written: u64,
}

impl<'a, W: Write + ?Sized> CountingWriter<'a, W> {
    fn new(inner: &'a mut W) -> Self {
        Self { inner, written: 0 }
    }
}

impl<W: Write + ?Sized> Write for CountingWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

struct CountingReader<'a, R: Read + ?Sized> {
    inner: &'a mut R,
    read: u64,
}

impl<'a, R: Read + ?Sized> CountingReader<'a, R> {
    fn new(inner: &'a mut R) -> Self {
        Self { inner, read: 0 }
    }
}

impl<R: Read + ?Sized> Read for CountingReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        Ok(n)
    }
}

impl Codec for PpmdCodec {
    fn name(&self) -> &'static str {
        "ppmd"
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        self.options.validate()?;

        match self.options.variant {
            PpmdVariant::H => {
                let mut compressed = Vec::new();
                let (bytes_in, bytes_written) = {
                    let mut counted = CountingWriter::new(&mut compressed);
                    let mut encoder = PpmdHEncoder::new(&mut counted, self.options)?;
                    let bytes_in =
                        std::io::copy(input, &mut encoder).map_err(|e| ArcError::Codec {
                            codec: "ppmd",
                            message: format!("encode failed: {e}"),
                        })?;
                    encoder.finish(false).map_err(|e| ArcError::Codec {
                        codec: "ppmd",
                        message: format!("finish failed: {e}"),
                    })?;
                    (bytes_in, counted.written)
                };
                framing::write_size_header(bytes_in as usize, output)?;
                output.write_all(&compressed)?;
                Ok(CodecReport {
                    bytes_in,
                    bytes_out: framing::SIZE_HEADER_LEN as u64 + bytes_written,
                })
            }
            PpmdVariant::Seven => {
                let mut source = Vec::new();
                input.read_to_end(&mut source)?;
                let uncompressed_len = source.len();
                let mut buf = Vec::new();
                let mut encoder = Ppmd7Encoder::new(
                    &mut buf,
                    self.options.order as u32,
                    self.options.memory_size as u32,
                )
                .map_err(|e| ArcError::Codec {
                    codec: "ppmd",
                    message: format!("encoder init failed: {e:?}"),
                })?;
                encoder.write_all(&source).map_err(|e| ArcError::Codec {
                    codec: "ppmd",
                    message: format!("encode failed: {e}"),
                })?;
                encoder.finish(false).map_err(|e| ArcError::Codec {
                    codec: "ppmd",
                    message: format!("finish failed: {e:?}"),
                })?;
                framing::write_size_header(uncompressed_len, output)?;
                output.write_all(&buf)?;

                Ok(CodecReport {
                    bytes_in: uncompressed_len as u64,
                    bytes_out: (framing::SIZE_HEADER_LEN + buf.len()) as u64,
                })
            }
        }
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        self.options.validate()?;
        let expected_len = framing::read_size_header(input)?;

        match self.options.variant {
            PpmdVariant::H => {
                let mut counted = CountingReader::new(input);
                {
                    let mut decoder = PpmdHDecoder::new(&mut counted, self.options)?;
                    let mut remaining = expected_len;
                    let mut buf = [0u8; 64 * 1024];
                    while remaining != 0 {
                        let n = remaining.min(buf.len());
                        decoder
                            .read_exact(&mut buf[..n])
                            .map_err(|e| ArcError::Codec {
                                codec: "ppmd",
                                message: format!("decode failed: {e}"),
                            })?;
                        output.write_all(&buf[..n])?;
                        remaining -= n;
                    }
                }

                Ok(CodecReport {
                    bytes_in: framing::SIZE_HEADER_LEN as u64 + counted.read,
                    bytes_out: expected_len as u64,
                })
            }
            PpmdVariant::Seven => {
                let mut compressed = Vec::new();
                input.read_to_end(&mut compressed)?;
                let bytes_in = (framing::SIZE_HEADER_LEN + compressed.len()) as u64;
                let mut decoder = Ppmd7Decoder::new(
                    std::io::Cursor::new(&compressed),
                    self.options.order as u32,
                    self.options.memory_size as u32,
                )
                .map_err(|e| ArcError::Codec {
                    codec: "ppmd",
                    message: format!("decoder init failed: {e:?}"),
                })?;
                let mut buf = vec![0u8; expected_len];
                decoder.read_exact(&mut buf).map_err(|e| ArcError::Codec {
                    codec: "ppmd",
                    message: format!("decode failed: {e}"),
                })?;
                output.write_all(&buf)?;

                Ok(CodecReport {
                    bytes_in,
                    bytes_out: buf.len() as u64,
                })
            }
        }
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        MemoryUsage {
            working_bytes: self.options.memory_size as u64,
            ..MemoryUsage::default()
        }
    }
}
