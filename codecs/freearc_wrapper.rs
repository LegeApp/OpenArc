//! FreeArc compression wrapper for miscellaneous files
//! 
//! Provides a simplified interface to create FreeArc archives for files that
//! don't benefit from specialized media codecs.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// FreeArc compression settings
#[derive(Debug, Clone)]
pub struct FreeArcSettings {
    /// Compression method (e.g., "arc:max", "arc:m4")
    pub method: String,
    /// Additional options
    pub options: Vec<String>,
}

impl Default for FreeArcSettings {
    fn default() -> Self {
        Self {
            method: "arc:max".to_string(), // Maximum compression
            options: vec![],
        }
    }
}

/// Create a FreeArc archive from a directory or list of files
pub fn create_freearc_archive(
    input_paths: &[impl AsRef<Path>],
    output_archive: impl AsRef<Path>,
    settings: &FreeArcSettings,
) -> Result<()> {
    let output_archive = output_archive.as_ref();

    // Ensure output directory exists
    if let Some(parent) = output_archive.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Build FreeArc command
    // Note: This assumes 'arc' or 'FreeArc' is in PATH
    // Fallback to using 7z with FreeArc plugin if needed
    let arc_command = if which::which("arc").is_ok() {
        "arc"
    } else if which::which("FreeArc").is_ok() {
        "FreeArc"
    } else {
        // Fallback to 7z with high compression
        return create_7z_archive(input_paths, output_archive);
    };

    let mut cmd = Command::new(arc_command);
    cmd.arg("a"); // Add to archive
    cmd.arg(format!("-m{}", settings.method)); // Compression method
    
    // Add custom options
    for opt in &settings.options {
        cmd.arg(opt);
    }

    cmd.arg(output_archive);

    // Add input paths
    for path in input_paths {
        cmd.arg(path.as_ref());
    }

    let output = cmd.output()
        .context("Failed to execute FreeArc - ensure it's installed")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("FreeArc compression failed: {}", stderr);
    }

    Ok(())
}

/// Fallback: Create a 7z archive with maximum compression
fn create_7z_archive(
    input_paths: &[impl AsRef<Path>],
    output_archive: impl AsRef<Path>,
) -> Result<()> {
    let output_archive = output_archive.as_ref();

    let mut cmd = Command::new("7z");
    cmd.arg("a"); // Add
    cmd.arg("-t7z"); // 7z format
    cmd.arg("-mx=9"); // Maximum compression
    cmd.arg("-m0=lzma2"); // LZMA2 method
    cmd.arg("-ms=on"); // Solid archive
    cmd.arg(output_archive);

    for path in input_paths {
        cmd.arg(path.as_ref());
    }

    let output = cmd.output()
        .context("Failed to execute 7z - ensure 7-Zip is installed")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("7z compression failed: {}", stderr);
    }

    Ok(())
}

/// Extract a FreeArc archive
pub fn extract_freearc_archive(
    archive_path: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> Result<()> {
    let archive_path = archive_path.as_ref();
    let output_dir = output_dir.as_ref();

    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;

    // Try FreeArc first
    let arc_command = if which::which("arc").is_ok() {
        "arc"
    } else if which::which("FreeArc").is_ok() {
        "FreeArc"
    } else {
        // Fallback to 7z
        let mut cmd = Command::new("7z");
        cmd.arg("x"); // Extract
        cmd.arg(archive_path);
        cmd.arg(format!("-o{}", output_dir.display()));
        
        let output = cmd.output()
            .context("Failed to execute 7z")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("7z extraction failed: {}", stderr);
        }

        return Ok(());
    };

    let mut cmd = Command::new(arc_command);
    cmd.arg("x"); // Extract
    cmd.arg(archive_path);
    cmd.arg(format!("-dp{}", output_dir.display()));

    let output = cmd.output()
        .context("Failed to execute FreeArc")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("FreeArc extraction failed: {}", stderr);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    #[ignore] // Requires FreeArc or 7-Zip to be installed
    fn test_create_and_extract() -> Result<()> {
        let temp = TempDir::new()?;
        
        // Create test files
        let test_file1 = temp.path().join("test1.txt");
        let test_file2 = temp.path().join("test2.txt");
        fs::write(&test_file1, b"Hello, world!")?;
        fs::write(&test_file2, b"FreeArc test")?;

        // Create archive
        let archive_path = temp.path().join("test.arc");
        create_freearc_archive(
            &[test_file1.clone(), test_file2.clone()],
            &archive_path,
            &FreeArcSettings::default(),
        )?;

        assert!(archive_path.exists());

        // Extract archive
        let extract_dir = temp.path().join("extracted");
        extract_freearc_archive(&archive_path, &extract_dir)?;

        assert!(extract_dir.join("test1.txt").exists());
        assert!(extract_dir.join("test2.txt").exists());

        Ok(())
    }
}
