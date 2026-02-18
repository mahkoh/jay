mod hash;

use {
    crate::vulkan::hash::{TREES, Tree, unchanged},
    anyhow::bail,
    std::process::Command,
};

pub fn main() -> anyhow::Result<()> {
    for tree in TREES {
        main_(tree)?;
    }
    Ok(())
}

fn main_(tree: &Tree) -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed={}", tree.root);
    if !std::fs::exists("compile-shaders")? {
        return Ok(());
    }
    if unchanged(tree) {
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
