// OpenArc Root Build Script
// Handles codec dependencies and GUI build

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=libs/");
    println!("cargo:rerun-if-changed=arcmax/codec_staging/");
    println!("cargo:rerun-if-changed=DocBrakeGUI/");
    println!("cargo:rerun-if-changed=openarc-ffi/src/lib.rs");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    
    // Ensure codec libraries are available
    check_codec_dependencies(&manifest_dir);
    
    // Build GUI components in release mode
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    if profile == "release" {
        build_gui_components(&manifest_dir);
    }
}

fn check_codec_dependencies(manifest_dir: &str) {
    let libs_dir = PathBuf::from(manifest_dir).join("libs");
    let codec_staging = PathBuf::from(manifest_dir).join("arcmax").join("codec_staging");
    
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
        println!("cargo:warning=Run 'build_codecs.bat' or 'make -C arcmax/codec_staging' first");
    }
}

fn build_gui_components(manifest_dir: &str) {
    use std::process::Command;
    
    // Build DocBrakeGUI if directory exists
    let gui_dir = PathBuf::from(manifest_dir).join("DocBrakeGUI");
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

