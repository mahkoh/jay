mod backend;
mod connector;
mod const_clock;
mod drmdev;
mod hardware_cursor;
mod idle;
mod input_device;
mod slow_clients;
mod udev_utils;

use crate::state::State;
use crate::tasks::backend::BackendEventHandler;
use crate::tasks::const_clock::run_const_clock;
use crate::tasks::slow_clients::SlowClientHandler;
use crate::tasks::slow_clients::SlowEiClientHandler;
pub use connector::handle as handle_connector;
pub use hardware_cursor::handle_hardware_cursor_tick;
pub use idle::idle;
use std::rc::Rc;
use std::time::Duration;

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

pub async fn handle_const_40hz_latch(state: Rc<State>) {
    let output = state.dummy_output.get().unwrap();
    let duration = Duration::from_nanos(1_000_000_000 / 40);
    run_const_clock(duration, &state.ring, &state.const_40hz_latch, |l| {
        l.after_latch(&output, false)
    })
    .await;
}
