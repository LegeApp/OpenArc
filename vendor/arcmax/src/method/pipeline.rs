use std::borrow::Cow;
use std::io::{self, Read, Write};
use std::path::Path;
use std::str::FromStr;

use crate::error::{ArcError, Result};
use crate::method::planner::{self, PipelineContext};
use crate::method::Method;

/// Magic bytes for the ArcMax Frame Block stream format.
const FRAME_MAGIC: &[u8; 4] = b"AMFB";
const FRAME_VERSION: u8 = 2;
/// Default block size: 8 MiB. All chunks fed to compressor stages are at most this large.
const DEFAULT_BLOCK_SIZE: usize = 8 * 1024 * 1024;

/// Executes a compression/decompression pipeline described by a [`Method`].
///
/// ## Wire format (block-framed stream)
///
/// ```text
/// [magic: b"AMFB"][version: u8 = 2][block_size_hint: u32 LE]
/// [method_len: u16 LE][method_utf8: method_len bytes]
/// ([compressed_len: u32 LE][compressed_bytes: compressed_len bytes])*
/// [terminator: u32 LE = 0]
/// ```
///
/// Each record holds one independently compressed block of at most `block_size`
/// uncompressed bytes. The terminator signals end-of-stream.
///
/// ## SREP carve-out
///
/// SREP's dedup value depends on cross-chunk context. When the pipeline
/// contains a `Method::Srep` stage, all stages up to and including SREP
/// are applied to the full input as a single pass; only the downstream
/// stages are chunked.
#[derive(Debug, Clone)]
pub struct CodecPipeline {
    method: Method,
    context: PipelineContext,
    block_size: usize,
}

impl CodecPipeline {
    pub fn new(method: Method) -> Self {
        Self {
            method,
            context: PipelineContext::default(),
            block_size: DEFAULT_BLOCK_SIZE,
        }
    }

    /// Construct a pipeline with a password for encrypted methods.
    pub fn with_password(method: Method, password: Vec<u8>) -> Self {
        Self {
            method,
            context: PipelineContext {
                password: Some(password),
            },
            block_size: DEFAULT_BLOCK_SIZE,
        }
    }

    /// Override the block size used during compression (default: 8 MiB).
    pub fn with_block_size(mut self, block_size: usize) -> Self {
        self.block_size = block_size;
        self
    }

    /// The method this pipeline was constructed from.
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Compress a file at `path` into `output`.
    ///
    /// When the `mmap` feature is enabled, the file is memory-mapped for
    /// zero-copy reads; this is particularly beneficial for large inputs fed
    /// into SREP, which otherwise allocates the entire file into a `Vec`.
    /// Falls back to ordinary `File` streaming if `mmap` fails (e.g. on pipes
    /// or network filesystems).
    pub fn compress_path<W: Write>(&mut self, path: &Path, output: W) -> Result<()> {
        if let Ok(bytes) = mmap_file(path) {
            return self.compress(bytes.as_ref(), output);
        }
        let file = std::fs::File::open(path).map_err(ArcError::Io)?;
        self.compress(file, output)
    }

    pub fn compress<R: Read, W: Write>(&mut self, mut input: R, mut output: W) -> Result<()> {
        if self.block_size > u32::MAX as usize {
            return Err(ArcError::InvalidMethod(format!(
                "block size exceeds frame limit: {}",
                self.block_size
            )));
        }

        // Write frame header.
        output.write_all(FRAME_MAGIC)?;
        output.write_all(&[FRAME_VERSION])?;
        output.write_all(&(self.block_size as u32).to_le_bytes())?;
        write_method_header(&self.method, &mut output)?;

        let mut all_input = Vec::new();
        input.read_to_end(&mut all_input)?;

        if let Some((whole_method, chunk_method)) = split_srep(&self.method) {
            // Run the SREP-and-predecessor stages on the full input.
            let mut whole_stages = planner::plan_with_context(&whole_method, &self.context);
            let mut buf = all_input;
            for stage in &mut whole_stages {
                buf = stage.compress(&buf)?;
            }

            // Chunk the SREP output through the remaining stages.
            let mut chunk_stages = plan_chunk_stages(&chunk_method, &self.context);
            for chunk in buf.chunks(self.block_size) {
                let compressed = compress_chunk(chunk, &mut chunk_stages)?;
                write_block(compressed.as_ref(), &mut output)?;
            }
        } else {
            let mut stages = plan_chunk_stages(&self.method, &self.context);
            for chunk in all_input.chunks(self.block_size) {
                let compressed = compress_chunk(chunk, &mut stages)?;
                write_block(compressed.as_ref(), &mut output)?;
            }
        }

        // Terminator.
        output.write_all(&0u32.to_le_bytes())?;
        Ok(())
    }

