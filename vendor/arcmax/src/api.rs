//! Small public compression API for embedding arcmax in other programs.
//!
//! These helpers produce and consume self-describing ArcMax frame streams. The
//! chosen compression method is stored in the frame, so callers do not need to
//! remember external codec parameters when decompressing later.

use std::io::{Cursor, Read, Write};

use crate::codec::zstd::ZstdOptions;
use crate::error::{ArcError, Result};
use crate::method::{CodecPipeline, Method};

/// Options for creating a self-describing compressed frame.
#[derive(Debug, Clone)]
pub struct CompressionOptions {
    /// Compression method to apply. The method string is embedded in the frame.
    pub method: Method,
    /// Password for encrypted methods.
    ///
    /// Encrypted self-describing frames need the generated salt and IV embedded
    /// in the method header. That metadata propagation is not complete yet, so
    /// encrypted methods currently return a clear error from this high-level API.
    pub password: Option<Vec<u8>>,
    /// Maximum uncompressed block size. Defaults to 8 MiB.
    pub block_size: usize,
}

impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            method: Method::Zstd(ZstdOptions { level: 3 }),
            password: None,
            block_size: 8 * 1024 * 1024,
        }
    }
}

impl CompressionOptions {
    pub fn with_method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    pub fn with_password(mut self, password: impl Into<Vec<u8>>) -> Self {
        self.password = Some(password.into());
        self
    }

    pub fn with_block_size(mut self, block_size: usize) -> Self {
        self.block_size = block_size;
        self
    }
}

/// Options for decompressing a self-describing compressed frame.
#[derive(Debug, Clone, Default)]
pub struct DecompressionOptions {
    /// Password for encrypted methods.
    pub password: Option<Vec<u8>>,
}

impl DecompressionOptions {
    pub fn with_password(mut self, password: impl Into<Vec<u8>>) -> Self {
        self.password = Some(password.into());
        self
    }
}

/// Compress bytes with the default public method (`zstd:3`).
pub fn compress(input: &[u8]) -> Result<Vec<u8>> {
    compress_with(input, CompressionOptions::default())
}

/// Compress bytes with explicit options.
pub fn compress_with(input: &[u8], options: CompressionOptions) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    compress_stream(Cursor::new(input), &mut output, options)?;
    Ok(output)
}

/// Decompress bytes from a self-describing frame.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>> {
    decompress_with(input, DecompressionOptions::default())
}

/// Decompress bytes from a self-describing frame with explicit options.
pub fn decompress_with(input: &[u8], options: DecompressionOptions) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    decompress_stream(Cursor::new(input), &mut output, options)?;
    Ok(output)
}

/// Compress a stream to a self-describing frame.
pub fn compress_stream<R: Read, W: Write>(
    input: R,
    output: W,
    options: CompressionOptions,
) -> Result<()> {
    if method_contains_encryption(&options.method) {
        return Err(ArcError::InvalidMethod(
            "self-describing encrypted frames are not available until encryption salt/IV metadata is serialized"
                .to_string(),
        ));
    }

    let mut pipeline = match options.password {
        Some(password) => CodecPipeline::with_password(options.method, password),
        None => CodecPipeline::new(options.method),
    }
    .with_block_size(options.block_size);
    pipeline.compress(input, output)
}

/// Decompress a self-describing frame stream.
pub fn decompress_stream<R: Read, W: Write>(
    input: R,
    output: W,
    options: DecompressionOptions,
) -> Result<()> {
    let mut pipeline = match options.password {
        Some(password) => CodecPipeline::with_password(Method::Store, password),
        None => CodecPipeline::new(Method::Store),
    };
    pipeline.decompress(input, output)
}

fn method_contains_encryption(method: &Method) -> bool {
    match method {
        Method::Encryption(_) => true,
        Method::Pipeline(stages) => stages.iter().any(method_contains_encryption),
        Method::Blocked(options) => method_contains_encryption(&options.inner),
        _ => false,
    }
}
