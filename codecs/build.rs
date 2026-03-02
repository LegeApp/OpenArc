use std::env;
use std::path::PathBuf;

fn main() {
    // Link MSYS2 libraries
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let libs_dir = manifest_dir.join("..").join("libs");

    // Add MSYS2 mingw64 library path first so system libs (png/jpeg/z/stdc++)
    // resolve against a consistent MinGW toolchain.
    if let Ok(msys2_path) = env::var("MSYS2_PATH") {
        println!("cargo:rustc-link-search=native={}/mingw64/lib", msys2_path);
    } else if std::path::Path::new("C:/msys64/mingw64/lib").exists() {
        println!("cargo:rustc-link-search=native=C:/msys64/mingw64/lib");
    }
    println!("cargo:rustc-link-search=native={}", libs_dir.display());

    // Link libraries
    println!("cargo:rustc-link-lib=raw");
    println!("cargo:rustc-link-lib=static=bpg_native");
    println!("cargo:rustc-link-lib=x265");  // Required by in-memory BPG encoder
    println!("cargo:rustc-link-lib=png");
    println!("cargo:rustc-link-lib=jpeg");
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-lib=lcms2");  // Required by libraw for color management
    println!("cargo:rustc-link-lib=stdc++");
    println!("cargo:rustc-link-lib=gcc");
    println!("cargo:rustc-link-lib=winpthread");
    println!("cargo:rustc-link-lib=gomp");

    // HEIC/HEIF decoding now uses pure Rust heic-decoder crate - no FFI needed
}
