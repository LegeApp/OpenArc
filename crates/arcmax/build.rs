use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=freearc_cpp_lib/");
    println!("cargo:rerun-if-changed=codec_staging/");
    println!("cargo:rerun-if-changed=codec_staging/linux/");
    println!("cargo:rerun-if-changed=codec_staging/windows-gnu/");
    println!("cargo:rerun-if-changed=codec_staging/windows-msvc/");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_string());
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let project_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let freearc_path = project_root.join("freearc_cpp_lib");
    let codec_staging_path = codec_staging_dir(&project_root, &target_os, &target_env);

    if target_os == "windows" && target_env == "msvc" {
        build_windows_msvc(&freearc_path);
    } else {
        link_staged_codecs(&target_os, &freearc_path, &codec_staging_path);
    }

    link_system_libs(&target_os, &target_env);
}

fn build_windows_msvc(freearc_path: &Path) {
    let compression_path = freearc_path.join("Compression");
    let mut build = cc::Build::new();
    build.cpp(true).warnings(false);

    for include in include_dirs(freearc_path) {
        build.include(include);
    }

    for file in [
        compression_path.join("Common.cpp"),
        compression_path.join("CompressionLibrary.cpp"),
        compression_path.join("CELS.cpp"),
        compression_path.join("MultiThreading.cpp"),
        compression_path.join("LZMA2").join("C_LZMA.cpp"),
        compression_path.join("PPMD").join("C_PPMD.cpp"),
        compression_path.join("Tornado").join("C_Tornado.cpp"),
        compression_path.join("GRZip").join("C_GRZip.cpp"),
        compression_path.join("LZP").join("C_LZP.cpp"),
        compression_path.join("Delta").join("C_Delta.cpp"),
        compression_path.join("Dict").join("C_Dict.cpp"),
        compression_path.join("MM").join("C_MM.cpp"),
        compression_path.join("REP").join("C_REP.cpp"),
        compression_path.join("4x4").join("C_4x4.cpp"),
        freearc_path.join("freearc_wrapper.cpp"),
    ] {
        println!("cargo:rerun-if-changed={}", file.display());
        build.file(file);
    }

    for define in [
        "_WIN32",
        "WIN32",
        "WIN32_LEAN_AND_MEAN",
        "NOMINMAX",
        "NDEBUG",
        "NOVERSETCONDITIONMASK",
    ] {
        build.define(define, None);
    }
    build.define("WINVER", Some("0x0601"));
    build.define("_WIN32_WINNT", Some("0x0601"));
    build.define("strcasecmp", Some("_stricmp"));
    build.define("strncasecmp", Some("_strnicmp"));
    build.flag_if_supported("/std:c++14");
    build.flag_if_supported("/EHsc");
    build.flag_if_supported("/Zc:__cplusplus");
    build.cpp_link_stdlib(None);
    build.compile("freearc_native");
}

fn link_staged_codecs(target_os: &str, freearc_path: &Path, codec_staging_path: &Path) {
    let use_staged_codecs =
        codec_staging_path.exists() && fs::metadata(codec_staging_path.join("libfreearc.a")).is_ok();

    if !use_staged_codecs {
        let hint = if target_os == "windows" {
            "Run arcmax/build_codecs.ps1 first."
        } else {
            "Run arcmax/build_codecs.sh first."
        };
        panic!("GCC-built codecs not found in codec_staging (missing libfreearc.a). {hint}");
    }

    let mut build = cc::Build::new();
    build.cpp(true).warnings(false);
    for include in include_dirs(freearc_path) {
        build.include(include);
    }

    if target_os == "windows" {
        for define in [
            "_WIN32",
            "WIN32",
            "WIN32_LEAN_AND_MEAN",
            "NOMINMAX",
            "NDEBUG",
            "NOVERSETCONDITIONMASK",
        ] {
            build.define(define, None);
        }
        build.define("WINVER", Some("0x0601"));
        build.define("_WIN32_WINNT", Some("0x0601"));
        build.define("__USE_MINGW_ANSI_STDIO", Some("0"));
    }

    println!("cargo:rustc-link-search=native={}", codec_staging_path.display());
    for lib in [
        "freearc",
        "lzma2",
        "ppmd",
        "tornado",
        "grzip",
        "lzp",
        "delta",
        "dict",
        "mm",
        "rep",
        "4x4",
    ] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
}

fn codec_staging_dir(project_root: &Path, target_os: &str, target_env: &str) -> PathBuf {
    let base = project_root.join("codec_staging");
    let target_specific = match (target_os, target_env) {
        ("linux", _) => base.join("linux"),
        ("windows", "gnu") => base.join("windows-gnu"),
        ("windows", "msvc") => base.join("windows-msvc"),
        ("macos", _) => base.join("macos"),
        _ => base.join(target_os),
    };

    if target_specific.join("libfreearc.a").exists() {
        target_specific
    } else {
        base
    }
}

fn include_dirs(freearc_path: &Path) -> Vec<PathBuf> {
    let compression = freearc_path.join("Compression");
    vec![
        freearc_path.to_path_buf(),
        compression.clone(),
        compression.join("LZMA2"),
        compression.join("PPMD"),
        compression.join("Tornado"),
        compression.join("GRZip"),
        compression.join("LZP"),
        compression.join("Delta"),
        compression.join("Dict"),
        compression.join("MM"),
        compression.join("REP"),
        compression.join("4x4"),
    ]
}

fn link_system_libs(target_os: &str, target_env: &str) {
    match target_os {
        "windows" => {
            println!("cargo:rustc-link-lib=advapi32");
            println!("cargo:rustc-link-lib=user32");
            println!("cargo:rustc-link-lib=kernel32");
            println!("cargo:rustc-link-lib=bcrypt");
            if target_env == "gnu" {
                println!("cargo:rustc-link-lib=dylib=msvcrt");
                println!("cargo:rustc-link-lib=dylib=stdc++");
            }
        }
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=pthread");
            println!("cargo:rustc-link-lib=dylib=m");
            println!("cargo:rustc-link-lib=dylib=dl");
        }
        "macos" => {
            println!("cargo:rustc-link-lib=dylib=c++");
        }
        _ => {}
    }
}
