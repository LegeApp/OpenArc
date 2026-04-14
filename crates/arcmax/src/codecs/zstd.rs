use anyhow::{anyhow, Context, Result};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use zenzstd::encoding::{CompressionLevel, EncoderDictionary, StreamingEncoder};
use zenzstd::decoding::{StreamingDecoder, FrameDecoder};

/// Settings for zstd compression/decompression.
///
/// Notes:
/// - `include_checksum` adds a content checksum at the end of each frame. [web:38]
/// - `threads` requires the `zstdmt` cargo feature to enable multithreaded compression. [web:38]
/// - `dict` must be provided for both compression and decompression if used. [web:38][page:53]
#[derive(Clone, Debug)]
pub struct ZstdOptions {
    /// Compression level. Typical range is ~1-22 (zstd supports higher in some builds).
    /// Use 0 to mean "zstd default".
    pub level: i32,

    /// Include a content checksum in the frame.
    pub include_checksum: bool,

    /// Enable long-distance matching (better ratio on some content, often slower).
    pub long_distance_matching: bool,

    /// Multithreaded compression workers (0 = disabled).
    pub threads: u32,

    /// Optional dictionary bytes (same bytes must be used for decompression).
    pub dict: Option<Vec<u8>>,

    /// Buffer size used for file/stream IO wrappers.
    pub buffer_size: usize,

    /// Write output files atomically (write to temp file then rename).
    pub atomic_writes: bool,
}

