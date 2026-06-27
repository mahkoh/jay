use generated::KEYCODES;

#[rustfmt::skip]
mod generated;

struct MappedKey<'a> {
    name: &'a str,
    value: u32,
}

pub fn input_event_code_from_name(name: &str) -> Option<u32> {
    let name = name.to_ascii_uppercase();
    let v = &KEYCODES[&*name];
    (v.name == name).then_some(v.value)
}
