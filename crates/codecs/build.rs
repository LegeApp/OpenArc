use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // Link native codec dependencies per target OS.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let libs_dir = manifest_dir
        .join("..")
        .join("..")
        .join("native")
        .join("libs");
    let linux_libs_dir = libs_dir.join("linux");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    if target_os == "windows" {
        println!("cargo:rustc-link-search=native={}", libs_dir.display());

        if target_env == "gnu" {
            println!(
                "cargo:rerun-if-changed={}",
                libs_dir.join("libpng.a").display()
            );
            println!(
                "cargo:rerun-if-changed={}",
                libs_dir.join("libjpeg.a").display()
            );
            println!(
                "cargo:rerun-if-changed={}",
                libs_dir.join("libz.a").display()
            );
            // Add MSYS2 mingw64 library path first so system libs resolve against a
            // consistent MinGW toolchain.
            if let Ok(msys2_path) = env::var("MSYS2_PATH") {
                println!("cargo:rustc-link-search=native={}/mingw64/lib", msys2_path);
            } else if std::path::Path::new("C:/msys64/mingw64/lib").exists() {
                println!("cargo:rustc-link-search=native=C:/msys64/mingw64/lib");
            }

            link_fixed_windows_gnu_bpg(&manifest_dir, &libs_dir, &out_dir);
            println!("cargo:rustc-link-lib=x265"); // Required by in-memory BPG encoder
            println!("cargo:rustc-link-lib=png");
            println!("cargo:rustc-link-lib=jpeg");
            println!("cargo:rustc-link-lib=z");
            println!("cargo:rustc-link-lib=stdc++");
            println!("cargo:rustc-link-lib=gcc");
            println!("cargo:rustc-link-lib=winpthread");
            println!("cargo:rustc-link-lib=gomp");
            // MinGW's static archive resolution is single-pass; emit the BPG archives twice so
            // bpg_api.o and the library-mode bpgenc.o can resolve against each other.
            println!("cargo:rustc-link-lib=static=bpg_native_nomain");
        } else {
            println!(
                "cargo:rerun-if-changed={}",
                libs_dir.join("png.lib").display()
            );
            println!(
                "cargo:rerun-if-changed={}",
                libs_dir.join("jpeg.lib").display()
            );
            println!(
                "cargo:rerun-if-changed={}",
                libs_dir.join("z.lib").display()
            );
            println!("cargo:rustc-link-lib=static=png");
            println!("cargo:rustc-link-lib=static=jpeg");
            println!("cargo:rustc-link-lib=static=z");
        }
    } else if target_os == "linux" {
        // On Linux prefer system packages to avoid accidental linkage against
        // Windows archives in workspace libs/.
        let linux_bpg_archive = linux_libs_dir.join("libbpg_native.a");
        println!("cargo:rerun-if-changed={}", linux_bpg_archive.display());
        println!("cargo:rustc-link-search=native={}", linux_libs_dir.display());
        println!("cargo:rustc-link-lib=static=bpg_native");
        println!("cargo:rustc-link-lib=x265");
        println!("cargo:rustc-link-lib=png");
        println!("cargo:rustc-link-lib=jpeg");
        println!("cargo:rustc-link-lib=z");
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=gomp");
    }

    // HEIC/HEIF decoding now uses pure Rust heic-decoder crate - no FFI needed
}

fn link_fixed_windows_gnu_bpg(manifest_dir: &Path, libs_dir: &Path, out_dir: &Path) {
    let bpg_dir = manifest_dir
        .join("..")
        .join("..")
        .join("native")
        .join("BPG")
        .join("libbpg-0.9.8");
    let version = fs::read_to_string(bpg_dir.join("VERSION"))
        .unwrap_or_else(|_| "0.9.8".to_string())
        .trim()
        .to_string();
    let version_define = format!("\"{version}\"");
    let fixed_archive = out_dir.join("libbpg_native_nomain.a");
    let original_archive = libs_dir.join("libbpg_native.a");

    println!("cargo:rerun-if-changed={}", original_archive.display());
    println!("cargo:rerun-if-changed={}", bpg_dir.join("bpgenc.c").display());
    println!("cargo:rerun-if-changed={}", bpg_dir.join("VERSION").display());

    fs::copy(&original_archive, &fixed_archive).expect("copy libbpg_native.a");
    run(Command::new(archiver()).arg("d").arg(&fixed_archive).arg("bpgenc.o"));

    let mut build = cc::Build::new();
    build
        .cargo_metadata(false)
        .file(bpg_dir.join("bpgenc.c"))
        .include(&bpg_dir)
        .define("_FILE_OFFSET_BITS", Some("64"))
        .define("_LARGEFILE_SOURCE", None)
        .define("_REENTRANT", None)
        .define("CONFIG_BPG_VERSION", Some(version_define.as_str()))
        .define("USE_X265", None)
        .define("BPG_ENCODER_LIB", None)
        .flag_if_supported("-std=c99")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-unused-function");

    if Path::new("C:/msys64/mingw64/include").exists() {
        build.include("C:/msys64/mingw64/include");
    }

    build.compile("bpgenc_lib_fix");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=bpg_native_nomain");
    println!("cargo:rustc-link-lib=static=bpgenc_lib_fix");
}

fn archiver() -> &'static str {
    env::var("AR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| Box::leak(value.into_boxed_str()) as &'static str)
        .unwrap_or("ar")
}

fn run(command: &mut Command) {
    let status = command.status().expect("run native build helper");
    assert!(status.success(), "command failed: {:?}", command);
}
