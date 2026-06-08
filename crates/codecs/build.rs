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

    compile_bpg(&bpg_dir, &target_os);
    compile_ffmpeg_wrapper(&wrapper_c, &target_os);
    link_codecs_and_ffmpeg(&target_os);
}

fn compile_bpg(bpg_dir: &Path, target_os: &str) {
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
        .opt_level(3)
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
        .opt_level(3)
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

    if target_os == "windows" {
        if let Some(libdir) = find_msys2_lib_dir() {
            let incdir = libdir
                .parent()
                .expect("mingw64/lib parent")
                .join("include");
            if incdir.exists() {
                enc.include(&incdir);
            }
        }
    } else {
        add_pkg_config_includes(&mut enc, &["x265", "libpng", "libraw", "lcms2"]);
    }

    enc.file(bpg_dir.join("bpgenc.c"));
    enc.file(bpg_dir.join("x265_glue.c"));
    enc.file(bpg_dir.join("bpg_api.c"));
    enc.compile("bpg_encoder");
}

fn compile_ffmpeg_wrapper(wrapper_c: &Path, target_os: &str) {
    if !wrapper_c.exists() {
        panic!("ffmpeg_wrapper.c not found at {}", wrapper_c.display());
    }

    let mut build = cc::Build::new();
    build
        .warnings(false)
        .opt_level(3)
        .flag_if_supported("-std=c11")
        .file(wrapper_c);

    if target_os == "windows" {
        if let Some(libdir) = find_msys2_lib_dir() {
            let incdir = libdir
                .parent()
                .expect("mingw64/lib parent")
                .join("include");
            if incdir.exists() {
                build.include(&incdir);
            }
        }
    } else {
        add_pkg_config_includes(
            &mut build,
            &[
                "libavformat",
                "libavcodec",
                "libavutil",
                "libswscale",
                "libswresample",
            ],
        );
    }

    build.compile("openarc_ffmpeg_wrapper");
}

fn add_pkg_config_includes(build: &mut cc::Build, packages: &[&str]) {
    let output = Command::new("pkg-config")
        .arg("--cflags")
        .args(packages)
        .output();

    let Ok(output) = output else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let Ok(flags) = String::from_utf8(output.stdout) else {
        return;
    };

    for token in flags.split_whitespace() {
        if let Some(path) = token.strip_prefix("-I") {
            build.include(path);
        } else if token.starts_with("-D") {
            build.flag(token);
        }
    }
}

fn add_pkg_config_link_libs(packages: &[&str]) {
    let output = Command::new("pkg-config")
        .arg("--libs")
        .args(packages)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "pkg-config failed while probing Linux codec libraries: {err}.\n\
Install the Linux -dev packages listed in BUILDING.md."
            )
        });

    if !output.status.success() {
        panic!(
            "pkg-config could not find Linux codec libraries: {}\n\
Install the Linux -dev packages listed in BUILDING.md.",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let flags = String::from_utf8(output.stdout)
        .expect("pkg-config emitted non-UTF-8 linker flags");

    for token in flags.split_whitespace() {
        if let Some(path) = token.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={path}");
        } else if let Some(lib) = token.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={lib}");
        } else if token == "-pthread" {
            println!("cargo:rustc-link-lib=pthread");
        }
    }
}

