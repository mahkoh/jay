extern crate core;

use std::fs::{File, OpenOptions};
use std::io::BufWriter;
use std::path::PathBuf;
use std::{env, io};

mod enums;
mod tokens;
mod wire;
mod wire_dbus;

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
    enums::main()?;

    println!("cargo:rerun-if-changed=build/build.rs");
    Ok(())
}