    pub fn decompress<R: Read, W: Write>(&mut self, mut input: R, mut output: W) -> Result<()> {
        // Verify frame header.
        let mut magic = [0u8; 4];
        input.read_exact(&mut magic)?;
        if &magic != FRAME_MAGIC {
            return Err(ArcError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "invalid frame magic: expected {:02x?}, got {:02x?}",
                    FRAME_MAGIC, magic
                ),
            )));
        }
        let mut ver = [0u8; 1];
        input.read_exact(&mut ver)?;
        if ver[0] != FRAME_VERSION {
            return Err(ArcError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported frame version {}", ver[0]),
            )));
        }
        let mut hint = [0u8; 4];
        input.read_exact(&mut hint)?;
        // block_size_hint is informational; we don't use it during decompression.

        let method = read_method_header(&mut input)?;

        if let Some((whole_method, chunk_method)) = split_srep(&method) {
            // Decompress each chunk through the post-SREP stages.
            let mut chunk_stages = planner::plan_with_context(&chunk_method, &self.context);
            let mut srep_buf = Vec::new();

            loop {
                let Some(block) = read_block(&mut input)? else {
                    break;
                };
                let plain = decompress_chunk(&block, &mut chunk_stages)?;
                srep_buf.extend_from_slice(&plain);
            }

            // Decompress the SREP-and-predecessor stages in reverse on the full buffer.
            let mut whole_stages = planner::plan_with_context(&whole_method, &self.context);
            let mut result = srep_buf;
            for stage in whole_stages.iter_mut().rev() {
                result = stage.decompress(&result)?;
            }
            output.write_all(&result)?;
        } else {
            let mut stages = planner::plan_with_context(&method, &self.context);
            loop {
                let Some(block) = read_block(&mut input)? else {
                    break;
                };
                let plain = decompress_chunk(&block, &mut stages)?;
                output.write_all(&plain)?;
            }
        }

        Ok(())
    }
}

// --- helpers -----------------------------------------------------------------

/// Compress `chunk` through all stages in order; returns the compressed bytes.
fn compress_chunk<'a>(chunk: &'a [u8], stages: &mut [planner::Stage]) -> Result<Cow<'a, [u8]>> {
    match stages {
        [] => Ok(Cow::Borrowed(chunk)),
        [first, rest @ ..] => {
            let mut buf = first.compress(chunk)?;
            for stage in rest.iter_mut() {
                buf = stage.compress(&buf)?;
            }
            Ok(Cow::Owned(buf))
        }
    }
}

fn plan_chunk_stages(method: &Method, context: &PipelineContext) -> Vec<planner::Stage> {
    if matches!(method, Method::Store) {
        Vec::new()
    } else {
        planner::plan_with_context(method, context)
    }
}

/// Decompress `block` through all stages in reverse order; returns the plain bytes.
fn decompress_chunk(block: &[u8], stages: &mut [planner::Stage]) -> Result<Vec<u8>> {
    let mut buf = block.to_vec();
    for stage in stages.iter_mut().rev() {
        buf = stage.decompress(&buf)?;
    }
    Ok(buf)
}

/// Write a length-prefixed block record.
fn write_block<W: Write>(data: &[u8], output: &mut W) -> Result<()> {
    let len = data.len() as u32;
    output.write_all(&len.to_le_bytes())?;
    output.write_all(data)?;
    Ok(())
}

/// Read a length-prefixed block record. Returns `None` at the terminator (len == 0).
fn read_block<R: Read>(input: &mut R) -> Result<Option<Vec<u8>>> {
    let mut len_bytes = [0u8; 4];
    input.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    if len == 0 {
        return Ok(None);
    }
    let mut block = vec![0u8; len];
    input.read_exact(&mut block)?;
    Ok(Some(block))
}

fn write_method_header<W: Write>(method: &Method, output: &mut W) -> Result<()> {
    let method = method.to_string();
    let len = u16::try_from(method.len()).map_err(|_| {
        ArcError::InvalidMethod(format!(
            "method string exceeds frame limit: {} bytes",
            method.len()
        ))
    })?;
    output.write_all(&len.to_le_bytes())?;
    output.write_all(method.as_bytes())?;
    Ok(())
}

fn read_method_header<R: Read>(input: &mut R) -> Result<Method> {
    let mut len_bytes = [0u8; 2];
    input.read_exact(&mut len_bytes)?;
    let len = u16::from_le_bytes(len_bytes) as usize;
    let mut method_bytes = vec![0u8; len];
    input.read_exact(&mut method_bytes)?;
    let method = std::str::from_utf8(&method_bytes)
        .map_err(|_| ArcError::InvalidMethod("frame method is not valid UTF-8".to_string()))?;
    Method::from_str(method)
}

/// If the method contains a `Srep` stage, split it into:
/// - `whole_method`: the stages up to and including SREP (applied to full input)
/// - `chunk_method`: the remaining downstream stages (applied per block)
///
/// Returns `None` when the method contains no SREP.
fn split_srep(method: &Method) -> Option<(Method, Method)> {
    match method {
        Method::Srep(_) => {
            // SREP alone: whole-file. Downstream is identity (Store).
            Some((method.clone(), Method::Store))
        }
        Method::Pipeline(stages) => {
            let srep_pos = stages.iter().position(|s| matches!(s, Method::Srep(_)))?;

            let whole: Vec<Method> = stages[..=srep_pos].to_vec();
            let whole_method = if whole.len() == 1 {
                whole.into_iter().next().unwrap()
            } else {
                Method::Pipeline(whole)
            };

            let rest: Vec<Method> = stages[srep_pos + 1..].to_vec();
            let chunk_method = if rest.is_empty() {
                Method::Store
            } else if rest.len() == 1 {
                rest.into_iter().next().unwrap()
            } else {
                Method::Pipeline(rest)
            };

            Some((whole_method, chunk_method))
        }
        _ => None,
    }
}

// --- mmap helper -------------------------------------------------------------

fn mmap_file(path: &Path) -> std::io::Result<memmap2::Mmap> {
    let file = std::fs::File::open(path)?;
    // SAFETY: the file is read-only and we do not modify the mapping.
    unsafe { memmap2::MmapOptions::new().map(&file) }
}
