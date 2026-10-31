use std::process::Command;
use std::env;

fn main() {
    // Tell Cargo to rerun this build script if vnc_module source changes
    println!("cargo:rerun-if-changed=../vnc_module/src");
    println!("cargo:rerun-if-changed=../vnc_module/Cargo.toml");
    
    
    // Get the current directory
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let workspace_root = current_dir.parent().unwrap();
    
    // Build vnc_module
    let vnc_module_path = workspace_root.join("vnc_module");
    let output = Command::new("cargo")
        .args(&["build", "--release"])
        .current_dir(&vnc_module_path)
        .output()
        .expect("Failed to execute cargo build for vnc_module");
    
    if !output.status.success() {
        panic!("Failed to build vnc_module: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    println!("Successfully built vnc_module");
    
}