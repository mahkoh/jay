use {
    crate::{
        json_schema::generate_json_schema,
        markdown::generate_markdown,
        types::{Described, TopLevelTypeSpec},
    },
    anyhow::Result,
    indexmap::IndexMap,
};

mod json_schema;
mod markdown;
mod types;

fn parse() -> Result<IndexMap<String, Described<TopLevelTypeSpec>>> {
    let file = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/spec/spec.yaml"))?;
    Ok(serde_yaml::from_str(&file)?)
}

fn main() -> Result<()> {
    let types = parse()?;
    let mut types_sorted: Vec<_> = types.iter().collect();
    types_sorted.sort_by_key(|t| t.0);
    generate_markdown(&types_sorted)?;
    generate_json_schema(&types_sorted)?;
    Ok(())
}
