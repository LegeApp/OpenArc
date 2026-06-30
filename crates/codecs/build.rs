use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=OPENARC_BPG_X265");
    println!("cargo:rerun-if-env-changed=BPG_X265_SINGLE_THREAD");
    println!("cargo:rerun-if-env-changed=BPG_X265_PARAMS");

    // The bpg-rs feature is a development backend and intentionally skips all
    // libbpg/x265 C/C++ compilation even when Cargo's additive default features
    // also leave bpg-c enabled.
    if env::var_os("CARGO_FEATURE_BPG_RS").is_some() {
        return;
    }

    println!("cargo:rerun-if-changed=../../native/BPG/libbpg-0.9.8");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.join("..").join("..");
    let bpg_dir = workspace_root.join("native").join("BPG").join("libbpg-0.9.8");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    let x265 = prepare_x265(&bpg_dir, &target_os);
    compile_bpg(&bpg_dir, &x265);
    link_bpg_deps(&target_os, &x265);
}

#[derive(Debug, Clone)]
struct X265Build {
    source_dir: Option<PathBuf>,
    generated_include_dir: Option<PathBuf>,
    lib_dir: Option<PathBuf>,
    vendored_multilib: bool,
}

fn prepare_x265(bpg_dir: &Path, target_os: &str) -> X265Build {
    let mode = env::var("OPENARC_BPG_X265").unwrap_or_else(|_| "vendored".to_string());
    if mode.eq_ignore_ascii_case("system") {
        return X265Build {
            source_dir: None,
            generated_include_dir: None,
            lib_dir: None,
            vendored_multilib: false,
        };
    }

    let x265_dir = bpg_dir.join("x265");
    let source_dir = x265_dir.join("source");
    if !source_dir.join("x265.h").exists() {
        panic!(
            "vendored x265 4.1 source not found at {}. Set OPENARC_BPG_X265=system to use pkg-config x265 instead.",
            source_dir.display()
        );
    }

    let out_root = PathBuf::from(env::var("OUT_DIR").unwrap()).join("x265-4.1");
    let build8 = out_root.join("8bit");
    let build10 = out_root.join("10bit");
    let build12 = out_root.join("12bit");
    let lib8 = build8.join(static_lib_name("x265", target_os));
    let lib10_alias = build8.join(static_lib_name("x265_main10", target_os));
    let lib12_alias = build8.join(static_lib_name("x265_main12", target_os));

    if !lib8.exists() || !lib10_alias.exists() || !lib12_alias.exists() {
        fs::create_dir_all(&build8).unwrap();
        fs::create_dir_all(&build10).unwrap();
        fs::create_dir_all(&build12).unwrap();

        configure_x265(
            &build12,
            &source_dir,
            &[
                "-DHIGH_BIT_DEPTH=ON",
                "-DEXPORT_C_API=OFF",
                "-DENABLE_SHARED=OFF",
                "-DENABLE_CLI=OFF",
                "-DMAIN12=ON",
            ],
        );
        build_x265(&build12);

        configure_x265(
            &build10,
            &source_dir,
            &[
                "-DHIGH_BIT_DEPTH=ON",
                "-DEXPORT_C_API=OFF",
                "-DENABLE_SHARED=OFF",
                "-DENABLE_CLI=OFF",
                "-DMAIN10=ON",
            ],
        );
        build_x265(&build10);

        copy_or_symlink(&build10.join(static_lib_name("x265", target_os)), &lib10_alias);
        copy_or_symlink(&build12.join(static_lib_name("x265", target_os)), &lib12_alias);

        configure_x265(
            &build8,
            &source_dir,
            &[
                "-DEXTRA_LIB=x265_main10.a;x265_main12.a",
                "-DEXTRA_LINK_FLAGS=-L.",
                "-DLINKED_10BIT=ON",
                "-DLINKED_12BIT=ON",
                "-DENABLE_SHARED=OFF",
                "-DENABLE_CLI=OFF",
            ],
        );
        build_x265(&build8);
    }

    X265Build {
        source_dir: Some(source_dir),
        generated_include_dir: Some(build8),
        lib_dir: Some(out_root.join("8bit")),
        vendored_multilib: true,
    }
}

fn static_lib_name(name: &str, target_os: &str) -> String {
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        format!("{name}.lib")
    } else {
        format!("lib{name}.a")
    }
}

fn configure_x265(build_dir: &Path, source_dir: &Path, opts: &[&str]) {
    if build_dir.join("CMakeCache.txt").exists() {
        return;
    }
    let mut cmd = Command::new("cmake");
    cmd.current_dir(build_dir)
        .arg(source_dir)
        .arg("-DCMAKE_BUILD_TYPE=Release");
    for opt in opts {
        cmd.arg(opt);
    }
    run(&mut cmd, "configuring vendored x265 4.1");
}

