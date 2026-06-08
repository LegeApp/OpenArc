use std::fs;
use std::io;
use std::path::Path;

use crate::error::Result;

/// Compress a directory tree into a streaming tar.zst file.
///
/// Files are stored with paths relative to `dir` (i.e. `./filename`).
pub fn archive_dir_tar_zst(dir: &Path, output: &Path, level: i32) -> Result<()> {
    let f = fs::File::create(output)?;
    let enc = zstd::Encoder::new(f, level)?;
    let mut ar = tar::Builder::new(enc);
    ar.append_dir_all(".", dir)?;
    ar.finish()?;
    let enc = ar.into_inner()?;
    enc.finish()?;
    Ok(())
}

/// Decompress and unpack a tar.zst archive into a directory.
pub fn extract_tar_zst(archive: &Path, output_dir: &Path) -> Result<()> {
    let f = fs::File::open(archive)?;
    let dec = zstd::Decoder::new(f)?;
    let mut ar = tar::Archive::new(dec);
    ar.unpack(output_dir)?;
    Ok(())
}

/// Open a tar.zst archive for streaming entry-by-entry reads.
///
/// Returns a boxed `Read` over the decompressed stream.  Wrap with
/// `tar::Archive::new(reader)` to iterate entries.
pub fn open_zst_reader(archive: &Path) -> Result<Box<dyn io::Read>> {
    let f = fs::File::open(archive)?;
    let dec = zstd::Decoder::new(f)?;
    Ok(Box::new(dec))
}
