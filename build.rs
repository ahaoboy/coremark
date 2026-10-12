//! Build script: captures the Rust compiler version, build profile and target
//! triple so the benchmark report can print them, mirroring the C
//! `COMPILER_VERSION` and `COMPILER_FLAGS` strings.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let version = Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "rustc (unknown)".to_string());

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=COREMARK_RUSTC_VERSION={version}");
    println!("cargo:rustc-env=COREMARK_PROFILE={profile}");
    println!("cargo:rustc-env=COREMARK_TARGET={target}");
}
