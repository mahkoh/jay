#![allow(
    clippy::len_zero,
    clippy::needless_lifetimes,
    clippy::enum_variant_names,
    clippy::useless_format,
    clippy::redundant_clone,
    clippy::collapsible_if,
    clippy::unnecessary_to_owned,
    clippy::match_like_matches_macro,
    clippy::too_many_arguments,
    clippy::iter_skip_next,
    clippy::uninlined_format_args,
    clippy::manual_is_ascii_check
)]

extern crate core;

use std::{
    env,
    fs::{File, OpenOptions},
    io::{self, BufWriter},
    path::PathBuf,
};

mod egl;
mod enums;
mod tokens;
mod vulkan;
mod wire;
mod wire_dbus;
mod wire_xcon;

fn open(s: &str) -> io::Result<BufWriter<File>> {
    let mut path = PathBuf::from(env::var("OUT_DIR").unwrap());
    path.push(s);
    Ok(BufWriter::new(
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?,
    ))
}

fn main() -> anyhow::Result<()> {
    wire::main()?;
    wire_dbus::main()?;
    wire_xcon::main()?;
    enums::main()?;
    egl::main()?;
    vulkan::main()?;

    println!("cargo:rerun-if-changed=build/build.rs");
    Ok(())
}
