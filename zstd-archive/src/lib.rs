use anyhow::{anyhow, Context, Result};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

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
        self.decompress_reader_to_writer(io::Cursor::new(input), &mut out)
            .context("zstd decompress_bytes failed")?;
        Ok(out)
    }

    /// Decompress an in-memory buffer but enforce an upper bound on output size.
    /// Useful to protect the GUI from allocating huge memory if input is malicious/corrupt.
    pub fn decompress_bytes_limited(&self, input: &[u8], max_output_bytes: usize) -> Result<Vec<u8>> {
        let mut decoder = self.make_decoder(BufReader::new(io::Cursor::new(input)))
            .context("Failed to create zstd decoder")?;

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

        let mut encoder = self
            .make_encoder(writer)
            .context("Failed to create zstd encoder")?;

        let bytes_in = io::copy(&mut reader, &mut encoder).context("Failed while streaming into zstd encoder")?;

        // Required to finalize the compressed stream. [web:38]
        let mut writer = encoder.finish().context("Failed to finish zstd stream")?;
        writer.flush().ok();

        Ok(bytes_in)
    }

    /// Stream decompression: reads zstd from `reader`, writes uncompressed bytes into `writer`.
    /// Returns number of uncompressed bytes written to `writer`.
    pub fn decompress_reader_to_writer<R: Read, W: Write>(&self, reader: R, writer: W) -> Result<u64> {
        let reader = BufReader::with_capacity(self.opts.buffer_size, reader);
        let mut decoder = self.make_decoder(reader).context("Failed to create zstd decoder")?;

        let mut writer = BufWriter::with_capacity(self.opts.buffer_size, writer);
        let bytes_out = io::copy(&mut decoder, &mut writer).context("Failed while streaming from zstd decoder")?;
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
            let encoder = self.make_encoder(out_file).context("Failed to create zstd encoder")?;
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
        let decoder = self.make_decoder(reader).context("Failed to create zstd decoder")?;

        let mut archive = tar::Archive::new(decoder);
        archive
            .unpack(dst_dir)
            .with_context(|| format!("Failed to unpack into {}", dst_dir.display()))?;

        Ok(())
    }

    fn make_encoder<W: Write>(&self, writer: W) -> Result<zstd::stream::write::Encoder<'static, W>> {
        // Level 0 means "zstd default" in the zstd crate API. [web:38]
        let level = self.opts.level;

        let mut enc = if let Some(ref dict) = self.opts.dict {
            zstd::stream::write::Encoder::with_dictionary(writer, level, dict)
                .context("Failed to create zstd encoder (dictionary)")?
        } else {
            zstd::stream::write::Encoder::new(writer, level).context("Failed to create zstd encoder")?
        };

        enc.include_checksum(self.opts.include_checksum)
            .context("Failed to set zstd include_checksum")?; // [web:38]

        enc.long_distance_matching(self.opts.long_distance_matching)
            .context("Failed to set zstd long_distance_matching")?; // [web:38]

        if self.opts.threads > 0 {
            #[cfg(feature = "zstdmt")]
            {
                enc.multithread(self.opts.threads)
                    .context("Failed to enable zstd multithread")?; // [web:38]
            }
            #[cfg(not(feature = "zstdmt"))]
            {
                return Err(anyhow!(
                    "threads={} requested but zstdmt feature is not enabled",
                    self.opts.threads
                ));
            }
        }

        Ok(enc)
    }

    fn make_decoder<R: io::BufRead>(&self, reader: R) -> Result<zstd::stream::read::Decoder<'static, R>> {
        if let Some(ref dict) = self.opts.dict {
            zstd::stream::read::Decoder::with_dictionary(reader, dict)
                .context("Failed to create zstd decoder (dictionary)") // [page:53]
        } else {
            zstd::stream::read::Decoder::with_buffer(reader).context("Failed to create zstd decoder")
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
}
