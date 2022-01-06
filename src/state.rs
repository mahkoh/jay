use crate::async_engine::{AsyncEngine, SpawnedFuture};
use crate::backend::{BackendEvent, OutputId, OutputIds, SeatId, SeatIds};
use crate::client::Clients;
use crate::event_loop::EventLoop;
use crate::format::Format;
use crate::globals::Globals;
use crate::tree::{DisplayNode, NodeIds};
use crate::utils::numcell::NumCell;
use crate::utils::queue::AsyncQueue;
use crate::Wheel;
use ahash::AHashMap;
use std::cell::RefCell;
use std::rc::Rc;

pub struct State {
    pub eng: Rc<AsyncEngine>,
    pub el: Rc<EventLoop>,
    pub wheel: Rc<Wheel>,
    pub clients: Clients,
    pub next_name: NumCell<u32>,
    pub globals: Globals,
    pub formats: AHashMap<u32, &'static Format>,
    pub output_ids: OutputIds,
    pub seat_ids: SeatIds,
    pub node_ids: NodeIds,
    pub root: Rc<DisplayNode>,
    pub backend_events: AsyncQueue<BackendEvent>,
    pub output_handlers: RefCell<AHashMap<OutputId, SpawnedFuture<()>>>,
    pub seat_handlers: RefCell<AHashMap<SeatId, SpawnedFuture<()>>>,
}
