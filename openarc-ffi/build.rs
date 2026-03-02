use cbindgen::Builder;
use std::env;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    // Generate C header file for FFI
    Builder::new()
        .with_crate(&crate_dir)
        .with_language(cbindgen::Language::C)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("openarc_ffi.h");

    println!("cargo:rerun-if-changed=src/lib.rs");

    // Build C# GUI when the gui feature is enabled
    #[cfg(feature = "gui")]
    build_csharp_gui(&crate_dir);
}

#[cfg(feature = "gui")]
fn build_csharp_gui(crate_dir: &str) {
    use std::path::PathBuf;
    use std::process::Command;

    let gui_dir = PathBuf::from(crate_dir).join("..").join("DocBrakeGUI");

    if !gui_dir.exists() {
        println!("cargo:warning=DocBrakeGUI directory not found at {:?}", gui_dir);
        return;
    }

    println!("cargo:rerun-if-changed=../DocBrakeGUI/DocBrakeGUI.csproj");

    // Determine build configuration based on Cargo profile
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let config = if profile == "release" { "Release" } else { "Debug" };

    println!("cargo:warning=Building C# GUI ({} configuration)...", config);

    // Build the C# project using dotnet
    let status = Command::new("dotnet")
        .arg("build")
        .arg("DocBrakeGUI.csproj")
        .arg("-c")
        .arg(config)
        .current_dir(&gui_dir)
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:warning=C# GUI build succeeded");

            // Copy the FFI DLL to the GUI output directory for convenience
            let target_dir = PathBuf::from(crate_dir)
                .join("..")
                .join("target")
                .join(&profile);
            let gui_output = gui_dir.join("..").join("Release");

            if target_dir.exists() && gui_output.exists() {
                let dll_name = if cfg!(windows) {
                    "openarc_ffi.dll"
                } else {
                    "libopenarc_ffi.so"
                };
                let src_dll = target_dir.join(dll_name);
                let dst_dll = gui_output.join(dll_name);

                if src_dll.exists() {
                    if let Err(e) = std::fs::copy(&src_dll, &dst_dll) {
                        println!("cargo:warning=Failed to copy FFI DLL: {}", e);
                    } else {
                        println!("cargo:warning=Copied {} to GUI output", dll_name);
                    }
                }
            }
        }
        Ok(s) => {
            println!("cargo:warning=C# GUI build failed with status: {}", s);
        }
        Err(e) => {
            println!("cargo:warning=Failed to run dotnet build: {}", e);
            println!("cargo:warning=Make sure .NET SDK is installed");
        }
    }
}