fn link_codecs_and_ffmpeg(target_os: &str) {
    if target_os != "windows" {
        add_pkg_config_link_libs(&[
            "libavformat",
            "libavcodec",
            "libavutil",
            "libswscale",
            "libswresample",
            "x265",
            "libpng",
            "libraw",
            "lcms2",
        ]);
        println!("cargo:rustc-link-lib=jpeg");
        println!("cargo:rustc-link-lib=z");
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=gomp");
        return;
    }

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

    // Ask pkg-config for the *full* static link list that FFmpeg needs.
    // The MSYS2 FFmpeg is compiled with many optional features whose transitive
    // deps change with each package update.  A hand-maintained list can't keep up.
    let pkg_config_bin = libdir
        .parent()
        .expect("mingw64/lib parent")
        .join("bin")
        .join("pkg-config.exe");
    let pkgconfig_path = libdir.join("pkgconfig");

    let pkg_libs = if pkg_config_bin.exists() {
        // Some MSYS2 .pc files don't fully expand transitive private deps for
        // static linking (e.g. libavcodec lists -ljxl but jxl's own .pc isn't
        // recursively expanded, so -lhwy -lbrotli* are missing).  We work
        // around this by collecting the initial lib list and then recursively
        // querying each returned lib's .pc file as well.
        let libs = pkg_config_recursive(
            &pkg_config_bin,
            &pkgconfig_path,
            &[
                "libavformat", "libavcodec", "libavutil",
                "libswscale", "libswresample",
            ],
        );
        if libs.is_empty() { None } else { Some(libs) }
    } else {
        None
    };

    // Windows system DLLs — always dynamic, never try to link statically.
    let windows_sys = [
        "ws2_32", "ole32", "user32", "kernel32", "advapi32", "bcrypt",
        "secur32", "ncrypt", "crypt32", "mfplat", "mfuuid", "strmiids",
        "gdi32", "uuid", "winmm", "vfw32",
        // GCC internal libs that come via -static-libgcc/-static-libstdc++
        "gcc", "gcc_s", "gcc_eh",
    ];
    // Libs that MUST be linked as DLLs even though static .a files exist.
    // MSYS2's FFmpeg static archives were compiled with these as shared libs,
    // so their objects contain __imp_xxx references (DLL-import stubs) that
    // can only be satisfied by the DLL import library, not by libX.a.
    let force_dynamic = [
        // GLib/GObject/GIO/GdkPixbuf — always DLL on Windows in MSYS2 FFmpeg
        "glib-2.0", "gobject-2.0", "gio-2.0", "gdk_pixbuf-2.0",
        // Cairo — MSYS2 FFmpeg's RSVG decoder uses __imp_cairo_*
        "cairo",
        // librsvg — Rust-based, always a DLL on Windows (no stable ABI as static)
        "rsvg-2",
        // Pango — also part of the GLib/GTK stack, DLL on Windows
        "pango-1.0", "pangocairo-1.0",
        // HWY (Highway SIMD) — libjxl was compiled against the DLL version
        "hwy",
        // Vulkan shader compiler — DLL only (no static build in MSYS2)
        "shaderc_shared",
    ];
    // Libs handled separately (GCC driver flags -static-libgcc / -static-libstdc++).
    let runtime_libs = ["stdc++", "atomic"];
    // Libs that don't exist on Windows.
    let skip_on_windows = ["dl", "m", "pthread"];

    if target_os == "windows" {
        // -----------------------------------------------------------------------
        // All native static libraries are emitted as cargo:rustc-link-lib so
        // that Cargo places them in the correct position relative to rlibs in
        // the final linker command.  The ordering matters: dependencies must
        // come AFTER the archives that need them (GNU ld single-pass model).
        //
        // The pkg-config output for libavformat already lists libs in the
        // correct dependency order (avformat first, avutil last, xml2 before
        // lzma/z/iconv, etc.).  We honour that order here.
        // -----------------------------------------------------------------------

        // Collect the full ordered lib list from pkg-config, falling back to a
        // known-good list when pkg-config is unavailable.
        let all_libs: Vec<String> = if let Some(flags) = pkg_libs {
            flags.into_iter().filter_map(|token| {
                if let Some(libname) = token.strip_prefix("-l") {
                    if skip_on_windows.contains(&libname)
                        || windows_sys.contains(&libname)
                        || runtime_libs.contains(&libname)
                    {
                        return None;
                    }
                    Some(libname.to_string())
                } else {
                    None
                }
            }).collect()
        } else {
            eprintln!("cargo:warning=pkg-config not found; using fallback FFmpeg static lib list");
            [
                "raw", "png", "jpeg", "lcms2",
                "avformat", "avcodec", "swresample", "swscale", "avutil",
                "x265", "x264",
                "vpx", "aom", "dav1d", "opus", "mp3lame",
                "vorbis", "vorbisenc", "ogg", "xml2", "lzma", "bz2", "z", "iconv",
                "srt", "ssl", "crypto", "gnutls", "hogweed", "nettle", "gmp",
            ].iter().map(|s| s.to_string()).collect()
        };

        // Emit everything as cargo:rustc-link-lib so they land in the right
        // linker-command section (after rlibs, before link-args).
        // Dynamic-import libs use the DLL import library (.dll.a).
        let mut seen = std::collections::HashSet::new();

        // Always-needed BPG/codec deps that may not appear in pkg-config output.
        for lib in ["raw", "png", "jpeg", "lcms2"] {
            if seen.insert(lib.to_string()) {
                println!("cargo:rustc-link-lib=static={lib}");
            }
        }

        for libname in &all_libs {
            if !seen.insert(libname.clone()) {
                continue; // deduplicate
            }
            // Some libs were compiled against the DLL version by MSYS2 FFmpeg;
            // their objects use __imp_xxx references that only a DLL import
            // library can satisfy — static linking would leave them unresolved.
            if force_dynamic.contains(&libname.as_str()) {
                let dll_a = libdir.join(format!("lib{libname}.dll.a"));
                if dll_a.exists() {
                    println!("cargo:rustc-link-lib=dylib={libname}");
                }
                continue;
            }
            let static_a = libdir.join(format!("lib{libname}.a"));
            let dll_a    = libdir.join(format!("lib{libname}.dll.a"));
            if static_a.exists() {
                println!("cargo:rustc-link-lib=static={libname}");
            } else if dll_a.exists() {
                println!("cargo:rustc-link-lib=dylib={libname}");
            }
            // else: silently skip — windows_sys filtered above
        }

        // Ensure ssl/crypto (needed by libsrt) and gomp/winpthread (needed by
        // libraw / OpenMP code) are always linked, even if pkg-config misses them.
        for lib in ["ssl", "crypto", "gomp", "winpthread"] {
            if seen.insert(lib.to_string()) {
                let a = libdir.join(format!("lib{lib}.a"));
                if a.exists() {
                    println!("cargo:rustc-link-lib=static={lib}");
                }
            }
        }

        // GCC C++ runtime: use GCC driver flags so GCC finds libstdc++.a in its
        // internal directory (not the MSYS2 lib search path).
        println!("cargo:rustc-link-arg=-static-libgcc");
        println!("cargo:rustc-link-arg=-static-libstdc++");

        // Windows system DLLs (must remain dynamic).
        for sys in [
            "ole32", "user32", "ws2_32", "secur32", "ncrypt",
            "crypt32", "bcrypt", "advapi32", "kernel32",
            "mfplat", "mfuuid", "strmiids", "gdi32", "uuid",
        ] {
            println!("cargo:rustc-link-lib={sys}");
        }
    } else {
        // Non-Windows: use cargo:rustc-link-lib for everything.
        let skip_linux = ["dl", "m", "pthread"];
        for lib in ["raw", "png", "jpeg", "lcms2",
                    "avformat", "avcodec", "swresample", "swscale", "avutil",
                    "x265", "x264"] {
            println!("cargo:rustc-link-lib=static={lib}");
        }
        if let Some(flags) = pkg_libs {
            let mut seen = std::collections::HashSet::new();
            for token in flags {
                if let Some(libname) = token.strip_prefix("-l") {
                    if skip_linux.contains(&libname) || !seen.insert(libname.to_string()) {
                        continue;
                    }
                    println!("cargo:rustc-link-lib=static={libname}");
                }
            }
        }
        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=pthread");
        println!("cargo:rustc-link-lib=dylib=m");
        println!("cargo:rustc-link-lib=dylib=dl");
        println!("cargo:rustc-link-lib=dylib=gomp");
    }
}

