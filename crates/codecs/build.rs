use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=ffmpeg_wrapper.c");
    println!("cargo:rerun-if-changed=../../native/BPG/libbpg-0.9.8");
    println!("cargo:rerun-if-env-changed=MSYS2_ROOT");
    println!("cargo:rerun-if-env-changed=MSYS2_PATH");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.join("..").join("..");
    let bpg_dir = workspace_root.join("native").join("BPG").join("libbpg-0.9.8");
    let wrapper_c = manifest_dir.join("ffmpeg_wrapper.c");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "windows" && target_env == "msvc" {
        // MSVC builds still use the prebuilt openarc_bpg.dll runtime-loaded path in
        // crates/codecs/bpg.rs. FFmpeg also uses the ffmpeg executable under MSVC,
        // because MSYS2 headers/static archives are not compatible with cl/link.exe.
        return;
    }

    compile_bpg(&bpg_dir);
    compile_ffmpeg_wrapper(&wrapper_c);
    link_codecs_and_ffmpeg(&target_os);
}

fn compile_bpg(bpg_dir: &Path) {
    if !bpg_dir.exists() {
        panic!("BPG source not found at {}", bpg_dir.display());
    }

    let libavcodec_files = [
        "hevc_cabac.c",
        "hevc_filter.c",
        "hevc.c",
        "hevcpred.c",
        "hevc_refs.c",
        "hevcdsp.c",
        "hevc_mvs.c",
        "hevc_ps.c",
        "hevc_sei.c",
        "utils.c",
        "cabac.c",
        "golomb.c",
        "videodsp.c",
    ];
    let libavutil_files = [
        "mem.c",
        "buffer.c",
        "log2_tab.c",
        "frame.c",
        "pixdesc.c",
        "md5.c",
    ];

    let version = std::fs::read_to_string(bpg_dir.join("VERSION"))
        .unwrap_or_else(|_| "0.9.8".to_string())
        .trim()
        .to_string();
    let version_define = format!("\"{version}\"");

    // Decoder portion: libavcodec/* + libavutil/* + libbpg.c.
    // These need -DHAVE_AV_CONFIG_H -DUSE_VAR_BIT_DEPTH -DUSE_PRED + c99.
    let mut dec = cc::Build::new();
    dec.warnings(false)
        .extra_warnings(false)
        .opt_level_str("s")
        .flag_if_supported("-std=c99")
        .flag_if_supported("-fno-asynchronous-unwind-tables")
        .flag_if_supported("-fdata-sections")
        .flag_if_supported("-ffunction-sections")
        .flag_if_supported("-fomit-frame-pointer")
        .define("_FILE_OFFSET_BITS", "64")
        .define("_LARGEFILE_SOURCE", None)
        .define("_REENTRANT", None)
        .define("CONFIG_BPG_VERSION", Some(version_define.as_str()))
        .define("_ISOC99_SOURCE", None)
        .define("_POSIX_C_SOURCE", "200112")
        .define("_XOPEN_SOURCE", "600")
        .define("HAVE_AV_CONFIG_H", None)
        .define("_GNU_SOURCE", "1")
        .define("USE_VAR_BIT_DEPTH", None)
        .define("USE_PRED", None)
        .include(bpg_dir);

    for f in &libavcodec_files {
        dec.file(bpg_dir.join("libavcodec").join(f));
    }
    for f in &libavutil_files {
        dec.file(bpg_dir.join("libavutil").join(f));
    }
    dec.file(bpg_dir.join("libbpg.c"));
    dec.compile("bpg_decoder");

    // Encoder portion: bpgenc.c (in library mode) + x265_glue.c + bpg_api.c.
    // Needs USE_X265 + BPG_ENCODER_LIB. Includes MSYS2 mingw64/include for x265.h.
    let mut enc = cc::Build::new();
    enc.warnings(false)
        .extra_warnings(false)
        .opt_level_str("s")
        .flag_if_supported("-std=c99")
        .define("_FILE_OFFSET_BITS", "64")
        .define("_LARGEFILE_SOURCE", None)
        .define("_REENTRANT", None)
        .define("CONFIG_BPG_VERSION", Some(version_define.as_str()))
        .define("USE_X265", None)
        .define("BPG_ENCODER_LIB", None)
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-unused-function")
        .include(bpg_dir);

    if let Some(libdir) = find_msys2_lib_dir() {
        let incdir = libdir
            .parent()
            .expect("mingw64/lib parent")
            .join("include");
        if incdir.exists() {
            enc.include(&incdir);
        }
    }

    enc.file(bpg_dir.join("bpgenc.c"));
    enc.file(bpg_dir.join("x265_glue.c"));
    enc.file(bpg_dir.join("bpg_api.c"));
    enc.compile("bpg_encoder");
}

fn compile_ffmpeg_wrapper(wrapper_c: &Path) {
    if !wrapper_c.exists() {
        panic!("ffmpeg_wrapper.c not found at {}", wrapper_c.display());
    }

    let mut build = cc::Build::new();
    build
        .warnings(false)
        .opt_level(2)
        .flag_if_supported("-std=c11")
        .file(wrapper_c);

    if let Some(libdir) = find_msys2_lib_dir() {
        let incdir = libdir
            .parent()
            .expect("mingw64/lib parent")
            .join("include");
        if incdir.exists() {
            build.include(&incdir);
        }
    }

    build.compile("openarc_ffmpeg_wrapper");
}