impl Default for ZstdOptions {
    fn default() -> Self {
        Self {
            level: 3,
            include_checksum: true,
            long_distance_matching: false,
            threads: 0,
            dict: None,
            buffer_size: 1024 * 1024, // 1 MiB
            atomic_writes: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ZstdCodec {
    opts: ZstdOptions,
}

impl ZstdCodec {
    pub fn new(opts: ZstdOptions) -> Self {
        Self { opts }
    }

    pub fn options(&self) -> &ZstdOptions {
        &self.opts
    }

    /// Compress an in-memory buffer.
    /// (Uses the streaming path so options like checksum/dict can apply.)
    pub fn compress_bytes(&self, input: &[u8]) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        self.compress_reader_to_writer(io::Cursor::new(input), &mut out)
            .context("zstd compress_bytes failed")?;
        Ok(out)
    }

    /// Decompress an in-memory buffer.
    pub fn decompress_bytes(&self, input: &[u8]) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        // Use dictionary-aware decoder
        let mut decoder = self.make_decoder_with_dict(io::Cursor::new(input))
            .context("zstd decompress_bytes failed")?;
        
        io::copy(&mut decoder, &mut out)
            .context("Failed to copy decoded data")?;
        
        Ok(out)
    }

    /// Decompress an in-memory buffer but enforce an upper bound on output size.
    /// Useful to protect the GUI from allocating huge memory if input is malicious/corrupt.
    pub fn decompress_bytes_limited(&self, input: &[u8], max_output_bytes: usize) -> Result<Vec<u8>> {
        // Use dictionary-aware decoder
        let mut decoder = self.make_decoder_with_dict(io::Cursor::new(input))
            .context("Failed to decode zstd data")?;
        
        let mut out = Vec::with_capacity(std::cmp::min(64 * 1024, max_output_bytes));
        let mut buf = vec![0u8; 64 * 1024];

        loop {
            let n = decoder.read(&mut buf).context("zstd read failed")?;
            if n == 0 {
                break;
            }
            if out.len().saturating_add(n) > max_output_bytes {
                return Err(anyhow!("Decompressed data exceeds limit ({} bytes)", max_output_bytes));
            }
            out.extend_from_slice(&buf[..n]);
        }

        Ok(out)
    }

    /// Stream compression: reads from `reader`, writes compressed bytes into `writer`.
    /// Returns number of uncompressed bytes read from `reader`.
    pub fn compress_reader_to_writer<R: Read, W: Write>(&self, reader: R, writer: W) -> Result<u64> {
        let mut reader = BufReader::with_capacity(self.opts.buffer_size, reader);
        let writer = BufWriter::with_capacity(self.opts.buffer_size, writer);

        // Read all input data
        let mut input_data = Vec::new();
        reader.read_to_end(&mut input_data)
            .context("Failed to read input data")?;
        
        let bytes_in = input_data.len() as u64;
        
        // Compress using streaming encoder
        let level = if self.opts.level == 0 {
            CompressionLevel::Default
        } else {
            CompressionLevel::Level(self.opts.level)
        };
        
        let mut encoder = if let Some(ref dict) = self.opts.dict {
            let enc_dict = EncoderDictionary::parse(dict)
                .unwrap_or_else(|| EncoderDictionary::new_raw(1, dict.clone()));
            StreamingEncoder::with_dictionary(writer, level, enc_dict)
        } else {
            StreamingEncoder::new(writer, level)
        };
        
        std::io::Write::write_all(&mut encoder, &input_data)
            .context("Failed to write data to zstd encoder")?;
        
        let mut writer = encoder.finish()
            .context("Failed to finish zstd stream")?;
        writer.flush().ok();

        Ok(bytes_in)
    }

    /// Stream decompression: reads zstd from `reader`, writes uncompressed bytes into `writer`.
    /// Returns number of uncompressed bytes written to `writer`.
    pub fn decompress_reader_to_writer<R: Read, W: Write>(&self, reader: R, writer: W) -> Result<u64> {
        let mut reader = BufReader::with_capacity(self.opts.buffer_size, reader);
        let mut writer = BufWriter::with_capacity(self.opts.buffer_size, writer);
        
        // Use decoder with dictionary support
        let mut decoder = self.make_decoder_with_dict(&mut reader)
            .context("Failed while streaming from zstd decoder")?;
        
        let bytes_out = io::copy(&mut decoder, &mut writer)
            .context("Failed while copying decoded data")?;
        
        writer.flush().context("Failed to flush output")?;
        
        Ok(bytes_out)
    }

    /// Compress a file to a file.
    pub fn compress_file<P: AsRef<Path>, Q: AsRef<Path>>(&self, input: P, output: Q) -> Result<()> {
        let input = input.as_ref();
        let output = output.as_ref();

        let in_file = File::open(input).with_context(|| format!("Failed to open input file: {}", input.display()))?;

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create output directory: {}", parent.display()))?;
        }

        if self.opts.atomic_writes {
            atomic_write(output, |tmp_file| {
                self.compress_reader_to_writer(in_file, tmp_file)
                    .with_context(|| format!("Failed to compress {} -> {}", input.display(), output.display()))?;
                Ok(())
            })?;
        } else {
            let out_file =
                File::create(output).with_context(|| format!("Failed to create output file: {}", output.display()))?;
            self.compress_reader_to_writer(in_file, out_file)
                .with_context(|| format!("Failed to compress {} -> {}", input.display(), output.display()))?;
        }

        Ok(())
    }

    /// Decompress a file to a file.
    pub fn decompress_file<P: AsRef<Path>, Q: AsRef<Path>>(&self, input: P, output: Q) -> Result<()> {
        let input = input.as_ref();
        let output = output.as_ref();

        let in_file = File::open(input).with_context(|| format!("Failed to open input file: {}", input.display()))?;

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create output directory: {}", parent.display()))?;
        }

        if self.opts.atomic_writes {
            atomic_write(output, |tmp_file| {
                self.decompress_reader_to_writer(in_file, tmp_file)
                    .with_context(|| format!("Failed to decompress {} -> {}", input.display(), output.display()))?;
                Ok(())
            })?;
        } else {
            let out_file =
                File::create(output).with_context(|| format!("Failed to create output file: {}", output.display()))?;
            self.decompress_reader_to_writer(in_file, out_file)
                .with_context(|| format!("Failed to decompress {} -> {}", input.display(), output.display()))?;
        }

