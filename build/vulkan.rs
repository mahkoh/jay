mod hash;

use {
    crate::vulkan::hash::{ROOT, unchanged},
    anyhow::bail,
    std::process::Command,
};

pub fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed={}", ROOT);
    if !std::fs::exists("compile-shaders")? {
        return Ok(());
    }
    if unchanged() {
        return Ok(());
    }
    let code = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "compile-shaders/Cargo.toml",
            "-p",
            "compile-shaders-compile",
        ])
        .status()?;
    if !code.success() {
        bail!("compile-shaders failed");
    }
    Ok(())
}
