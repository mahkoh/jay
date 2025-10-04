include!("../../../build/vulkan/hash.rs");

pub const BIN: &str = "src/gfx_apis/vulkan/shaders_bin";

pub fn update_hash() -> anyhow::Result<()> {
    std::fs::write(HASH, calculate_hash()?)?;
    Ok(())
}
