use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    #[cfg(target_os = "windows")]
    compile_hlsl_shaders();
}

#[cfg(target_os = "windows")]
fn compile_hlsl_shaders() {
    use std::fs;

    const SHADERS: &[(&str, &str)] = &[
        ("shaders/ycbcr_resize.hlsl", "ycbcr_resize.cso"),
    ];

    let shader_dir = Path::new("../gpu-shaders");  // Go up one level to bpg-viewer directory where shaders are located
    println!("cargo:rerun-if-changed={}", shader_dir.display());
    for (src, _) in SHADERS {
        println!("cargo:rerun-if-changed={}", shader_dir.join(src).display());
    }

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let shader_out_dir = PathBuf::from(&out_dir).join("hlsl-shaders");
    fs::create_dir_all(&shader_out_dir).unwrap();

    let dxc_path = find_dxc().expect(
        "dxc.exe not found. Install Windows SDK or add dxc.exe to PATH."
    );

    for (hlsl_file, cso_file) in SHADERS {
        let hlsl_path = shader_dir.join(hlsl_file);
        let cso_path = shader_out_dir.join(cso_file);

        println!("cargo:warning=Compiling shader: {} -> {}", hlsl_path.display(), cso_path.display());

        let output = Command::new(&dxc_path)
            .args([
                "-T", "cs_6_0",
                "-E", "main",
                "-O3",
                "-Fo", cso_path.to_str().unwrap(),
                hlsl_path.to_str().unwrap(),
            ])
            .output()
            .expect("Failed to execute dxc");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            panic!(
                "DXC failed for {}:\nstdout: {}\nstderr: {}",
                hlsl_file, stdout, stderr
            );
        }
    }

    // Generate shaders.rs include file with include_bytes! for each compiled .cso
    let mut include_content = String::from("// Auto-generated shader includes\n\n");
    for (const_name, cso_file) in [
        ("YCBCR_RESIZE_SHADER", "ycbcr_resize.cso"),
    ] {
        let cso_path = shader_out_dir.join(cso_file);
        let normalized = cso_path.to_str().unwrap().replace('\\', "/");
        include_content.push_str(&format!(
            "pub const {}: &[u8] = include_bytes!(r\"{}\");\n",
            const_name, normalized
        ));
    }

    let include_path = shader_out_dir.join("shaders.rs");
    fs::write(&include_path, &include_content).unwrap();
    println!("cargo:rustc-env=SHADER_INCLUDE_PATH={}", include_path.display());
}

#[cfg(target_os = "windows")]
fn find_dxc() -> Option<PathBuf> {
    const SDK_PATHS: &[&str] = &[
        r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\dxc.exe",
        r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\dxc.exe",
        r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.22000.0\x64\dxc.exe",
        r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.20348.0\x64\dxc.exe",
        r"C:\Program Files (x86)\Windows Kits\10\bin\x64\dxc.exe",
    ];
    for path in SDK_PATHS {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    // Fallback: locate via PATH
    if let Ok(output) = Command::new("where").arg("dxc").output() {
        if output.status.success() {
            if let Some(line) = String::from_utf8_lossy(&output.stdout).lines().next() {
                let p = PathBuf::from(line.trim());
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    None
}
