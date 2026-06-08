use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=freearc_cpp_lib");
    println!("cargo:rerun-if-changed=vendor/libbsc");

    let target_os_libbsc =
        env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_string());
    let target_env_libbsc = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let project_root_libbsc =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let libbsc_root = project_root_libbsc.join("vendor").join("libbsc");
    if libbsc_root.exists() && env::var_os("CARGO_FEATURE_BSC").is_some() {
        build_libbsc(&libbsc_root, &target_os_libbsc, &target_env_libbsc);
    }

    if env::var_os("CARGO_FEATURE_FFI_CODECS").is_none() {
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_string());
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let project_root =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let freearc_path = project_root.join("freearc_cpp_lib");

    if !freearc_path.exists() {
        panic!("FreeArc C++ source not found at {}", freearc_path.display());
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

fn build_libbsc(root: &Path, target_os: &str, target_env: &str) {
    let mut build = cc::Build::new();
    build.cpp(true).warnings(false);
    build.include(root);

    let cpp_sources = [
        root.join("adler32/adler32.cpp"),
        root.join("bwt/bwt.cpp"),
        root.join("coder/coder.cpp"),
        root.join("coder/qlfc/qlfc.cpp"),
        root.join("coder/qlfc/qlfc_model.cpp"),
        root.join("filters/detectors.cpp"),
        root.join("filters/preprocessing.cpp"),
        root.join("libbsc/libbsc.cpp"),
        root.join("lzp/lzp.cpp"),
        root.join("platform/platform.cpp"),
        root.join("st/st.cpp"),
    ];
    for file in &cpp_sources {
        println!("cargo:rerun-if-changed={}", file.display());
        build.file(file);
    }

    // CUDA and OpenMP disabled by leaving LIBBSC_OPENMP and LIBBSC_CUDA_SUPPORT
    // *undefined*.  libbsc uses `#if defined(LIBBSC_OPENMP)` guards — defining
    // the macros as "0" still counts as "defined" and triggers an `#error`
    // in bwt.cpp that requires LIBSAIS_OPENMP to also be defined.

    match (target_os, target_env) {
        ("windows", "msvc") => {
            build.flag_if_supported("/std:c++17");
            build.flag_if_supported("/EHsc");
        }
        ("windows", _) => {
            build.flag_if_supported("-std=c++17");
            build.opt_level(3);
        }
        _ => {
            build.flag_if_supported("-std=c++17");
            build.flag_if_supported("-fPIC");
            build.opt_level(3);
        }
    }

    build.compile("bsc_static");

    // platform.cpp calls OpenProcessToken / AdjustTokenPrivileges on Windows.
    if target_os == "windows" {
        println!("cargo:rustc-link-lib=advapi32");
        if target_env == "gnu" {
            println!("cargo:rustc-link-arg=-static-libstdc++");
            println!("cargo:rustc-link-arg=-static-libgcc");
        }
    }

    let libsais = root.join("bwt/libsais/libsais.c");
    println!("cargo:rerun-if-changed={}", libsais.display());
    let mut c_build = cc::Build::new();
    c_build.warnings(false).file(&libsais);
    // Leave LIBSAIS_OPENMP undefined (same reason as LIBBSC_OPENMP above).
    match (target_os, target_env) {
        ("windows", "msvc") => {}
        _ => {
            c_build.opt_level(3);
            c_build.flag_if_supported("-fPIC");
        }
    }
    c_build.compile("bsc_sais");
}

fn link_system_libs(target_os: &str, target_env: &str) {
    match target_os {
        "windows" => {
            println!("cargo:rustc-link-lib=advapi32");
            println!("cargo:rustc-link-lib=user32");
            println!("cargo:rustc-link-lib=kernel32");
            println!("cargo:rustc-link-lib=bcrypt");
            if target_env == "gnu" {
                // -static-libstdc++ tells the GCC driver to find libstdc++.a from its
                // own internal directory (not the MSYS2 lib search path, where it doesn't live).
                // -static-libgcc does the same for libgcc_eh / libgcc.
                // The transitional FreeArc C++ bridge currently pulls 7-Zip's
                // ThreadsWin32.c through more than one translation unit. Keep
                // linking tests/builds while this bridge is being replaced.
                println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
                println!("cargo:rustc-link-arg=-static-libstdc++");
                println!("cargo:rustc-link-arg=-static-libgcc");
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
