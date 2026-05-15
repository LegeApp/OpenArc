use std::env;
use std::fs;
use std::path::PathBuf;
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
