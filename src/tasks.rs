mod backend;
mod connector;
mod drmdev;
mod hardware_cursor;
mod idle;
mod input_device;
mod slow_clients;
mod udev_utils;

use {
    crate::{
        state::State,
        tasks::{
            backend::BackendEventHandler,
            slow_clients::{SlowClientHandler, SlowEiClientHandler},
        },
    },
    std::rc::Rc,
};
pub use {hardware_cursor::handle_hardware_cursor_tick, idle::idle};

pub async fn handle_backend_events(state: Rc<State>) {
    let mut beh = BackendEventHandler { state };
    beh.handle_events().await;
}

pub async fn handle_slow_clients(state: Rc<State>) {
    let mut sch = SlowClientHandler { state };
    sch.handle_events().await;
}

pub async fn handle_slow_ei_clients(state: Rc<State>) {
    let mut sch = SlowEiClientHandler { state };
    sch.handle_events().await;
}
