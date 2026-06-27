use {crate::generate_map, anyhow::Result, indexmap::IndexMap, regex::Regex, std::fmt::Write};

const HEADER: &str = include_str!("input-event-codes.h");

pub fn main() -> Result<()> {
    let regex = Regex::new(
        r#"(?xm)
        ^\#define\s+
        (?<name>(KEY_|BTN_)\S+)\s+
        (?<value>\S+)
    "#,
    )?;
    let mut codes = IndexMap::new();
    for capture in regex.captures_iter(HEADER) {
        let name = capture.name("name").unwrap().as_str();
        let value = capture.name("value").unwrap().as_str();
        if matches!(name, "KEY_MIN_INTERESTING" | "KEY_MAX" | "KEY_CNT") {
            continue;
        }
        let value = if let Some(value) = value.strip_prefix("0x")
            && let Ok(value) = u32::from_str_radix(value, 16)
        {
            value
        } else if let Ok(value) = u32::from_str_radix(value, 10) {
            value
        } else if let Some(value) = codes.get(value) {
            *value
        } else {
            panic!("Could not parse {}", capture.get(0).unwrap().as_str());
        };
        if value == 0 {
            continue;
        }
        codes.insert(name, value);
    }
    {
        #[derive(Debug)]
        #[expect(dead_code)]
        struct MappedKey<'a> {
            name: &'a str,
            value: u32,
        }
        let mut keys = vec![];
        let mut values = vec![];
        for (name, value) in codes.iter() {
            let Some(name) = name.strip_prefix("KEY_") else {
                continue;
            };
            keys.push(name);
            values.push(MappedKey {
                name,
                value: *value,
            });
        }
        let map = generate_map("KEYCODES", "str", "MappedKey", &keys, &mut values)?;
        let mut out = String::new();
        writeln!(out, "use super::MappedKey;")?;
        writeln!(out, "use crate::phf_map::PhfMap;")?;
        writeln!(out)?;
        writeln!(out, "{}", map)?;
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../toml-config/src/config/keycodes/generated.rs",
            ),
            out,
        )?;
    }
    Ok(())
}
