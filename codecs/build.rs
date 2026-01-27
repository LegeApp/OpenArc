use std::env;
use std::path::PathBuf;

fn main() {
    // Link MSYS2 libraries
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let libs_dir = manifest_dir.join("..").join("libs");

    println!("cargo:rustc-link-search=native={}", libs_dir.display());

    // Add MSYS2 mingw64 library path for x265 and other system libs
    if let Ok(msys2_path) = env::var("MSYS2_PATH") {
        println!("cargo:rustc-link-search=native={}/mingw64/lib", msys2_path);
    } else if std::path::Path::new("C:/msys64/mingw64/lib").exists() {
        println!("cargo:rustc-link-search=native=C:/msys64/mingw64/lib");
    }

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

    // Link libheif for HEIC/HEIF decoding when feature is enabled
    #[cfg(feature = "heif")]
    {
        // Check for libheif in libs directory or system
        let heif_lib = libs_dir.join("libheif.a");
        if heif_lib.exists() {
            println!("cargo:rustc-link-lib=static=heif");
            // libheif dependencies
            println!("cargo:rustc-link-lib=de265");  // HEVC decoder
            println!("cargo:rustc-link-lib=x265");   // HEVC encoder (optional)
        } else {
            // Try dynamic linking from system
            println!("cargo:rustc-link-lib=heif");
        }
    }
}
