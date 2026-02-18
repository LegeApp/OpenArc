use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=freearc_cpp_lib/");
    println!("cargo:rerun-if-changed=codec_staging/");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_string());

    let project_root = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let freearc_path = format!("{project_root}/freearc_cpp_lib");
    let codec_staging_path = format!("{project_root}/codec_staging");

    // Use prebuilt/staged codecs created by build_codecs.sh / build_codecs.ps1.
    let use_staged_codecs = Path::new(&codec_staging_path).exists()
        && fs::metadata(format!("{codec_staging_path}/libfreearc.a")).is_ok();

    if !use_staged_codecs {
        let hint = if target_os == "windows" {
            "Run arcmax/build_codecs.ps1 first."
        } else {
            "Run arcmax/build_codecs.sh first."
        };
        panic!("GCC-built codecs not found in codec_staging (missing libfreearc.a). {hint}");
    }

    // Keep include paths available if wrapper/object compilation is added later.
    let mut _build = cc::Build::new();
    _build
        .cpp(true)
        .warnings(false)
        .include(&freearc_path)
        .include(format!("{freearc_path}/Compression"))
        .include(format!("{freearc_path}/Compression/LZMA2"))
        .include(format!("{freearc_path}/Compression/PPMD"))
        .include(format!("{freearc_path}/Compression/Tornado"))
        .include(format!("{freearc_path}/Compression/GRZip"))
        .include(format!("{freearc_path}/Compression/LZP"))
        .include(format!("{freearc_path}/Compression/Delta"))
        .include(format!("{freearc_path}/Compression/Dict"))
        .include(format!("{freearc_path}/Compression/MM"))
        .include(format!("{freearc_path}/Compression/REP"))
        .include(format!("{freearc_path}/Compression/4x4"));

    if target_os == "windows" {
        _build
            .flag("-D_WIN32")
            .flag("-DWIN32")
            .flag("-DWIN32_LEAN_AND_MEAN")
            .flag("-DNOMINMAX")
            .flag("-DNDEBUG")
            .flag("-DWINVER=0x0601")
            .flag("-D_WIN32_WINNT=0x0601")
            .flag("-DNOVERSETCONDITIONMASK")
            .flag("-D__USE_MINGW_ANSI_STDIO=0");
    }

    println!("cargo:rustc-link-search=native={codec_staging_path}");
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

    // Platform-specific system libs.
    match target_os.as_str() {
        "windows" => {
            println!("cargo:rustc-link-lib=advapi32");
            println!("cargo:rustc-link-lib=user32");
            println!("cargo:rustc-link-lib=kernel32");
            println!("cargo:rustc-link-lib=bcrypt");
            println!("cargo:rustc-link-lib=dylib=msvcrt");
            println!("cargo:rustc-link-lib=dylib=stdc++");
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
