use crate::async_engine::AsyncEngine;
use crate::client::Clients;
use crate::event_loop::EventLoop;
use crate::format::Format;
use crate::globals::Globals;
use crate::utils::numcell::NumCell;
use ahash::AHashMap;
use std::rc::Rc;

pub struct State {
    pub eng: Rc<AsyncEngine>,
    pub el: Rc<EventLoop>,
    pub clients: Clients,
    pub next_name: NumCell<u32>,
    pub globals: Globals,
    pub formats: AHashMap<u32, &'static Format>,
}
