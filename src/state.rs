use crate::async_engine::{AsyncEngine, SpawnedFuture};
use crate::backend::{BackendEvent, KeyboardId, KeyboardIds, MouseId, MouseIds, OutputId, OutputIds};
use crate::client::{Client, Clients};
use crate::config::ConfigProxy;
use crate::cursor::ServerCursors;
use crate::event_loop::EventLoop;
use crate::globals::{Globals, GlobalsError, WaylandGlobal};
use crate::ifs::wl_output::WlOutputGlobal;
use crate::ifs::wl_seat::{SeatIds, WlSeatGlobal};
use crate::ifs::wl_surface::NoneSurfaceExt;
use crate::render::RenderContext;
use crate::tree::{DisplayNode, NodeIds};
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::LinkedList;
use crate::utils::numcell::NumCell;
use crate::utils::queue::AsyncQueue;
use crate::{ErrorFmt, Wheel, XkbContext};
use ahash::AHashMap;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use crate::xkbcommon::XkbKeymap;

pub struct State {
    pub xkb_ctx: XkbContext,
    pub default_keymap: Rc<XkbKeymap>,
    pub eng: Rc<AsyncEngine>,
    pub el: Rc<EventLoop>,
    pub render_ctx: CloneCell<Option<Rc<RenderContext>>>,
    pub cursors: CloneCell<Option<Rc<ServerCursors>>>,
    pub wheel: Rc<Wheel>,
    pub clients: Clients,
    pub next_name: NumCell<u32>,
    pub globals: Globals,
    pub output_ids: OutputIds,
    pub seat_ids: SeatIds,
    pub kb_ids: KeyboardIds,
    pub mouse_ids: MouseIds,
    pub node_ids: NodeIds,
    pub root: Rc<DisplayNode>,
    pub backend_events: AsyncQueue<BackendEvent>,
    pub output_handlers: RefCell<AHashMap<OutputId, SpawnedFuture<()>>>,
    pub mouse_handlers: RefCell<AHashMap<MouseId, MouseData>>,
    pub kb_handlers: RefCell<AHashMap<KeyboardId, KeyboardData>>,
    pub outputs: CopyHashMap<OutputId, Rc<WlOutputGlobal>>,
    pub seat_queue: LinkedList<Rc<WlSeatGlobal>>,
    pub slow_clients: AsyncQueue<Rc<Client>>,
    pub none_surface_ext: Rc<NoneSurfaceExt>,
    pub tree_changed_sent: Cell<bool>,
    pub config: CloneCell<Option<Rc<ConfigProxy>>>,
}

pub struct MouseData {
    pub handler: SpawnedFuture<()>,
    pub id: MouseId,
    pub data: Rc<DeviceHandlerData>
}

pub struct KeyboardData {
    pub handler: SpawnedFuture<()>,
    pub id: KeyboardId,
    pub data: Rc<DeviceHandlerData>
}

pub struct DeviceHandlerData {
    pub seat: CloneCell<Option<Rc<WlSeatGlobal>>>,
}

impl State {
    pub fn set_render_ctx(&self, ctx: &Rc<RenderContext>) {
        let cursors = match ServerCursors::load(ctx) {
            Ok(c) => Some(Rc::new(c)),
            Err(e) => {
                log::error!("Could not load the cursors: {}", ErrorFmt(e));
                None
            }
        };
        self.cursors.set(cursors);
        self.render_ctx.set(Some(ctx.clone()));
    }

    pub fn add_global<T: WaylandGlobal>(&self, global: &Rc<T>) {
        self.globals.add_global(self, global)
    }

    pub fn remove_global<T: WaylandGlobal>(&self, global: &T) -> Result<(), GlobalsError> {
        self.globals.remove(self, global)
    }

    pub fn tree_changed(&self) {
        if self.tree_changed_sent.replace(true) {
            return;
        }
        let seats = self.globals.seats.lock();
        for seat in seats.values() {
            seat.trigger_tree_changed();
        }
    }
}
