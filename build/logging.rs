use {
    crate::open,
    std::{fmt::Write as _, io::Write as _, process::Command},
};

pub fn main() -> anyhow::Result<()> {
    create_bridge()?;
    create_version()?;
    Ok(())
}

fn create_bridge() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=src/bridge.c");
    cc::Build::new().file("src/bridge.c").compile("bridge");
    Ok(())
}

fn create_version() -> anyhow::Result<()> {
    let mut version_string = env!("CARGO_PKG_VERSION").to_string();
    if let Ok(output) = Command::new("git").arg("rev-parse").arg("HEAD").output()
        && output.status.success()
        && let Ok(commit) = std::str::from_utf8(&output.stdout)
    {
        write!(version_string, " ({})", commit.trim())?;
    }
    let mut f = open("version.rs")?;
    writeln!(f, "pub const VERSION: &str = \"{}\";", version_string)?;
    Ok(())
}