/// Recursively expand pkg-config static link flags.
///
/// `pkg-config --static --libs` sometimes omits transitive private deps
/// (e.g. libavcodec lists -ljxl but the jxl package's own Requires.private
/// aren't expanded at that level).  We fix this by:
///   1. Running pkg-config for the requested packages.
///   2. For every -l<name> in the output, if lib<name>.pc exists, running
///      pkg-config on that package too and merging the results.
///
/// Returns a deduplicated, ordered list of -l / -L tokens.
fn pkg_config_recursive(pkg_config: &Path, pkg_path: &Path, packages: &[&str]) -> Vec<String> {
    let mut ordered: Vec<String> = Vec::new();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();

    fn run_one(
        pkg_config: &Path,
        pkg_path: &Path,
        pkg: &str,
        ordered: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
    ) {
        if !visited.insert(pkg.to_string()) {
            return;
        }
        let out = Command::new(pkg_config)
            .env("PKG_CONFIG_PATH", pkg_path.display().to_string())
            .args(["--static", "--libs", pkg])
            .output()
            .ok();
        let flags = match out.and_then(|o| if o.status.success() {
            String::from_utf8(o.stdout).ok()
        } else {
            None
        }) {
            Some(f) => f,
            None => return,
        };

        for token in flags.split_whitespace() {
            if let Some(libname) = token.strip_prefix("-l") {
                // Recurse into this lib's own .pc if it exists.
                // pkg-config files may be named either "lib{name}.pc" or "{name}.pc".
                let pc_with_lib = pkg_path.join(format!("lib{libname}.pc"));
                let pc_bare     = pkg_path.join(format!("{libname}.pc"));
                if pc_with_lib.exists() {
                    run_one(pkg_config, pkg_path, &format!("lib{libname}"), ordered, visited);
                } else if pc_bare.exists() {
                    run_one(pkg_config, pkg_path, libname, ordered, visited);
                }
                let tok = token.to_string();
                if !ordered.contains(&tok) {
                    ordered.push(tok);
                }
            } else if token.starts_with("-L") {
                let tok = token.to_string();
                if !ordered.contains(&tok) {
                    ordered.push(tok);
                }
            }
        }
    }

    for pkg in packages {
        run_one(pkg_config, pkg_path, pkg, &mut ordered, &mut visited);
    }
    ordered
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
