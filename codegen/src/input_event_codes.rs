use {
    crate::{generate_map, update},
    anyhow::Result,
    indexmap::IndexMap,
    linearize::{Linearize, LinearizeExt},
    regex::Regex,
    std::{fmt, fmt::Write},
};

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
    let mut by_value = IndexMap::new();
    let mut max = 0;
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
        by_value.entry(value).or_insert_with(Vec::new).push(name);
        max = max.max(value);
    }
    #[derive(Linearize)]
    enum MappingType {
        Keycode,
        InputEventCode,
    }
    for ty in MappingType::variants() {
        #[derive(Debug)]
        #[expect(dead_code)]
        struct MappedKey<'a> {
            name: &'a str,
            value: u32,
        }
        let mut keys = vec![];
        let mut values = vec![];
        for (name, value) in codes.iter() {
            let name = match ty {
                MappingType::Keycode => match name.strip_prefix("KEY_") {
                    Some(n) => n,
                    _ => continue,
                },
                MappingType::InputEventCode => *name,
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
        let file = match ty {
            MappingType::Keycode => "toml-config/src/config/keycodes/generated.rs",
            MappingType::InputEventCode => "toml-config/src/config/input_event_codes/generated.rs",
        };
        update(file, &out)?;
    }
    {
        let mut out = String::new();
        writeln!(out, "pub const MAX_INPUT_EVENT_CODE: usize = {max};")?;
        writeln!(out)?;
        writeln!(
            out,
            "#[derive(Copy, Clone, Debug, Eq, PartialEq, linearize::Linearize)]"
        )?;
        writeln!(out, "#[expect(non_camel_case_types)]")?;
        writeln!(out, "pub enum InputEventCode {{")?;
        for names in by_value.values() {
            writeln!(out, "    {},", names[0])?;
        }
        writeln!(out, "}}")?;
        writeln!(out)?;
        writeln!(out, "impl InputEventCode {{")?;
        writeln!(out, "    pub fn raw(self) -> u32 {{")?;
        writeln!(out, "        match self {{")?;
        for (value, names) in by_value.iter() {
            writeln!(out, "            Self::{} => {value},", names[0])?;
        }
        writeln!(out, "        }}")?;
        writeln!(out, "    }}")?;
        writeln!(out)?;
        writeln!(out, "    pub fn from_raw(raw: u32) -> Option<Self> {{")?;
        writeln!(
            out,
            "        static MAP: [Option<InputEventCode>; {}] = [",
            max + 1
        )?;
        for i in 0..=max {
            if let Some(value) = by_value.get(&i) {
                writeln!(out, "            Some(InputEventCode::{}),", value[0])?;
            } else {
                writeln!(out, "            None,")?;
            }
        }
        writeln!(out, "        ];")?;
        writeln!(out, "        MAP.get(raw as usize).copied().flatten()")?;
        writeln!(out, "    }}")?;
        writeln!(out, "}}")?;
        writeln!(out)?;
        writeln!(
            out,
            "impl crate::utils::static_text::StaticText for InputEventCode {{"
        )?;
        writeln!(out, "    fn text(&self) -> &'static str {{")?;
        writeln!(out, "        match self {{")?;
        for names in by_value.values() {
            writeln!(
                out,
                "            Self::{} => \"{}\",",
                names[0],
                fmt::from_fn(|f| {
                    for (idx, name) in names.iter().enumerate() {
                        if idx > 0 {
                            f.write_str(",")?;
                        }
                        f.write_str(name)?;
                    }
                    Ok(())
                })
            )?;
        }
        writeln!(out, "        }}")?;
        writeln!(out, "    }}")?;
        writeln!(out, "}}")?;
        update("src/evdev/input_event_codes.rs", &out)?;
    }
    {
        let mut out = String::new();
        writeln!(out, "use super::InputEventCode;")?;
        writeln!(out)?;
        for (name, value) in &codes {
            writeln!(
                out,
                "pub const {name}: InputEventCode = InputEventCode({value});"
            )?;
        }
        update("jay-config/src/input/input_event_codes.rs", &out)?;
    }
    Ok(())
}
