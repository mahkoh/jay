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

use crate::acceptor::AcceptorError;
use crate::async_engine::{AsyncError, Phase};
use crate::backends::metal;
use crate::client::Clients;
use crate::clientmem::ClientMemError;
use crate::dbus::{Dbus, FALSE};
use crate::event_loop::EventLoopError;
use crate::forker::ForkerError;
use crate::globals::Globals;
use crate::ifs::wl_compositor::WlCompositorGlobal;
use crate::ifs::wl_shm::WlShmGlobal;
use crate::ifs::wl_subcompositor::WlSubcompositorGlobal;
use crate::ifs::wl_surface::NoneSurfaceExt;
use crate::ifs::xdg_wm_base::XdgWmBaseGlobal;
use crate::ifs::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1Global;
use crate::ifs::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1Global;
use crate::render::RenderError;
use crate::sighand::SighandError;
use crate::state::State;
use crate::tree::{
    container_layout, container_render_data, float_layout, float_titles, DisplayNode, NodeIds,
};
use crate::udev::Udev;
use crate::utils::clonecell::CloneCell;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fdcloser::FdCloser;
use crate::utils::numcell::NumCell;
use crate::utils::queue::AsyncQueue;
use crate::utils::run_toplevel::RunToplevel;
use crate::wheel::WheelError;
use crate::wire_dbus::org;
use crate::xkbcommon::XkbContext;
use acceptor::Acceptor;
use async_engine::AsyncEngine;
use event_loop::EventLoop;
use log::LevelFilter;
use std::cell::Cell;
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;
use wheel::Wheel;

#[macro_use]
mod macros;
#[macro_use]
mod leaks;
mod acceptor;
mod async_engine;
mod backend;
mod backends;
mod bugs;
mod client;
mod clientmem;
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
    env_logger::builder()
        .format_timestamp_millis()
        .filter_level(LevelFilter::Info)
        .filter_level(LevelFilter::Debug)
        // .filter_level(LevelFilter::Trace)
        .init();
    if let Err(e) = main_() {
        log::error!("A fatal error occurred: {}", ErrorFmt(e));
        std::process::exit(1);
    }
}

#[derive(Debug, Error)]
enum MainError {
    #[error("The client acceptor caused an error")]
    AcceptorError(#[from] AcceptorError),
    #[error("The event loop caused an error")]
    EventLoopError(#[from] EventLoopError),
    #[error("The signal handler caused an error")]
    SighandError(#[from] SighandError),
    #[error("The clientmem subsystem caused an error")]
    ClientmemError(#[from] ClientMemError),
    #[error("The timer subsystem caused an error")]
    WheelError(#[from] WheelError),
    #[error("The async subsystem caused an error")]
    AsyncError(#[from] AsyncError),
    #[error("The render backend caused an error")]
    RenderError(#[from] RenderError),
    #[error("The ol' forker caused an error")]
    ForkerError(#[from] ForkerError),
}

fn main_() -> Result<(), MainError> {
    let forker = Rc::new(forker::ForkerProxy::create()?);
    leaks::init();
    render::init()?;
    clientmem::init()?;
    let el = EventLoop::new()?;
    sighand::install(&el)?;
    let xkb_ctx = XkbContext::new().unwrap();
    let xkb_keymap = xkb_ctx.keymap_from_str(include_str!("keymap.xkb")).unwrap();
    let wheel = Wheel::install(&el)?;
    let engine = AsyncEngine::install(&el, &wheel)?;
    let (_run_toplevel_future, run_toplevel) = RunToplevel::install(&engine);
    let node_ids = NodeIds::default();
    let state = Rc::new(State {
        xkb_ctx,
        backend: Default::default(),
        forker: Default::default(),
        default_keymap: xkb_keymap,
        eng: engine.clone(),
        el: el.clone(),
        render_ctx: Default::default(),
        cursors: Default::default(),
        wheel,
        clients: Clients::new(),
        next_name: NumCell::new(1),
        globals: Globals::new(),
        output_ids: Default::default(),
        root: Rc::new(DisplayNode::new(node_ids.next())),
        node_ids,
        backend_events: AsyncQueue::new(),
        output_handlers: Default::default(),
        seat_ids: Default::default(),
        outputs: Default::default(),
        seat_queue: Default::default(),
        slow_clients: AsyncQueue::new(),
        none_surface_ext: Rc::new(NoneSurfaceExt),
        tree_changed_sent: Cell::new(false),
        config: Default::default(),
        input_device_ids: Default::default(),
        input_device_handlers: Default::default(),
        theme: Default::default(),
        pending_container_layout: Default::default(),
        pending_container_render_data: Default::default(),
        pending_float_layout: Default::default(),
        pending_float_titles: Default::default(),
        dbus: Dbus::new(&engine, &run_toplevel),
        fdcloser: FdCloser::new(),
    });
    forker.install(&state);
    let config = config::ConfigProxy::default(&state);
    state.config.set(Some(Rc::new(config)));
    let _global_event_handler = engine.spawn(tasks::handle_backend_events(state.clone()));
    let _slow_client_handler = engine.spawn(tasks::handle_slow_clients(state.clone()));
    let _container_do_layout = engine.spawn2(Phase::Layout, container_layout(state.clone()));
    let _container_render_titles =
        engine.spawn2(Phase::PostLayout, container_render_data(state.clone()));
    let _float_do_layout = engine.spawn2(Phase::Layout, float_layout(state.clone()));
    let _float_render_titles = engine.spawn2(Phase::PostLayout, float_titles(state.clone()));
    let socket_path = Acceptor::install(&state)?;
    forker.setenv(b"WAYLAND_DISPLAY", socket_path.as_bytes());
    let _xwayland = engine.spawn(xwayland::manage(state.clone()));
    let _backend = engine.spawn(tasks::start_backend(state.clone()));
    el.run()?;
    drop(_xwayland);
    state.clients.clear();
    for (_, seat) in state.globals.seats.lock().deref() {
        seat.clear();
    }
    leaks::log_leaked();
    Ok(())
}
