use {anyhow::bail, compile_shaders_core::ROOT, std::process::Command};

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed={}", ROOT);
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
