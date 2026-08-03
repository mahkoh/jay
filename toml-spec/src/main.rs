use crate::json_schema::generate_json_schema;
use crate::markdown::generate_markdown;
use crate::types::Described;
use crate::types::TopLevelTypeSpec;
use anyhow::Context;
use anyhow::Result;
use indexmap::IndexMap;

mod json_schema;
mod markdown;
mod types;

fn parse() -> Result<IndexMap<String, Described<TopLevelTypeSpec>>> {
    let file = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/spec/spec.yaml"))?;
    Ok(serde_yaml::from_str(&file)?)
}

fn filter_subtree<'a>(
    types: &'a IndexMap<String, Described<TopLevelTypeSpec>>,
    name: &'a str,
) -> Result<Vec<(&'a str, &'a Described<TopLevelTypeSpec>)>> {
    let mut map = IndexMap::new();
    let mut todo = vec![];
    todo.push(name);
    while let Some(name) = todo.pop() {
        let ty = types
            .get(name)
            .with_context(|| format!("{} not found", name))?;
        if map.insert(name, ty).is_some() {
            continue;
        }
        ty.value.collect_refs(&mut todo);
    }
    let mut sorted: Vec<_> = map.into_iter().collect();
    sorted.sort_by_key(|t| t.0);
    Ok(sorted)
}

fn generate_subtree_json(
    types: &IndexMap<String, Described<TopLevelTypeSpec>>,
    file: &str,
    name: &str,
) -> Result<()> {
    let subtree = filter_subtree(types, name)?;
    generate_json_schema(&subtree, file, name)?;
    Ok(())
}

fn main() -> Result<()> {
    let types = parse()?;
    {
        const CONFIG: &str = "Config";
        let types = filter_subtree(&types, CONFIG)?;
        generate_markdown(&types)?;
        generate_json_schema(&types, "spec", CONFIG)?;
    }
    generate_subtree_json(&types, "client-match", "ClientMatch")?;
    generate_subtree_json(&types, "window-match", "WindowMatch")?;
    Ok(())
}
