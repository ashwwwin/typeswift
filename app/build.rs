use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Build Swift library
    let swift_dir = PathBuf::from("VoicySwift");
    
    println!("cargo:rerun-if-changed=VoicySwift/Sources");
    println!("cargo:rerun-if-changed=VoicySwift/Package.swift");
    
    // Build the Swift package
    let output = Command::new("swift")
        .current_dir(&swift_dir)
        .args(&["build", "-c", "release", "--product", "TypeswiftSwift"])
        .output()
        .expect("Failed to build Swift library");
    
    if !output.status.success() {
        panic!(
            "Swift build failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    
    // Get the build directory
    let build_dir = swift_dir.join(".build/release");
    
    // Link the Swift library
    println!("cargo:rustc-link-search=native={}", build_dir.display());
    println!("cargo:rustc-link-lib=dylib=TypeswiftSwift");
    
    // Link Swift runtime and system frameworks
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=CoreML");
    println!("cargo:rustc-link-lib=framework=Accelerate");
    println!("cargo:rustc-link-lib=framework=ApplicationServices");
    
    // Set rpath for finding the dylib at runtime
    if cfg!(target_os = "macos") {
        // Where we expect to stage the Swift dylib inside the app bundle
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
        // Also include local build folder rpaths for dev runs
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../VoicySwift/.build/release");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", build_dir.display());
    }
}
