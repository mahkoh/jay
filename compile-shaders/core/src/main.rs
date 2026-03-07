use compile_shaders_core::{update_hash, TREES};

fn main() -> anyhow::Result<()> {
    for tree in TREES {
        update_hash(tree)?;
    }
    Ok(())
}
