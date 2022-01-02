#![feature(generic_associated_types, type_alias_impl_trait)]

use crate::acceptor::AcceptorError;
use crate::client::Clients;
use crate::clientmem::ClientMemError;
use crate::event_loop::EventLoopError;
use crate::globals::Globals;
use crate::ifs::wl_compositor::WlCompositorGlobal;
use crate::ifs::wl_shm::WlShmGlobal;
use crate::ifs::wl_subcompositor::WlSubcompositorGlobal;
use crate::ifs::xdg_wm_base::XdgWmBaseGlobal;
use crate::sighand::SighandError;
use crate::state::State;
use crate::utils::numcell::NumCell;
use anyhow::anyhow;
use log::LevelFilter;
use std::rc::Rc;
use thiserror::Error;

#[macro_use]
mod macros;
mod acceptor;
mod async_engine;
mod client;
mod clientmem;
mod event_loop;
mod globals;
mod ifs;
mod object;
mod pixman;
mod sighand;
mod state;
mod time;
mod utils;
mod wheel;

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Trace)
        .init();
    if let Err(e) = main_() {
        log::error!("A fatal error occurred: {:#}", anyhow!(e));
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
}

fn main_() -> Result<(), MainError> {
    clientmem::init()?;
    let el = event_loop::EventLoop::new().unwrap();
    sighand::install(&el.to_ref())?;
    let wheel = wheel::WheelRef::new(&el.to_ref()).unwrap();
    let engine = Rc::new(async_engine::AsyncEngine::new(&el.to_ref(), &wheel).unwrap());
    let globals = Globals::new();
    globals.insert_no_broadcast(Rc::new(WlCompositorGlobal::new(globals.name())));
    globals.insert_no_broadcast(Rc::new(WlShmGlobal::new(globals.name())));
    globals.insert_no_broadcast(Rc::new(WlSubcompositorGlobal::new(globals.name())));
    globals.insert_no_broadcast(Rc::new(XdgWmBaseGlobal::new(globals.name())));
    let state = Rc::new(State {
        eng: engine,
        el: el.to_ref(),
        clients: Clients::new(),
        next_name: NumCell::new(1),
        globals,
    });
    acceptor::Acceptor::install(&state)?;
    el.run()?;
    Ok(())
}
