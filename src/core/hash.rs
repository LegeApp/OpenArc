use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

pub fn sha256_bytes_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

pub fn sha256_file_hex(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let mut file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    sha256_reader_hex(&mut file).with_context(|| format!("Failed to hash {}", path.display()))
}

pub fn sha256_reader_hex<R: Read>(reader: &mut R) -> Result<String> {
    let mut h = Sha256::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = reader.read(&mut buf).context("Failed to read while hashing")?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

pub fn build_dedup_map(files: &[PathBuf]) -> Result<HashMap<String, Vec<PathBuf>>> {
    let mut map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for f in files {
        let hash = sha256_file_hex(f)?;
        map.entry(hash).or_default().push(f.clone());
    }
    Ok(map)
}

pub fn write_hashes_file(hashes: &[(String, String)], output_path: impl AsRef<Path>) -> Result<()> {
    let output_path = output_path.as_ref();
    let mut out = std::fs::File::create(output_path)
        .with_context(|| format!("Failed to create {}", output_path.display()))?;

    for (hash_hex, rel_path) in hashes {
        use std::io::Write;
        writeln!(out, "{}  {}", hash_hex, rel_path)?;
    }

    Ok(())
}

pub fn read_hashes_file(path: impl AsRef<Path>) -> Result<Vec<(String, String)>> {
    let path = path.as_ref();
    let f = std::fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let r = BufReader::new(f);
    let mut out = Vec::new();

    for line in r.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let hash = parts.next().ok_or_else(|| anyhow!("Invalid hashes line"))?.to_string();
        let rel = parts.next().ok_or_else(|| anyhow!("Invalid hashes line"))?.to_string();
        out.push((hash, rel));
    }

    Ok(out)
}

pub fn verify_dir_against_hashes(root_dir: impl AsRef<Path>, hashes_file: impl AsRef<Path>) -> Result<()> {
    let root_dir = root_dir.as_ref();
    let hashes_file = hashes_file.as_ref();

    let entries = read_hashes_file(hashes_file)?;
    for (expected_hash, rel) in entries {
        let path = root_dir.join(rel);
        let actual = sha256_file_hex(&path)?;
        if actual != expected_hash {
            return Err(anyhow!(
                "Hash mismatch for {} (expected {}, got {})",
                path.display(),
                expected_hash,
                actual
            ));
        }
    }

    Ok(())
}

pub fn verify_tar_zst_archive(zstd: &zstd_archive::ZstdCodec, archive_path: impl AsRef<Path>) -> Result<()> {
    let archive_path = archive_path.as_ref();
    let tmp = tempfile::TempDir::new().context("Failed to create temp dir")?;
    zstd.extract_tar_zst(archive_path, tmp.path())
        .with_context(|| format!("Failed to extract {}", archive_path.display()))?;

    let hashes_path = tmp.path().join("HASHES.sha256");
    verify_dir_against_hashes(tmp.path(), &hashes_path)
}
