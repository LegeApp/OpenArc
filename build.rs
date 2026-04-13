// OpenArc Root Build Script
// Handles codec dependencies and GUI build

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=native/libs/");
    println!("cargo:rerun-if-changed=crates/arcmax/codec_staging/");
    println!("cargo:rerun-if-changed=crates/arcmax/codec_staging/linux/");
    println!("cargo:rerun-if-changed=crates/arcmax/codec_staging/windows-gnu/");
    println!("cargo:rerun-if-changed=crates/arcmax/codec_staging/windows-msvc/");
    println!("cargo:rerun-if-changed=gui/DocBrakeGUI/");
    println!("cargo:rerun-if-changed=crates/openarc-ffi/src/lib.rs");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    
    // Ensure codec libraries are available
    check_codec_dependencies(&manifest_dir);

    // Ensure top-level binaries also pull in native BPG/ArcMax symbols.
    // These native links do not reliably propagate from dependent rlibs into
    // the final openarc binary across all toolchains.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    link_workspace_native_libs(&manifest_dir, &target_os, &target_env);
    
    // Build GUI components only on Windows release builds when explicitly requested.
    // This keeps CLI-only release builds fast and avoids unexpected dotnet invocations.
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let build_gui = env_flag("OPENARC_BUILD_GUI");
    if profile == "release" && target_os == "windows" && build_gui {
        build_gui_components(&manifest_dir);
    } else if profile == "release" && target_os == "windows" && !build_gui {
        println!(
            "cargo:warning=Skipping DocBrakeGUI publish during Rust build (set OPENARC_BUILD_GUI=1 to enable)"
        );
    }
}

fn link_workspace_native_libs(manifest_dir: &str, target_os: &str, target_env: &str) {
    if target_os != "windows" && target_os != "linux" {
        return;
    }

    let libs_dir = PathBuf::from(manifest_dir).join("native").join("libs");
    let linux_libs_dir = libs_dir.join("linux");
    let codec_staging_dir = PathBuf::from(manifest_dir)
        .join("crates")
        .join("arcmax");
    let codec_staging_dir = arcmax_codec_staging_dir(&codec_staging_dir, target_os, target_env);
    let bpg_archive = if target_os == "linux" {
        linux_libs_dir.join("libbpg_native.a")
    } else {
        libs_dir.join("libbpg_native.a")
    };
    let arcmax_archives = [
        "libfreearc.a",
        "liblzma2.a",
        "libppmd.a",
        "libtornado.a",
        "libgrzip.a",
        "liblzp.a",
        "libdelta.a",
        "libdict.a",
        "libmm.a",
        "librep.a",
        "lib4x4.a",
    ];
    if target_os == "linux" && linux_libs_dir.exists() {
        println!("cargo:rustc-link-search=native={}", linux_libs_dir.display());
    } else if !libs_dir.exists() {
        println!(
            "cargo:warning=Workspace libs directory not found: {}",
            libs_dir.display()
        );
    } else {
        println!("cargo:rustc-link-search=native={}", libs_dir.display());
    }

    if codec_staging_dir.exists() {
        println!("cargo:rustc-link-search=native={}", codec_staging_dir.display());
    } else {
        println!(
            "cargo:warning=Codec staging directory not found: {}",
            codec_staging_dir.display()
        );
    }

    println!("cargo:rustc-link-arg-bin=openarc=-Wl,--start-group");

    if bpg_archive.exists() {
        println!(
            "cargo:rustc-link-arg-bin=openarc={}",
            bpg_archive.display()
        );
    } else {
        println!(
            "cargo:warning=Missing BPG archive for CLI link: {}",
            bpg_archive.display()
        );
    }

    for archive in arcmax_archives {
        let path = codec_staging_dir.join(archive);
        if path.exists() {
            println!("cargo:rustc-link-arg-bin=openarc={}", path.display());
        } else {
            println!(
                "cargo:warning=Missing ArcMax archive for CLI link: {}",
                path.display()
            );
        }
    }

    match target_os {
        "windows" => {
            for lib in [
                "-lraw",
                "-lx265",
                "-lpng",
                "-ljpeg",
                "-lz",
                "-llcms2",
                "-lstdc++",
                "-lgcc",
                "-lwinpthread",
                "-lgomp",
                "-lmsvcrt",
                "-lws2_32",
            ] {
                println!("cargo:rustc-link-arg-bin=openarc={}", lib);
            }
        }
        "linux" => {
            for lib in [
                "-lraw",
                "-lx265",
                "-lpng",
                "-ljpeg",
                "-lz",
                "-llcms2",
                "-lstdc++",
                "-lpthread",
                "-lm",
                "-ldl",
                "-lgomp",
            ] {
                println!("cargo:rustc-link-arg-bin=openarc={}", lib);
            }
        }
        _ => {}
    }
    println!("cargo:rustc-link-arg-bin=openarc=-Wl,--end-group");
}

fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

fn check_codec_dependencies(manifest_dir: &str) {
    let libs_dir = PathBuf::from(manifest_dir).join("native").join("libs");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let arcmax_dir = PathBuf::from(manifest_dir)
        .join("crates")
        .join("arcmax");
    let codec_staging = arcmax_codec_staging_dir(&arcmax_dir, &target_os, &target_env);
    
    // Check if essential codec libraries exist
    let essential_libs = vec![
        "libbpg_native.a",
        "libpng.a", 
        "libjpeg.a",
        "libz.a",
        "libraw.a"
    ];
    
    let mut missing_libs = Vec::new();
    for lib in essential_libs {
        if !libs_dir.join(lib).exists() {
            missing_libs.push(lib.to_string());
        }
    }
    
    if !codec_staging.join("libfreearc.a").exists() {
        missing_libs.push("libfreearc.a".to_string());
    }
    
    if !missing_libs.is_empty() {
        println!("cargo:warning=Missing codec libraries: {:?}", missing_libs);
        println!(
            "cargo:warning=Run the target-specific codec build helper first (for Linux: 'bash crates/arcmax/build_codecs.sh')"
        );
    }
}

fn arcmax_codec_staging_dir(arcmax_dir: &Path, target_os: &str, target_env: &str) -> PathBuf {
    let base = arcmax_dir.join("codec_staging");
    let target_specific = match (target_os, target_env) {
        ("linux", _) => base.join("linux"),
        ("windows", "gnu") => base.join("windows-gnu"),
        ("windows", "msvc") => base.join("windows-msvc"),
        ("macos", _) => base.join("macos"),
        _ => base.join(target_os),
    };

    if target_specific.exists() {
        target_specific
    } else {
        base
    }
}

fn build_gui_components(manifest_dir: &str) {
    // Build DocBrakeGUI if directory exists
    let gui_dir = PathBuf::from(manifest_dir)
        .join("gui")
        .join("DocBrakeGUI");
    if gui_dir.exists() {
        println!("cargo:warning=Building DocBrakeGUI (Release configuration)...");

        let status = Command::new("dotnet")
            .arg("publish")
            .arg("DocBrakeGUI.csproj")
            .arg("-c")
            .arg("Release")
            .arg("-r")
            .arg("win-x64")
            .arg("--self-contained")
            .arg("true")
            .arg("-p:PublishSingleFile=true")
            .arg("-p:IncludeNativeLibrariesForSelfExtract=true")
            .arg("-o")
            .arg(PathBuf::from(manifest_dir).join("Release"))
            .current_dir(&gui_dir)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=DocBrakeGUI build succeeded");
                copy_ffi_dll(manifest_dir);
            }
            Ok(s) => {
                println!("cargo:warning=DocBrakeGUI build failed with status: {}", s);
            }
            Err(e) => {
                println!("cargo:warning=Failed to run dotnet publish: {}", e);
                println!("cargo:warning=Make sure .NET SDK 8.0+ is installed");
            }
        }
    }
}

fn copy_ffi_dll(manifest_dir: &str) {
    let target_dir = PathBuf::from(manifest_dir)
        .join("target")
        .join("release");
    let release_dir = PathBuf::from(manifest_dir).join("Release");

    // Copy openarc_ffi.dll
    let dll_name = "openarc_ffi.dll";
    let src_dll = target_dir.join(dll_name);
    let dst_dll = release_dir.join(dll_name);

    if src_dll.exists() {
        if let Err(e) = std::fs::copy(&src_dll, &dst_dll) {
            println!("cargo:warning=Failed to copy FFI DLL: {}", e);
        } else {
            println!("cargo:warning=Copied {} to Release directory", dll_name);
        }
    } else {
        println!("cargo:warning=FFI DLL not found at {:?} - build openarc-ffi first", src_dll);
    }

    // Copy bpg_viewer.dll
    let bpg_dll_name = "bpg_viewer.dll";
    let src_bpg_dll = target_dir.join(bpg_dll_name);
    let dst_bpg_dll = release_dir.join(bpg_dll_name);

    if src_bpg_dll.exists() {
        if let Err(e) = std::fs::copy(&src_bpg_dll, &dst_bpg_dll) {
            println!("cargo:warning=Failed to copy BPG viewer DLL: {}", e);
        } else {
            println!("cargo:warning=Copied {} to Release directory", bpg_dll_name);
        }
    } else {
        println!("cargo:warning=BPG viewer DLL not found at {:?} - build bpg-viewer first", src_bpg_dll);
    }
}

