// OpenArc Comprehensive Build Script
// Builds codecs separately, then builds all workspace components

use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::fs;

fn main() {
    println!("=== OpenArc Comprehensive Build ===");
    
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    
    // Step 1: Build codecs if not already built
    if !check_codecs_built(&manifest_dir) {
        println!("Building codecs...");
        build_codecs(&manifest_dir);
    } else {
        println!("Codecs already built, skipping...");
    }
    
    // Step 2: Build all workspace components
    println!("Building workspace components...");
    build_workspace(&profile);
    
    // Step 3: Build GUI in release mode if needed
    if profile == "release" {
        println!("Building GUI components...");
        build_gui_components(&manifest_dir);
    }
    
    println!("=== Build Complete ===");
}

fn check_codecs_built(manifest_dir: &str) -> bool {
    let libs_dir = PathBuf::from(manifest_dir).join("libs");
    let codec_staging = PathBuf::from(manifest_dir).join("arcmax").join("codec_staging");
    
    // Check for essential codec libraries
    let essential_libs = vec![
        "libbpg_native.a",
        "libpng.a", 
        "libjpeg.a",
        "libz.a",
        "libraw.a"
    ];
    
    for lib in essential_libs {
        if !libs_dir.join(lib).exists() {
            return false;
        }
    }
    
    // Check for FreeArc staging libraries
    if !codec_staging.join("libfreearc.a").exists() {
        return false;
    }
    
    true
}

fn build_codecs(manifest_dir: &str) {
    let root = PathBuf::from(manifest_dir);
    
    // Build BPG encoder
    println!("  Building BPG encoder...");
    let status = Command::new("make")
        .arg("-C")
        .arg(root.join("BPG"))
        .arg("-j4")
        .status();
    
    match status {
        Ok(s) if s.success() => println!("    BPG encoder built successfully"),
        _ => println!("    Warning: BPG encoder build failed"),
    }
    
    // Build FreeArc codecs
    println!("  Building FreeArc codecs...");
    let status = Command::new("make")
        .arg("-C")
        .arg(root.join("arcmax").join("codec_staging"))
        .arg("-j4")
        .status();
    
    match status {
        Ok(s) if s.success() => println!("    FreeArc codecs built successfully"),
        _ => println!("    Warning: FreeArc codecs build failed"),
    }
    
    // Copy BPG library to libs
    let bpg_lib = root.join("BPG").join("libbpg.a");
    let target_lib = root.join("libs").join("libbpg_native.a");
    if bpg_lib.exists() {
        if let Err(e) = fs::copy(&bpg_lib, &target_lib) {
            println!("    Warning: Failed to copy BPG library: {}", e);
        }
    }
}

fn build_workspace(profile: &str) {
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    
    if profile == "release" {
        cmd.arg("--release");
    }
    
    cmd.arg("--workspace");
    cmd.arg("--exclude");
    cmd.arg("codecs"); // Exclude codecs from workspace build
    
    let status = cmd.status();
    
    match status {
        Ok(s) if s.success() => println!("  Workspace built successfully"),
        Ok(s) => {
            println!("  ERROR: Workspace build failed with status: {}", s);
            std::process::exit(1);
        }
        Err(e) => {
            println!("  ERROR: Failed to run cargo build: {}", e);
            std::process::exit(1);
        }
    }
}

fn build_gui_components(manifest_dir: &str) {
    let root = PathBuf::from(manifest_dir);
    
    // Build openarc-gui
    println!("  Building openarc-gui...");
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--package")
        .arg("openarc-gui")
        .status();
    
    match status {
        Ok(s) if s.success() => println!("    openarc-gui built successfully"),
        _ => println!("    Warning: openarc-gui build failed"),
    }
    
    // Build DocBrakeGUI if directory exists
    let gui_dir = root.join("DocBrakeGUI");
    if gui_dir.exists() {
        println!("  Building DocBrakeGUI...");
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
            .arg(root.join("Release"))
            .current_dir(&gui_dir)
            .status();
        
        match status {
            Ok(s) if s.success() => println!("    DocBrakeGUI built successfully"),
            _ => println!("    Warning: DocBrakeGUI build failed"),
        }
        
        // Copy FFI DLL
        copy_ffi_dll(manifest_dir);
    }
}

fn copy_ffi_dll(manifest_dir: &str) {
    let root = PathBuf::from(manifest_dir);
    let target_dir = root.join("target").join("release");
    let release_dir = root.join("Release");
    
    let dll_name = "openarc_ffi.dll";
    let src_dll = target_dir.join(dll_name);
    let dst_dll = release_dir.join(dll_name);
    
    if src_dll.exists() {
        if let Err(e) = fs::copy(&src_dll, &dst_dll) {
            println!("    Warning: Failed to copy FFI DLL: {}", e);
        } else {
            println!("    Copied {} to Release directory", dll_name);
        }
    }
}
