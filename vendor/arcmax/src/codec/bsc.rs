//! libbsc FFI bindings and a Codec wrapper.
//!
//! libbsc is block-sorting (BWT + QLFC entropy coder). It excels on text and
//! struct-rich binary; it tends to underperform on already-compressed media.
//!
//! Single-threaded build: OpenMP and CUDA are disabled at build time.

use std::io::{Read, Write};

use crate::codec::traits::{Codec, CodecReport, Direction, MemoryUsage};
use crate::error::{ArcError, Result};

pub const LIBBSC_HEADER_SIZE: usize = 28;
pub const LIBBSC_NO_ERROR: i32 = 0;

pub const LIBBSC_BLOCKSORTER_BWT: i32 = 1;
pub const LIBBSC_BLOCKSORTER_ST3: i32 = 3;
pub const LIBBSC_BLOCKSORTER_ST4: i32 = 4;
pub const LIBBSC_BLOCKSORTER_ST5: i32 = 5;
pub const LIBBSC_BLOCKSORTER_ST6: i32 = 6;

pub const LIBBSC_CODER_QLFC_STATIC: i32 = 1;
pub const LIBBSC_CODER_QLFC_ADAPTIVE: i32 = 2;
pub const LIBBSC_CODER_QLFC_FAST: i32 = 3;

pub const LIBBSC_FEATURE_NONE: i32 = 0;
pub const LIBBSC_FEATURE_FASTMODE: i32 = 1;

pub const LIBBSC_DEFAULT_LZPHASHSIZE: i32 = 15;
pub const LIBBSC_DEFAULT_LZPMINLEN: i32 = 72;

extern "C" {
    fn bsc_init(features: i32) -> i32;
    fn bsc_compress(
        input: *const u8,
        output: *mut u8,
        n: i32,
        lzp_hash_size: i32,
        lzp_min_len: i32,
        block_sorter: i32,
        coder: i32,
        features: i32,
    ) -> i32;
    fn bsc_block_info(
        block_header: *const u8,
        header_size: i32,
        p_block_size: *mut i32,
        p_data_size: *mut i32,
        features: i32,
    ) -> i32;
    fn bsc_decompress(
        input: *const u8,
        input_size: i32,
        output: *mut u8,
        output_size: i32,
        features: i32,
    ) -> i32;
}

