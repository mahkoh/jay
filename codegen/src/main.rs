#![expect(clippy::from_str_radix_10)]

use {
    crate::phf::PhfHash,
    anyhow::Result,
    permutation::Permutation,
    std::{fmt::Debug, io, path::PathBuf},
};

macro_rules! define_w {
    ($w:ident) => {
        define_w!($w, $);
    };
    ($w:ident, $dol:tt) => {
        #[allow(unused_macros)]
        macro_rules! w {
            ($dol($arg:tt)*) => {
                write!($w, $dol($arg)*)?;
            };
        }
        macro_rules! wl {
            ($dol($arg:tt)*) => {
                writeln!($w, $dol($arg)*)?;
            };
        }
    };
}

mod input_event_codes;
mod keysyms;
#[path = "../../toml-config/src/phf.rs"]
mod phf;
mod phf_generator;

fn main() -> Result<()> {
    input_event_codes::main()?;
    keysyms::main()?;
    Ok(())
}

fn generate_map(
    name: &str,
    key_type: &str,
    value_type: &str,
    keys: &[impl PhfHash],
    values: &mut [impl Debug],
) -> Result<String> {
    use std::fmt::Write;
    let state = phf_generator::generate_hash(keys);
    Permutation::oneline(state.map).apply_inv_slice_in_place(values);
    let mut res = String::new();
    define_w!(res);
    wl!("pub(super) static {name}: PhfMap<{key_type}, {value_type}> = PhfMap {{");
    wl!("    key: {},", state.key);
    wl!("    disps: &[");
    for disp in state.disps {
        wl!("        {disp:?},");
    }
    wl!("    ],");
    wl!("    map: &[");
    for value in values {
        wl!("        {value:?},");
    }
    wl!("    ],");
    wl!("    _phantom: core::marker::PhantomData,");
    wl!("}};");
    Ok(res)
}

fn update(file: &str, data: &str) -> io::Result<()> {
    let mut path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
    path.push(file);
    if let Ok(current) = std::fs::read_to_string(&file)
        && current == data
    {
        return Ok(());
    }
    std::fs::write(&file, data)
}
