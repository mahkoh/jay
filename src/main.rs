#![feature(
    c_variadic,
    thread_local,
    label_break_value,
    try_blocks,
    generic_associated_types,
    extern_types
)]
#![allow(
    clippy::len_zero,
    clippy::needless_lifetimes,
    clippy::enum_variant_names,
    clippy::useless_format,
    clippy::redundant_clone
)]

use crate::cli::{Cli, Cmd};
use crate::compositor::start_compositor;
use clap::Parser;

#[macro_use]
mod macros;
#[macro_use]
mod leaks;
mod acceptor;
mod async_engine;
mod backend;
mod backends;
mod bugs;
mod cli;
mod client;
mod clientmem;
mod compositor;
mod config;
mod cursor;
mod dbus;
mod drm;
mod event_loop;
mod fixed;
mod forker;
mod format;
mod globals;
mod ifs;
mod libinput;
mod logind;
mod object;
mod pango;
mod pixman;
mod rect;
mod render;
mod sighand;
mod state;
mod tasks;
mod text;
mod theme;
mod time;
mod tree;
mod udev;
mod utils;
mod wheel;
mod wire;
mod wire_dbus;
mod wire_xcon;
mod xcon;
mod xkbcommon;
mod xwayland;

fn main() {
    let cli: Cli = Cli::parse();
    println!("{:?}", cli);
    match cli.command {
        Cmd::Run => start_compositor(),
        _ => {}
    }
}