fn ensure_init(features: i32) -> Result<()> {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    static mut RC: i32 = 0;
    ONCE.call_once(|| unsafe {
        RC = bsc_init(features);
    });
    let rc = unsafe { RC };
    if rc != LIBBSC_NO_ERROR {
        return Err(ArcError::Codec {
            codec: "bsc",
            message: format!("bsc_init failed: {}", rc),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct BscOptions {
    pub block_sorter: i32,
    pub coder: i32,
    pub lzp_hash_size: i32,
    pub lzp_min_len: i32,
    pub features: i32,
    /// Max bytes per BWT block. libbsc supports up to ~1 GiB but practical
    /// blocks are 16-64 MiB. 25 MiB is the upstream default for `bsc`.
    pub block_size: usize,
}

impl Default for BscOptions {
    fn default() -> Self {
        Self {
            block_sorter: LIBBSC_BLOCKSORTER_BWT,
            coder: LIBBSC_CODER_QLFC_ADAPTIVE,
            lzp_hash_size: LIBBSC_DEFAULT_LZPHASHSIZE,
            lzp_min_len: LIBBSC_DEFAULT_LZPMINLEN,
            features: LIBBSC_FEATURE_NONE,
            block_size: 25 * 1024 * 1024,
        }
    }
}

impl BscOptions {
    pub fn fast() -> Self {
        Self {
            coder: LIBBSC_CODER_QLFC_FAST,
            features: LIBBSC_FEATURE_FASTMODE,
            ..Self::default()
        }
    }

    pub fn max() -> Self {
        Self {
            block_sorter: LIBBSC_BLOCKSORTER_BWT,
            coder: LIBBSC_CODER_QLFC_ADAPTIVE,
            block_size: 64 * 1024 * 1024,
            ..Self::default()
        }
    }
}

pub struct BscCodec {
    options: BscOptions,
    name: &'static str,
}

impl BscCodec {
    pub fn new(options: BscOptions) -> Self {
        Self {
            options,
            name: "bsc",
        }
    }

    pub fn with_name(options: BscOptions, name: &'static str) -> Self {
        Self { options, name }
    }
}

impl Codec for BscCodec {
    fn name(&self) -> &'static str {
        self.name
    }

    fn compress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        ensure_init(self.options.features)?;

        let mut src = Vec::new();
        input.read_to_end(&mut src)?;
        let mut total_in: u64 = 0;
        let mut total_out: u64 = 0;

        let bs = self.options.block_size.max(1);
        let mut block_out = vec![0u8; bs + LIBBSC_HEADER_SIZE + 1024];

        // Stream-of-blocks: [u32 LE compressed_size][compressed_block]...[u32 LE 0]
        for chunk in src.chunks(bs) {
            let n = chunk.len() as i32;
            let rc = unsafe {
                bsc_compress(
                    chunk.as_ptr(),
                    block_out.as_mut_ptr(),
                    n,
                    self.options.lzp_hash_size,
                    self.options.lzp_min_len,
                    self.options.block_sorter,
                    self.options.coder,
                    self.options.features,
                )
            };
            if rc < 0 {
                return Err(ArcError::Codec {
                    codec: "bsc",
                    message: format!("bsc_compress error {}", rc),
                });
            }
            let compressed = rc as usize;
            let len = compressed as u32;
            output.write_all(&len.to_le_bytes())?;
            output.write_all(&block_out[..compressed])?;
            total_in += chunk.len() as u64;
            total_out += 4 + compressed as u64;
        }
        // sentinel
        output.write_all(&0u32.to_le_bytes())?;
        total_out += 4;

        Ok(CodecReport {
            bytes_in: total_in,
            bytes_out: total_out,
        })
    }

    fn decompress(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<CodecReport> {
        ensure_init(self.options.features)?;

        let mut src = Vec::new();
        input.read_to_end(&mut src)?;
        let mut pos = 0usize;
        let mut total_out: u64 = 0;

        loop {
            if pos + 4 > src.len() {
                return Err(ArcError::Codec {
                    codec: "bsc",
                    message: "truncated bsc stream".into(),
                });
            }
            let len = u32::from_le_bytes(src[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4;
            if len == 0 {
                break;
            }
            if pos + len > src.len() {
                return Err(ArcError::Codec {
                    codec: "bsc",
                    message: "truncated bsc block".into(),
                });
            }
            let block = &src[pos..pos + len];
            pos += len;

            if block.len() < LIBBSC_HEADER_SIZE {
                return Err(ArcError::Codec {
                    codec: "bsc",
                    message: "block smaller than header".into(),
                });
            }
            let mut block_size = 0i32;
            let mut data_size = 0i32;
            let rc = unsafe {
                bsc_block_info(
                    block.as_ptr(),
                    LIBBSC_HEADER_SIZE as i32,
                    &mut block_size,
                    &mut data_size,
                    self.options.features,
                )
            };
            if rc != LIBBSC_NO_ERROR {
                return Err(ArcError::Codec {
                    codec: "bsc",
                    message: format!("bsc_block_info error {}", rc),
                });
            }
            if block_size as usize != block.len() {
                return Err(ArcError::Codec {
                    codec: "bsc",
                    message: format!(
                        "block_size mismatch header={} got={}",
                        block_size,
                        block.len()
                    ),
                });
            }
            let mut decoded = vec![0u8; data_size as usize];
            let rc = unsafe {
                bsc_decompress(
                    block.as_ptr(),
                    block.len() as i32,
                    decoded.as_mut_ptr(),
                    data_size,
                    self.options.features,
                )
            };
            if rc != LIBBSC_NO_ERROR {
                return Err(ArcError::Codec {
                    codec: "bsc",
                    message: format!("bsc_decompress error {}", rc),
                });
            }
            output.write_all(&decoded)?;
            total_out += decoded.len() as u64;
        }

        Ok(CodecReport {
            bytes_in: src.len() as u64,
            bytes_out: total_out,
        })
    }

    fn memory_usage(&self, _direction: Direction) -> MemoryUsage {
        // libbsc working set ≈ 6 * blocksize for BWT (libsais).
        MemoryUsage {
            working_bytes: (self.options.block_size as u64) * 6,
            ..MemoryUsage::default()
        }
    }
}
