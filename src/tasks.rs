mod backend;
mod connector;
mod input_device;
mod slow_clients;
mod start_backend;

pub use start_backend::start_backend;
use {
    crate::{
        state::State,
        tasks::{backend::BackendEventHandler, slow_clients::SlowClientHandler},
    },
    std::rc::Rc,
};

pub async fn handle_backend_events(state: Rc<State>) {
    let mut beh = BackendEventHandler { state };
    beh.handle_events().await;
}

pub async fn handle_slow_clients(state: Rc<State>) {
    let mut sch = SlowClientHandler { state };
    sch.handle_events().await;
}