fn build_x265(build_dir: &Path) {
    let jobs = env::var("NUM_JOBS").unwrap_or_else(|_| "1".to_string());
    let mut cmd = Command::new("cmake");
    cmd.arg("--build")
        .arg(build_dir)
        .arg("--config")
        .arg("Release")
        .arg("--parallel")
        .arg(jobs);
    run(&mut cmd, "building vendored x265 4.1");
}

fn copy_or_symlink(src: &Path, dst: &Path) {
    let _ = fs::remove_file(dst);
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        if symlink(src, dst).is_ok() {
            return;
        }
    }
    fs::copy(src, dst).unwrap_or_else(|err| {
        panic!("failed to copy {} to {}: {err}", src.display(), dst.display())
    });
}

fn run(cmd: &mut Command, what: &str) {
    let output = cmd.output().unwrap_or_else(|err| {
        panic!("failed to run command while {what}: {err}");
    });
    if !output.status.success() {
        panic!(
            "command failed while {what}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn compile_bpg(bpg_dir: &Path, x265: &X265Build) {
    if !bpg_dir.exists() {
        panic!("BPG source not found at {}", bpg_dir.display());
    }

    let version = fs::read_to_string(bpg_dir.join("VERSION"))
        .unwrap_or_else(|_| "0.9.8".to_string())
        .trim()
        .to_string();
    let version_define = format!("\"{version}\"");

    let mut enc = cc::Build::new();
    enc.warnings(false)
        .extra_warnings(false)
        .opt_level(3)
        .flag_if_supported("-std=c99")
        .flag_if_supported("-fdata-sections")
        .flag_if_supported("-ffunction-sections")
        .define("_FILE_OFFSET_BITS", "64")
        .define("_LARGEFILE_SOURCE", None)
        .define("_GNU_SOURCE", "1")
        .define("_REENTRANT", None)
        .define("CONFIG_BPG_VERSION", Some(version_define.as_str()))
        .define("USE_X265", None)
        .define("BPG_ENCODER_LIB", None)
        .include(bpg_dir)
        .file(bpg_dir.join("bpgenc.c"))
        .file(bpg_dir.join("x265_glue.c"))
        .file(bpg_dir.join("bpg_api.c"));

    if let Some(dir) = &x265.source_dir {
        enc.include(dir);
    }
    if let Some(dir) = &x265.generated_include_dir {
        enc.include(dir);
    }
    if x265.source_dir.is_none() {
        add_pkg_config_cflags(&mut enc, &["x265"]);
    }
    enc.compile("bpg_encoder");

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
    let libavutil_files = ["mem.c", "buffer.c", "log2_tab.c", "frame.c", "pixdesc.c", "md5.c"];

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
}

fn link_bpg_deps(target_os: &str, x265: &X265Build) {
    if let Some(dir) = &x265.lib_dir {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }

    if x265.vendored_multilib {
        println!("cargo:rustc-link-lib=static=x265");
        println!("cargo:rustc-link-lib=static=x265_main10");
        println!("cargo:rustc-link-lib=static=x265_main12");
    } else {
        add_pkg_config_link_libs(&["x265"]);
    }

    add_pkg_config_link_libs(&["libpng", "libjpeg", "lcms2"]);
    println!("cargo:rustc-link-lib=z");
    if target_os != "windows" {
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=rt");
        println!("cargo:rustc-link-lib=numa");
    } else {
        println!("cargo:rustc-link-lib=static=stdc++");
        println!("cargo:rustc-link-lib=ws2_32");
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=kernel32");
    }
}

fn add_pkg_config_cflags(build: &mut cc::Build, packages: &[&str]) {
    let Ok(output) = Command::new("pkg-config").arg("--cflags").args(packages).output() else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let flags = String::from_utf8_lossy(&output.stdout);
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
        .unwrap_or_else(|err| panic!("pkg-config failed for {packages:?}: {err}"));
    if !output.status.success() {
        panic!(
            "pkg-config could not find {packages:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let flags = String::from_utf8_lossy(&output.stdout);
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

#[allow(dead_code)]
fn run_with_stdin(cmd: &mut Command, stdin: &[u8], what: &str) {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("failed to spawn command while {what}: {err}"));
    child.stdin.as_mut().unwrap().write_all(stdin).unwrap();
    let output = child.wait_with_output().unwrap();
    if !output.status.success() {
        panic!(
            "command failed while {what}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
