#![expect(clippy::from_str_radix_10)]

use {crate::phf::PhfHash, anyhow::Result, permutation::Permutation, std::fmt::Debug};

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
    writeln!(
        res,
        "pub(super) static {name}: PhfMap<{key_type}, {value_type}> = PhfMap {{"
    )?;
    writeln!(res, "    key: {},", state.key)?;
    writeln!(res, "    disps: &[")?;
    for disp in state.disps {
        writeln!(res, "        {disp:?},")?;
    }
    writeln!(res, "    ],")?;
    writeln!(res, "    map: &[")?;
    for value in values {
        writeln!(res, "        {value:?},")?;
    }
    writeln!(res, "    ],")?;
    writeln!(res, "    _phantom: core::marker::PhantomData,")?;
    writeln!(res, "}};")?;
    Ok(res)
}