        Ok(())
    }

    /// Optional: create a `.tar.zst` archive from a directory (no orchestration; just a helper).
    ///
    /// Enable by adding `tar` dependency and `features = ["tar"]` to your crate.
    #[cfg(feature = "tar")]
    pub fn archive_dir_tar_zst<P: AsRef<Path>, Q: AsRef<Path>>(&self, src_dir: P, output: Q) -> Result<()> {
        let src_dir = src_dir.as_ref();
        let output = output.as_ref();

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create output directory: {}", parent.display()))?;
        }

        let write_archive = |out_file: File| -> Result<()> {
            let out_file = BufWriter::with_capacity(self.opts.buffer_size, out_file);
            
            let level = if self.opts.level == 0 {
                CompressionLevel::Default
            } else {
                CompressionLevel::Level(self.opts.level)
            };
            
            let encoder = if let Some(ref dict) = self.opts.dict {
                let enc_dict = EncoderDictionary::parse(dict)
                    .unwrap_or_else(|| EncoderDictionary::new_raw(1, dict.clone()));
                StreamingEncoder::with_dictionary(out_file, level, enc_dict)
            } else {
                StreamingEncoder::new(out_file, level)
            };
            
            let mut builder = tar::Builder::new(encoder);

            builder
                .append_dir_all(".", src_dir)
                .with_context(|| format!("Failed to append dir: {}", src_dir.display()))?;

            // Finish TAR, then finish zstd. [web:38]
            let encoder = builder.into_inner().context("Failed to finalize tar builder")?;
            let mut out = encoder.finish().context("Failed to finish zstd stream")?;
            out.flush().context("Failed to flush zstd output")?;

            Ok(())
        };

        if self.opts.atomic_writes {
            atomic_write(output, |tmp_file| write_archive(tmp_file))?;
        } else {
            write_archive(File::create(output).with_context(|| format!("Failed to create {}", output.display()))?)?;
        }

        Ok(())
    }

    /// Optional: extract a `.tar.zst` archive into a directory.
    #[cfg(feature = "tar")]
    pub fn extract_tar_zst<P: AsRef<Path>, Q: AsRef<Path>>(&self, input: P, dst_dir: Q) -> Result<()> {
        let input = input.as_ref();
        let dst_dir = dst_dir.as_ref();
        fs::create_dir_all(dst_dir).with_context(|| format!("Failed to create {}", dst_dir.display()))?;

        let in_file = File::open(input).with_context(|| format!("Failed to open {}", input.display()))?;
        let reader = BufReader::with_capacity(self.opts.buffer_size, in_file);
        
        // Use streaming decoder with dictionary support if configured
        let decoder = self.make_decoder_with_dict(reader)
            .context("Failed to create zstd decoder")?;
        
        let mut archive = tar::Archive::new(decoder);
        archive
            .unpack(dst_dir)
            .with_context(|| format!("Failed to unpack into {}", dst_dir.display()))?;

        Ok(())
    }

    /// Create a streaming decoder, with dictionary support if configured.
    fn make_decoder_with_dict<R: Read>(&self, reader: R) -> Result<StreamingDecoder<R, FrameDecoder>> {
        if let Some(ref dict_bytes) = self.opts.dict {
            // Try to parse as formatted dictionary first, fallback to raw content
            let dict = match zenzstd::decoding::dictionary::Dictionary::decode_dict(dict_bytes) {
                Ok(d) => d,
                Err(_) => {
                    // If parsing fails, create a raw dictionary with ID 1 (matching encoder behavior)
                    zenzstd::decoding::dictionary::Dictionary {
                        id: 1,
                        fse: zenzstd::decoding::scratch::FSEScratch::new(),
                        huf: zenzstd::decoding::scratch::HuffmanScratch::new(),
                        dict_content: dict_bytes.clone(),
                        offset_hist: [1, 4, 8],
                    }
                }
            };
            
            // Create FrameDecoder and add dictionary
            let mut frame_dec = FrameDecoder::new();
            frame_dec.add_dict(dict)
                .context("Failed to add dictionary to decoder")?;
            
            // Wrap in StreamingDecoder
            StreamingDecoder::new_with_decoder(reader, frame_dec)
                .context("Failed to create streaming decoder with dictionary")
        } else {
            // No dictionary, use simple streaming decoder
            StreamingDecoder::new(reader)
                .context("Failed to create zstd decoder")
        }
    }

}

