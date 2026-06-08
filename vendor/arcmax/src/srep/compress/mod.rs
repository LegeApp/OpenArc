pub mod block_sink;
pub mod cdc;
pub mod fixed;
pub mod inmem;
pub mod literal;

use std::io::{Read, Seek, Write};

use crate::srep::config::{Method, SrepConfig};
use crate::srep::error::SrepError;

/// Statistics returned after a successful compression run.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompressionReport {
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub blocks_written: u64,
    pub matches_written: u64,
}

/// Compress `input` into `output` as an SREP archive using `config`.
pub fn compress<R, W>(
    input: R,
    output: W,
    config: SrepConfig,
) -> Result<CompressionReport, SrepError>
where
    R: Read + Seek,
    W: Write + Seek,
{
    match config.method {
        Method::M0 => inmem::encode(input, output, &config),
        Method::M1 | Method::M2 => cdc::encode(input, output, &config),
        Method::M3 | Method::M4 | Method::M5 => fixed::encode(input, output, &config),
    }
}
