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
    clippy::redundant_clone,
    clippy::collapsible_if,
    clippy::match_like_matches_macro,
    clippy::collapsible_else_if,
    clippy::identity_op,
    clippy::module_inception,
    clippy::single_char_pattern,
    clippy::too_many_arguments,
    clippy::from_over_into,
    clippy::single_match,
    clippy::upper_case_acronyms,
    clippy::manual_map,
    clippy::type_complexity,
    clippy::option_map_unit_fn,
    clippy::wrong_self_convention,
    clippy::single_char_add_str,
    clippy::ptr_eq
)]

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
mod edid;
mod event_loop;
mod fixed;
mod forker;
mod format;
mod globals;
mod ifs;
mod libinput;
mod logger;
mod logind;
mod object;
mod pango;
mod rect;
mod render;
mod sighand;
mod state;
mod tasks;
mod text;
mod theme;
mod time;
mod tools;
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
    cli::main();
}
