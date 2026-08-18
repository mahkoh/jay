#![allow(clippy::from_str_radix_10, clippy::match_like_matches_macro)]

use crate::phf::PhfHash;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use permutation::Permutation;
use rayon::prelude::*;
use std::fmt::Debug;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

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

#[macro_use]
#[expect(unused_macros)]
#[path = "../../src/macros.rs"]
mod macros;
mod gen_cm_paths;
mod gen_lut;
mod input_event_codes;
mod keysyms;
#[path = "../../toml-config/src/phf.rs"]
mod phf;
mod phf_generator;

fn main() -> Result<()> {
    let tasks: &[&(dyn Fn() -> Result<()> + Sync)] = &[
        &|| input_event_codes::main(),
        &|| keysyms::main(),
        &|| gen_cm_paths::main(),
        &|| gen_lut::main(),
    ];
    tasks.into_par_iter().try_for_each(|generator| generator())
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

fn get_absolute_path(relative: &str) -> PathBuf {
    let mut absolute = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
    absolute.push(relative);
    absolute
}

fn format_rust(path: &Path, raw: &str) -> Result<String> {
    let dir = path.parent().context("file path has no parent")?;
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::Builder::default()
        .suffix(".rs")
        .tempfile_in(dir)?;
    tmp.write_all(raw.as_bytes())?;
    let status = Command::new("rustfmt")
        .arg("+nightly")
        .arg("--edition=2024")
        .arg(tmp.path())
        .status()?;
    if !status.success() {
        tmp.disable_cleanup(true);
        bail!("rustfmt failed");
    }
    Ok(std::fs::read_to_string(&tmp)?)
}

fn update(relative: &str, raw: &str) -> Result<()> {
    update_(relative, raw, true)
}

fn update_noformat(relative: &str, raw: &str) -> Result<()> {
    update_(relative, raw, false)
}

fn update_(relative: &str, raw: &str, format: bool) -> Result<()> {
    let absolute = get_absolute_path(relative);

    let formatted;
    let mut out = raw;
    if format {
        formatted = format_rust(&absolute, out)?;
        out = &*formatted;
    }

    if let Ok(current) = std::fs::read_to_string(&absolute)
        && current == out
    {
        return Ok(());
    }
    std::fs::write(&absolute, out)?;

    Ok(())
}
