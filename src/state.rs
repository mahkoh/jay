use crate::async_engine::{AsyncEngine, SpawnedFuture};
use crate::backend::{Backend, BackendEvent, InputDevice, InputDeviceId, InputDeviceIds, OutputId, OutputIds};
use crate::client::{Client, Clients};
use crate::config::ConfigProxy;
use crate::cursor::ServerCursors;
use crate::dbus::Dbus;
use crate::event_loop::EventLoop;
use crate::forker::ForkerProxy;
use crate::globals::{Globals, GlobalsError, WaylandGlobal};
use crate::ifs::wl_output::WlOutputGlobal;
use crate::ifs::wl_seat::{SeatIds, WlSeatGlobal};
use crate::ifs::wl_surface::NoneSurfaceExt;
use crate::rect::Rect;
use crate::render::RenderContext;
use crate::theme::Theme;
use crate::tree::{
    ContainerNode, ContainerSplit, DisplayNode, FloatNode, Node, NodeIds, WorkspaceNode,
};
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::fdcloser::FdCloser;
use crate::utils::linkedlist::LinkedList;
use crate::utils::numcell::NumCell;
use crate::utils::queue::AsyncQueue;
use crate::xkbcommon::XkbKeymap;
use crate::{ErrorFmt, Wheel, XkbContext};
use ahash::AHashMap;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

pub struct State {
    pub xkb_ctx: XkbContext,
    pub backend: CloneCell<Option<Rc<dyn Backend>>>,
    pub forker: CloneCell<Option<Rc<ForkerProxy>>>,
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
    pub input_device_ids: InputDeviceIds,
    pub node_ids: NodeIds,
    pub root: Rc<DisplayNode>,
    pub backend_events: AsyncQueue<BackendEvent>,
    pub output_handlers: RefCell<AHashMap<OutputId, SpawnedFuture<()>>>,
    pub input_device_handlers: RefCell<AHashMap<InputDeviceId, InputDeviceData>>,
    pub outputs: CopyHashMap<OutputId, Rc<WlOutputGlobal>>,
    pub seat_queue: LinkedList<Rc<WlSeatGlobal>>,
    pub slow_clients: AsyncQueue<Rc<Client>>,
    pub none_surface_ext: Rc<NoneSurfaceExt>,
    pub tree_changed_sent: Cell<bool>,
    pub config: CloneCell<Option<Rc<ConfigProxy>>>,
    pub theme: Theme,
    pub pending_container_layout: AsyncQueue<Rc<ContainerNode>>,
    pub pending_container_titles: AsyncQueue<Rc<ContainerNode>>,
    pub pending_float_layout: AsyncQueue<Rc<FloatNode>>,
    pub pending_float_titles: AsyncQueue<Rc<FloatNode>>,
    pub dbus: Dbus,
    pub fdcloser: Arc<FdCloser>,
}

pub struct InputDeviceData {
    pub handler: SpawnedFuture<()>,
    pub id: InputDeviceId,
    pub device: Rc<dyn InputDevice>,
    pub data: Rc<DeviceHandlerData>,
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

    pub fn map_tiled(self: &Rc<Self>, node: Rc<dyn Node>) {
        let seat = self.seat_queue.last();
        if let Some(seat) = seat {
            if let Some(prev) = seat.last_tiled_keyboard_toplevel(&*node) {
                if let Some(container) = prev.parent() {
                    if let Some(container) = container.into_container() {
                        container.add_child_after(prev.as_node(), node);
                        return;
                    }
                }
            }
        }
        let output = {
            let outputs = self.root.outputs.lock();
            outputs.values().next().cloned()
        };
        if let Some(output) = output {
            if let Some(workspace) = output.workspace.get() {
                if let Some(container) = workspace.container.get() {
                    container.append_child(node);
                } else {
                    let container = ContainerNode::new(
                        self,
                        &workspace,
                        workspace.clone(),
                        node,
                        ContainerSplit::Horizontal,
                    );
                    workspace.set_container(&container);
                };
                return;
            }
        }
        todo!("map_tiled");
    }

    pub fn map_floating(
        self: &Rc<Self>,
        node: Rc<dyn Node>,
        mut width: i32,
        mut height: i32,
        workspace: &Rc<WorkspaceNode>,
    ) {
        node.clone().set_workspace(workspace);
        width += 2 * self.theme.border_width.get();
        height += 2 * self.theme.border_width.get() + self.theme.title_height.get();
        let output = workspace.output.get();
        let output_rect = output.position.get();
        let position = {
            let mut x1 = output_rect.x1();
            let mut y1 = output_rect.y1();
            if width < output_rect.width() {
                x1 += (output_rect.width() - width) as i32 / 2;
            } else {
                width = output_rect.width();
            }
            if height < output_rect.height() {
                y1 += (output_rect.height() - height) as i32 / 2;
            } else {
                height = output_rect.height();
            }
            Rect::new_sized(x1, y1, width, height).unwrap()
        };
        FloatNode::new(self, workspace, position, node);
    }
}
