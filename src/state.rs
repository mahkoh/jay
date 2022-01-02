use crate::async_engine::AsyncEngine;
use crate::event_loop::EventLoopRef;
use crate::globals::Globals;
use crate::utils::numcell::NumCell;
use crate::wl_client::WlClients;
use std::rc::Rc;

pub struct State {
    pub eng: Rc<AsyncEngine>,
    pub el: EventLoopRef,
    pub clients: WlClients,
    pub next_name: NumCell<u32>,
    pub globals: Globals,
}
