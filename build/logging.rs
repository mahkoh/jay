pub fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=src/bridge.c");
    cc::Build::new().file("src/bridge.c").compile("bridge");
    Ok(())
}
