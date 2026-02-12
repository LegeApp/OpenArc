// Build script for BPG viewer library
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let _out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Determine the BPG library location
    // Priority:
    // 1. BPG_LIB_PATH environment variable
    // 2. ./libs/libbpg_native.a (Windows-compatible library)
    // 3. ../BPG/libbpg-0.9.8 (relative to project)
    // 4. System library paths

    let bpg_lib_path = if let Ok(path) = env::var("BPG_LIB_PATH") {
        PathBuf::from(path)
    } else {
        // Try local libs directory first (Windows-compatible library)
        let mut path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        path.push("libs");
        if path.exists() && path.join("libbpg_native.a").exists() {
            path
        } else {
            // Fall back to BPG directory
            let mut path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
            path.push("../BPG/libbpg-0.9.8");
            path
        }
    };

    if bpg_lib_path.exists() {
        println!("cargo:rustc-link-search=native={}", bpg_lib_path.display());

        // Check if x265 directory exists and add it to search path
        let x265_path = bpg_lib_path.join("x265");
        if x265_path.exists() {
            println!("cargo:rustc-link-search=native={}", x265_path.display());
        }
    }

    // Link against BPG library (libbpg_native on Windows, libbpg on Linux)
    let bpg_native_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("libs")
        .join("libbpg_native.a");

    if bpg_native_path.exists() {
        println!("cargo:rustc-link-lib=static=bpg_native");
    } else {
        println!("cargo:rustc-link-lib=static=bpg");
    }

    // Link against x265 (HEVC encoder used by BPG)
    let _x265_lib_found = if std::path::Path::new(&format!("{}/libx265.a", bpg_lib_path.display())).exists() ||
       std::path::Path::new(&format!("{}/libx265.lib", bpg_lib_path.display())).exists() {
        println!("cargo:rustc-link-lib=static=x265");
    } else {
        let x265_path = format!("{}/x265", bpg_lib_path.display());
        if std::path::Path::new(&format!("{}/libx265.a", x265_path)).exists() ||
           std::path::Path::new(&format!("{}/libx265.lib", x265_path)).exists() {
            println!("cargo:rustc-link-lib=static=x265");
        }
    };

    // Link against image format libraries
    println!("cargo:rustc-link-lib=png");
    println!("cargo:rustc-link-lib=jpeg");
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-lib=lcms2");

    // Platform-specific linking
    if cfg!(target_os = "windows") {
        // For Windows with GNU toolchain, link stdc++ for BPG
        // (since BPG was compiled as C++)
        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=gcc");
        println!("cargo:rustc-link-lib=dylib=winpthread");
        println!("cargo:rustc-link-lib=dylib=gomp");
    } else if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=m");
        println!("cargo:rustc-link-lib=dylib=pthread");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=dylib=c++");
    }

    // Rerun if BPG library changes
    if bpg_lib_path.exists() {
        println!("cargo:rerun-if-changed={}", bpg_lib_path.display());
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BPG_LIB_PATH");

    // Build GPU thumbnail shaders if the feature is enabled
    #[cfg(feature = "gpu-thumbnails")]
    {
        // Include the GPU thumbnail build logic
        let result = std::panic::catch_unwind(|| {
            #[cfg(target_os = "windows")]
            compile_hlsl_shaders();
        });
        
        if result.is_err() {
            eprintln!("Warning: GPU thumbnail build failed, but continuing without GPU support");
        }
    }
}

#[cfg(target_os = "windows")]
fn compile_hlsl_shaders() {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const SHADERS: &[(&str, &str)] = &[
        ("ycbcr_resize.hlsl", "ycbcr_resize.cso"),
    ];

    let shader_dir = Path::new("./gpu-shaders");
    println!("cargo:rerun-if-changed={}", shader_dir.display());
    for (src, _) in SHADERS {
        println!("cargo:rerun-if-changed={}", shader_dir.join(src).display());
    }

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
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
