use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let workspace_root = workspace_root();
    let extra: Vec<String> = env::args().skip(1).collect();

    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir(&workspace_root)
        .args(["build", "--release", "--package", "openarc"])
        .args(&extra);

    eprintln!("xtask: {:?}", cmd);
    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("xtask: failed to invoke cargo: {e}");
            return ExitCode::from(1);
        }
    };
    if !status.success() {
        return ExitCode::from(status.code().unwrap_or(1) as u8);
    }

    let exe_name = if cfg!(windows) { "openarc.exe" } else { "openarc" };
    let target_dir = workspace_root.join("target");

    let src = match locate_binary(&target_dir, exe_name) {
        Some(p) => p,
        None => {
            eprintln!("xtask: built binary not found under {}", target_dir.display());
            return ExitCode::from(1);
        }
    };

    let dist = workspace_root.join("dist");
    if let Err(e) = fs::create_dir_all(&dist) {
        eprintln!("xtask: cannot create {}: {e}", dist.display());
        return ExitCode::from(1);
    }
    let dst = dist.join(exe_name);
    if let Err(e) = fs::copy(&src, &dst) {
        eprintln!("xtask: copy {} -> {} failed: {e}", src.display(), dst.display());
        return ExitCode::from(1);
    }
    if let Err(e) = stage_runtime_dlls(&dst, &dist) {
        eprintln!("xtask: failed to stage runtime DLLs: {e}");
        return ExitCode::from(1);
    }

    println!("\nopenarc -> {}", dst.display());
    ExitCode::SUCCESS
}

fn locate_binary(target_dir: &PathBuf, exe_name: &str) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    candidates.push(target_dir.join("release").join(exe_name));
    if let Ok(triple) = env::var("CARGO_BUILD_TARGET") {
        candidates.push(target_dir.join(&triple).join("release").join(exe_name));
    }
    if let Ok(entries) = fs::read_dir(target_dir) {
        for entry in entries.flatten() {
            let p = entry.path().join("release").join(exe_name);
            if p.is_file() {
                candidates.push(p);
            }
        }
    }

    candidates.into_iter().find(|p| p.is_file())
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().expect("xtask/.. exists").to_path_buf()
}

/// Copy the MSYS2 runtime DLLs the binary actually imports.
///
/// Most codec libraries are statically linked, but MSYS2's static FFmpeg
/// archives were compiled against a few DLL-only libraries (the GLib stack
/// for the rsvg decoder, libhwy for jxl, shaderc for Vulkan filters), so the
/// exe carries DLL imports for those. We walk the import table transitively
/// with objdump and copy every DLL that resolves inside mingw64/bin; system
/// DLLs (kernel32 and friends) don't live there and are skipped naturally.
fn stage_runtime_dlls(exe: &Path, dist: &Path) -> Result<(), String> {
    if !cfg!(windows) {
        return Ok(());
    }

    let msys_bin = env::var_os("MSYS2_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\msys64"))
        .join("mingw64")
        .join("bin");
    let objdump = {
        let bundled = msys_bin.join("objdump.exe");
        if bundled.is_file() {
            bundled
        } else {
            PathBuf::from("objdump")
        }
    };

    let mut staged: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut pending: Vec<PathBuf> = vec![exe.to_path_buf()];

    while let Some(binary) = pending.pop() {
        let output = Command::new(&objdump)
            .arg("-p")
            .arg(&binary)
            .output()
            .map_err(|e| format!("failed to run {}: {e}", objdump.display()))?;
        if !output.status.success() {
            return Err(format!(
                "objdump -p {} failed: {}",
                binary.display(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Some(name) = line.trim().strip_prefix("DLL Name:") else {
                continue;
            };
            let name = name.trim();
            if !seen.insert(name.to_ascii_lowercase()) {
                continue;
            }
            let src = msys_bin.join(name);
            if src.is_file() {
                copy_file(&src, &dist.join(name))?;
                staged.push(name.to_string());
                pending.push(src);
            }
        }
    }

    if !staged.is_empty() {
        staged.sort();
        println!("staged MSYS2 runtime DLLs: {}", staged.join(", "));
    }
    Ok(())
}

fn copy_file(src: &Path, dst: &Path) -> Result<(), String> {
    fs::copy(src, dst)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {} failed: {e}", src.display(), dst.display()))
}
