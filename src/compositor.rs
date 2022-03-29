use crate::acceptor::{Acceptor, AcceptorError};
use crate::async_engine::{AsyncEngine, AsyncError, Phase};
use crate::client::Clients;
use crate::clientmem::ClientMemError;
use crate::config::ConfigProxy;
use crate::dbus::Dbus;
use crate::event_loop::{EventLoop, EventLoopError};
use crate::forker::ForkerError;
use crate::globals::Globals;
use crate::ifs::wl_surface::NoneSurfaceExt;
use crate::render::RenderError;
use crate::sighand::SighandError;
use crate::state::State;
use crate::tree::{
    container_layout, container_render_data, float_layout, float_titles, DisplayNode, NodeIds,
};
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fdcloser::FdCloser;
use crate::utils::numcell::NumCell;
use crate::utils::queue::AsyncQueue;
use crate::utils::run_toplevel::RunToplevel;
use crate::wheel::{Wheel, WheelError};
use crate::xkbcommon::XkbContext;
use crate::{clientmem, forker, leaks, render, sighand, tasks, xwayland};
use log::LevelFilter;
use std::cell::Cell;
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;

pub fn start_compositor() {
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
    let config = ConfigProxy::default(&state);
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
    forker.setenv(b"_JAVA_AWT_WM_NONREPARENTING", b"1");
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