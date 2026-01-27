use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=freearc_cpp_lib/");
    println!("cargo:rerun-if-changed=codec_staging/");
    println!("Build script starting...");

    // Get the project root directory
    let project_root = env::var("CARGO_MANIFEST_DIR").unwrap();
    let freearc_path = format!("{}/freearc_cpp_lib", project_root);
    let codec_staging_path = format!("{}/codec_staging", project_root);
    
    println!("FreeARC path: {}", freearc_path);
    println!("Codec staging path: {}", codec_staging_path);

    // Check if we have GCC-built codecs in the staging directory
    let use_gcc_built_codecs = Path::new(&codec_staging_path).exists()
        && fs::metadata(format!("{}/libfreearc.a", codec_staging_path)).is_ok();

    if use_gcc_built_codecs {
        println!("Using GCC-built codecs from staging directory");
        
        // Build only the FFI wrapper to link against the pre-built libraries
        let mut build = cc::Build::new();
        build
            .cpp(true)
            .warnings(false)
            .include(&freearc_path)
            .include(format!("{}/Compression", freearc_path))
            .include(format!("{}/Compression/LZMA2", freearc_path))
            .include(format!("{}/Compression/PPMD", freearc_path))
            .include(format!("{}/Compression/Tornado", freearc_path))
            .include(format!("{}/Compression/GRZip", freearc_path))
            .include(format!("{}/Compression/LZP", freearc_path))
            .include(format!("{}/Compression/Delta", freearc_path))
            .include(format!("{}/Compression/Dict", freearc_path))
            .include(format!("{}/Compression/MM", freearc_path))
            .include(format!("{}/Compression/REP", freearc_path))
            .include(format!("{}/Compression/4x4", freearc_path))
            .flag("-D_WIN32")
            .flag("-DWIN32")
            .flag("-DWIN32_LEAN_AND_MEAN")
            .flag("-DNOMINMAX")
            .flag("-DNDEBUG")
            .flag("-DWINVER=0x0601")
            .flag("-D_WIN32_WINNT=0x0601")
            .flag("-DNOVERSETCONDITIONMASK")
            .flag("-D__USE_MINGW_ANSI_STDIO=0");

        // The wrapper is already included in the combined library
        // Just link against the pre-built GCC library
        println!("cargo:rustc-link-search=native={}", codec_staging_path);
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
            println!("cargo:rustc-link-lib=static={}", lib);
        }
    } else {
        println!("No GCC-built codecs found in staging directory");
        println!("Please run build_codecs.bat first to build the codecs with GCC");
        panic!("GCC-built codecs not found. Run build_codecs.bat first.");
    }

    // Link system libraries that FreeARC needs
    println!("cargo:rustc-link-lib=advapi32");
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=kernel32");
    println!("cargo:rustc-link-lib=bcrypt");
    
    // Link MinGW C runtime for __mingw_fprintf and other MinGW-specific functions
    println!("cargo:rustc-link-lib=dylib=msvcrt");
    
    // Link C++ standard library for exception handling and RTTI
    println!("cargo:rustc-link-lib=dylib=stdc++");
    
    // Ensure C++ exception handling symbols are available
}
