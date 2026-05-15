use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=freearc_cpp_lib");
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_string());
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let project_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let freearc_path = project_root.join("freearc_cpp_lib");

    if !freearc_path.exists() {
        panic!(
            "FreeArc C++ source not found at {}",
            freearc_path.display()
        );
    }

    build_freearc(&freearc_path, &target_os, &target_env);
    link_system_libs(&target_os, &target_env);
}

fn build_freearc(freearc_path: &Path, target_os: &str, target_env: &str) {
    let compression = freearc_path.join("Compression");
    let mut build = cc::Build::new();
    build.cpp(true).warnings(false);

    for include in include_dirs(freearc_path) {
        build.include(include);
    }

    let sources = [
        compression.join("Common.cpp"),
        compression.join("CompressionLibrary.cpp"),
        compression.join("CELS.cpp"),
        compression.join("MultiThreading.cpp"),
        compression.join("LZMA2").join("C_LZMA.cpp"),
        compression.join("PPMD").join("C_PPMD.cpp"),
        compression.join("Tornado").join("C_Tornado.cpp"),
        compression.join("GRZip").join("C_GRZip.cpp"),
        compression.join("LZP").join("C_LZP.cpp"),
        compression.join("Delta").join("C_Delta.cpp"),
        compression.join("Dict").join("C_Dict.cpp"),
        compression.join("MM").join("C_MM.cpp"),
        compression.join("REP").join("C_REP.cpp"),
        compression.join("4x4").join("C_4x4.cpp"),
        freearc_path.join("freearc_wrapper.cpp"),
    ];

    for file in &sources {
        println!("cargo:rerun-if-changed={}", file.display());
        build.file(file);
    }

    match (target_os, target_env) {
        ("windows", "msvc") => {
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
        }
        ("windows", _) => {
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
            build.flag_if_supported("-std=c++11");
            build.opt_level(2);
        }
        _ => {
            build.define("NDEBUG", None);
            build.define("_FILE_OFFSET_BITS", Some("64"));
            build.define("_LARGEFILE_SOURCE", None);
            build.define("_REENTRANT", None);
            build.flag_if_supported("-std=c++11");
            build.flag_if_supported("-fPIC");
            build.opt_level(2);
        }
    }

    build.compile("freearc_native");
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
                println!("cargo:rustc-link-lib=static=stdc++");
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
