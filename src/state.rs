use crate::async_engine::{AsyncEngine, SpawnedFuture};
use crate::backend::{
    Backend, BackendEvent, InputDevice, InputDeviceId, InputDeviceIds, OutputId, OutputIds,
};
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
use crate::logger::Logger;
use crate::rect::Rect;
use crate::render::RenderContext;
use crate::theme::Theme;
use crate::tree::walker::NodeVisitorBase;
use crate::tree::{
    ContainerNode, ContainerSplit, DisplayNode, FloatNode, Node, NodeIds, OutputNode, WorkspaceNode,
};
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fdcloser::FdCloser;
use crate::utils::linkedlist::LinkedList;
use crate::utils::numcell::NumCell;
use crate::utils::queue::AsyncQueue;
use crate::wheel::Wheel;
use crate::xkbcommon::{XkbContext, XkbKeymap};
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
    pub workspaces: CopyHashMap<String, Rc<WorkspaceNode>>,
    pub dummy_output: CloneCell<Option<Rc<OutputNode>>>,
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
    pub pending_container_render_data: AsyncQueue<Rc<ContainerNode>>,
    pub pending_float_layout: AsyncQueue<Rc<FloatNode>>,
    pub pending_float_titles: AsyncQueue<Rc<FloatNode>>,
    pub dbus: Dbus,
    pub fdcloser: Arc<FdCloser>,
    pub logger: Arc<Logger>,
}

pub struct InputDeviceData {
    pub handler: SpawnedFuture<()>,
    pub id: InputDeviceId,
    pub data: Rc<DeviceHandlerData>,
}

pub struct DeviceHandlerData {
    pub seat: CloneCell<Option<Rc<WlSeatGlobal>>>,
    pub device: Rc<dyn InputDevice>,
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

        struct Walker;
        impl NodeVisitorBase for Walker {
            fn visit_container(&mut self, node: &Rc<ContainerNode>) {
                node.schedule_compute_render_data();
                node.visit_children(self);
            }

            fn visit_output(&mut self, node: &Rc<OutputNode>) {
                node.update_render_data();
                node.visit_children(self);
            }

            fn visit_float(&mut self, node: &Rc<FloatNode>) {
                node.schedule_render_titles();
                node.visit_children(self);
            }
        }
        self.root.clone().visit(&mut Walker);
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
        if let Some(seat) = &seat {
            if let Some(prev) = seat.last_tiled_keyboard_toplevel(&*node) {
                if let Some(container) = prev.parent() {
                    if let Some(container) = container.into_container() {
                        container.add_child_after(prev.as_node(), node);
                        return;
                    }
                }
            }
        }
        let mut output = seat.map(|s| s.get_output());
        if output.is_none() {
            let outputs = self.root.outputs.lock();
            output = outputs.values().next().cloned();
        }
        let output = match output {
            Some(output) => output,
            _ => self.dummy_output.get().unwrap(),
        };
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
        log::warn!("Output has no workspace set");
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
        let output_rect = output.global.pos.get();
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

    pub fn show_workspace(&self, seat: &Rc<WlSeatGlobal>, name: &str) {
        let output = match self.workspaces.get(name) {
            Some(ws) => {
                let output = ws.output.get();
                if let Some(old) = output.workspace.get() {
                    if old.id == ws.id {
                        return;
                    }
                }
                output.show_workspace(&ws);
                output
            }
            _ => {
                let output = seat.get_output();
                if output.is_dummy {
                    log::warn!("Not showing workspace because seat is on dummy output");
                    return;
                }
                let workspace = Rc::new(WorkspaceNode {
                    id: self.node_ids.next(),
                    output: CloneCell::new(output.clone()),
                    position: Cell::new(Default::default()),
                    container: Default::default(),
                    stacked: Default::default(),
                    seat_state: Default::default(),
                    name: name.to_string(),
                    output_link: Cell::new(None),
                });
                workspace
                    .output_link
                    .set(Some(output.workspaces.add_last(workspace.clone())));
                output.show_workspace(&workspace);
                self.workspaces.set(name.to_string(), workspace);
                output
            }
        };
        output.update_render_data();
        self.tree_changed();
        let seats = self.globals.seats.lock();
        for seat in seats.values() {
            seat.workspace_changed(&output);
        }
    }
}