fn link_codecs_and_ffmpeg(target_os: &str) {
    let libdir = find_msys2_lib_dir().unwrap_or_else(|| {
        panic!(
            "{}",
            r"MSYS2 mingw64 libraries not found.
Install MSYS2 (https://www.msys2.org/) and run:

  pacman -S --needed mingw-w64-x86_64-gcc mingw-w64-x86_64-x265 \
    mingw-w64-x86_64-libpng mingw-w64-x86_64-libjpeg-turbo \
    mingw-w64-x86_64-libraw mingw-w64-x86_64-lcms2 \
    mingw-w64-x86_64-zlib mingw-w64-x86_64-ffmpeg \
    mingw-w64-x86_64-x264

Or set MSYS2_ROOT to a custom install path (e.g. D:\msys64)."
        )
    });

    require_static_libs(
        &libdir,
        &[
            "libavformat.a",
            "libavcodec.a",
            "libavutil.a",
            "libswscale.a",
            "libswresample.a",
            "libx265.a",
            "libx264.a",
        ],
        "FFmpeg or x264/x265",
    );

    println!("cargo:rustc-link-search=native={}", libdir.display());

    // Image / codec deps used by BPG and ffmpeg_wrapper.c.
    // Order matters for MinGW static linker resolution; FFmpeg first, then its
    // codec backends, then transitive system deps.
    let static_libs = [
        // BPG and image format deps
        "raw",
        "png",
        "jpeg",
        "lcms2",
        // FFmpeg muxers/demuxers/codecs (high level first)
        "avformat",
        "avcodec",
        "avfilter",
        "swresample",
        "swscale",
        "avutil",
        // Codec backends
        "x265",
        "x264",
        // FFmpeg transitive deps commonly required on Windows MSYS2
        "z",
        "bz2",
        "iconv",
    ];
    for lib in static_libs {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    match target_os {
        "windows" => {
            // C++ runtime + MinGW runtime, statically linked so the exe is portable.
            println!("cargo:rustc-link-arg=-static-libgcc");
            println!("cargo:rustc-link-arg=-static-libstdc++");
            println!("cargo:rustc-link-arg=-Wl,-Bstatic");
            println!("cargo:rustc-link-arg=-lstdc++");
            println!("cargo:rustc-link-arg=-lwinpthread");
            println!("cargo:rustc-link-arg=-lgomp");
            println!("cargo:rustc-link-arg=-Wl,-Bdynamic");
            // System DLLs that must remain dynamic.
            for sys in [
                "ole32",
                "user32",
                "ws2_32",
                "secur32",
                "ncrypt",
                "crypt32",
                "bcrypt",
                "advapi32",
                "kernel32",
                "mfplat",
                "mfuuid",
                "strmiids",
            ] {
                println!("cargo:rustc-link-lib={sys}");
            }
        }
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=pthread");
            println!("cargo:rustc-link-lib=dylib=m");
            println!("cargo:rustc-link-lib=dylib=dl");
            println!("cargo:rustc-link-lib=dylib=gomp");
        }
        _ => {}
    }
}

fn require_static_libs(libdir: &Path, libs: &[&str], context: &str) {
    let mut missing = Vec::new();
    for lib in libs {
        if !libdir.join(lib).exists() {
            missing.push(*lib);
        }
    }
    if !missing.is_empty() {
        panic!(
            "Required static libraries are missing from {}: {:?}.
{} static linking is required. Install the static-enabled MSYS2 packages, e.g.:

  pacman -S --needed mingw-w64-x86_64-ffmpeg mingw-w64-x86_64-x264 \\
    mingw-w64-x86_64-x265

If your MSYS2 only ships dynamic libs, the package needs to be rebuilt with static support.",
            libdir.display(),
            missing,
            context
        );
    }
}

fn find_msys2_lib_dir() -> Option<PathBuf> {
    let try_root = |root: &Path| -> Option<PathBuf> {
        let lib = root.join("mingw64").join("lib");
        if lib.join("libx265.a").exists()
            || lib.join("libx265.dll.a").exists()
            || lib.join("x265.lib").exists()
        {
            Some(lib)
        } else {
            None
        }
    };

    for env_var in ["MSYS2_ROOT", "MSYS2_PATH"] {
        if let Ok(p) = env::var(env_var) {
            if let Some(lib) = try_root(Path::new(&p)) {
                return Some(lib);
            }
        }
    }

    for candidate in [
        r"C:\msys64",
        r"C:\msys2",
        r"D:\msys64",
        r"D:\msys2",
        r"E:\msys64",
    ] {
        if let Some(lib) = try_root(Path::new(candidate)) {
            return Some(lib);
        }
    }

    if let Ok(output) = Command::new("where").arg("gcc.exe").output() {
        if output.status.success() {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let p = PathBuf::from(line.trim());
                let mut anc = p.as_path();
                while let Some(parent) = anc.parent() {
                    if parent.file_name().map(|f| f == "mingw64").unwrap_or(false) {
                        let lib = parent.join("lib");
                        if lib.exists() {
                            return Some(lib);
                        }
                    }
                    anc = parent;
                }
            }
        }
    }

    None
}
