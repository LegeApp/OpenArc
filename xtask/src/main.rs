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
    if let Err(e) = stage_windows_runtime_files(&workspace_root, &dist) {
        eprintln!("xtask: failed to stage runtime files: {e}");
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

fn stage_windows_runtime_files(workspace_root: &Path, dist: &Path) -> Result<(), String> {
    if !cfg!(windows) {
        return Ok(());
    }

    let bpg_dir = workspace_root.join("native").join("BPG").join("libbpg-0.9.8");
    let bpg_dll = bpg_dir.join("openarc_bpg.dll");
    if !bpg_dll.exists() {
        let script = bpg_dir.join("build_openarc_combined_dll.ps1");
        if !script.exists() {
            return Err(format!("{} is missing", bpg_dll.display()));
        }

        let status = Command::new("pwsh")
            .current_dir(&bpg_dir)
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                "build_openarc_combined_dll.ps1",
                "-Jobs",
                "12",
            ])
            .status()
            .map_err(|e| format!("failed to start BPG DLL build: {e}"))?;
        if !status.success() {
            return Err(format!("BPG DLL build failed with status {status}"));
        }
    }

    copy_file(&bpg_dll, &dist.join("openarc_bpg.dll"))?;

    let msys_bin = env::var_os("MSYS2_ROOT")
        .map(PathBuf::from)
        .map(|root| root.join("mingw64").join("bin"))
        .unwrap_or_else(|| PathBuf::from(r"C:\msys64\mingw64\bin"));
    for name in [
        "libgcc_s_seh-1.dll",
        "libjpeg-8.dll",
        "libpng16-16.dll",
        "libstdc++-6.dll",
        "libwinpthread-1.dll",
        "libx265-215.dll",
        "zlib1.dll",
    ] {
        copy_file(&msys_bin.join(name), &dist.join(name))?;
    }

    Ok(())
}

fn copy_file(src: &Path, dst: &Path) -> Result<(), String> {
    fs::copy(src, dst)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {} failed: {e}", src.display(), dst.display()))
}
