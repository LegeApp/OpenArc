// Build script for BPG viewer library
use std::env;
use std::path::PathBuf;

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
}
