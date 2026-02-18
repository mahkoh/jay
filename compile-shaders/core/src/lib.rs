include!("../../../build/vulkan/hash.rs");

pub fn update_hash(tree: &Tree) -> anyhow::Result<()> {
    std::fs::write(tree.hash, calculate_hash(tree)?)?;
    Ok(())
}