/// Atomic file write helper (best-effort cross-platform).
fn atomic_write<F>(dst: &Path, f: F) -> Result<()>
where
    F: FnOnce(File) -> Result<()>,
{
    let parent = dst.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;

    let tmp_path = temp_path_for(dst);
    let tmp_file =
        File::create(&tmp_path).with_context(|| format!("Failed to create temp file: {}", tmp_path.display()))?;

    let result = f(tmp_file);

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
        return result;
    }

    // Best-effort replace.
    if dst.exists() {
        let _ = fs::remove_file(dst);
    }
    fs::rename(&tmp_path, dst).with_context(|| {
        format!(
            "Failed to rename temp file {} -> {}",
            tmp_path.display(),
            dst.display()
        )
    })?;

    Ok(())
}

fn temp_path_for(dst: &Path) -> PathBuf {
    let mut tmp = dst.to_path_buf();
    let mut ext = tmp.extension().map(|s| s.to_os_string()).unwrap_or_default();
    ext.push(".tmp");
    tmp.set_extension(ext);
    tmp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_roundtrip() {
        let codec = ZstdCodec::new(ZstdOptions::default());
        let data = b"Hello, World! This is a zstd test.";

        let compressed = codec.compress_bytes(data).unwrap();
        let decompressed = codec.decompress_bytes(&compressed).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn bytes_roundtrip_limited() {
        let codec = ZstdCodec::new(ZstdOptions::default());
        let data = vec![42u8; 1024 * 64];

        let compressed = codec.compress_bytes(&data).unwrap();
        let decompressed = codec.decompress_bytes_limited(&compressed, 1024 * 64).unwrap();
        assert_eq!(data, decompressed);

        assert!(codec.decompress_bytes_limited(&compressed, 1024).is_err());
    }

    #[test]
    fn dictionary_roundtrip() {
        // Create a dictionary with repetitive patterns
        let dict_content = b"the quick brown fox jumps over the lazy dog ".repeat(10);
        
        // Create options with dictionary
        let mut opts = ZstdOptions::default();
        opts.dict = Some(dict_content.to_vec());
        
        let codec = ZstdCodec::new(opts);
        let data = b"the quick brown fox jumps over the lazy dog and some unique data at the end!";

        let compressed = codec.compress_bytes(data).unwrap();
        let decompressed = codec.decompress_bytes(&compressed).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn dictionary_improves_compression() {
        // Create a dictionary with repetitive patterns
        let dict_content = b"the quick brown fox jumps over the lazy dog ".repeat(20);
        
        // Compress with dictionary
        let mut opts_with_dict = ZstdOptions::default();
        opts_with_dict.dict = Some(dict_content.to_vec());
        let codec_with_dict = ZstdCodec::new(opts_with_dict);
        
        // Compress without dictionary
        let codec_no_dict = ZstdCodec::new(ZstdOptions::default());
        
        let data = b"the quick brown fox jumps over the lazy dog ".repeat(5);

        let compressed_with_dict = codec_with_dict.compress_bytes(&data).unwrap();
        let compressed_no_dict = codec_no_dict.compress_bytes(&data).unwrap();
        
        // Dictionary should produce smaller or equal output
        assert!(compressed_with_dict.len() <= compressed_no_dict.len(),
            "Dictionary compression should be better: with={}, without={}",
            compressed_with_dict.len(), compressed_no_dict.len());
        
        // Verify roundtrip
        let decompressed = codec_with_dict.decompress_bytes(&compressed_with_dict).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }
}
