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
    clippy::ptr_eq,
    clippy::zero_prefixed_literal,
    clippy::unnecessary_unwrap,
    clippy::needless_return,
    clippy::missing_safety_doc,
    clippy::collapsible_if,
    clippy::mut_from_ref,
    clippy::bool_comparison,
    clippy::collapsible_match,
    clippy::field_reassign_with_default,
    clippy::new_ret_no_self,
    clippy::or_fun_call,
    clippy::uninlined_format_args,
    clippy::manual_is_ascii_check,
    clippy::needless_borrow,
    clippy::unnecessary_cast,
    clippy::manual_flatten
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
mod cursor_user;
mod damage;
mod dbus;
mod drm_feedback;
mod edid;
mod fixed;
mod forker;
mod format;
mod gfx_api;
mod gfx_apis;
mod globals;
mod ifs;
mod io_uring;
#[cfg(feature = "it")]
mod it;
mod libinput;
mod logger;
mod logind;
mod object;
mod output_schedule;
mod pango;
mod pipewire;
mod portal;
mod rect;
mod renderer;
mod scale;
mod screenshoter;
mod security_context_acceptor;
mod sighand;
mod state;
mod tasks;
mod text;
mod theme;
mod time;
mod tools;
mod tree;
mod udev;
mod user_session;
mod utils;
mod version;
mod video;
mod wheel;
mod wire;
mod wire_dbus;
mod wire_xcon;
mod wl_usr;
mod xcon;
mod xkbcommon;
mod xwayland;

fn main() {
    cli::main();
}
