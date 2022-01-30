mod types;

use crate::client::DynEventFormatter;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::{NodeSeatState, WlSeatGlobal};
use crate::ifs::wl_surface::xdg_surface::{XdgSurface, XdgSurfaceError, XdgSurfaceExt};
use crate::object::{Interface, Object, ObjectId};
use crate::rect::Rect;
use crate::render::Renderer;
use crate::tree::{ContainerNode, FindTreeResult};
use crate::tree::{FloatNode, FoundNode, Node, NodeId, ToplevelNodeId, WorkspaceNode};
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use ahash::{AHashMap, AHashSet};
use num_derive::FromPrimitive;
use std::cell::{Cell, RefCell};
use std::mem;
use std::rc::Rc;
pub use types::*;
use crate::backend::SeatId;
use crate::utils::linkedlist::LinkedNode;
use crate::utils::smallmap::SmallMap;

const DESTROY: u32 = 0;
const SET_PARENT: u32 = 1;
const SET_TITLE: u32 = 2;
const SET_APP_ID: u32 = 3;
const SHOW_WINDOW_MENU: u32 = 4;
const MOVE: u32 = 5;
const RESIZE: u32 = 6;
const SET_MAX_SIZE: u32 = 7;
const SET_MIN_SIZE: u32 = 8;
const SET_MAXIMIZED: u32 = 9;
const UNSET_MAXIMIZED: u32 = 10;
const SET_FULLSCREEN: u32 = 11;
const UNSET_FULLSCREEN: u32 = 12;
const SET_MINIMIZED: u32 = 13;

const CONFIGURE: u32 = 0;
const CLOSE: u32 = 1;

#[derive(Copy, Clone, Debug, FromPrimitive)]
pub enum ResizeEdge {
    None = 0,
    Top = 1,
    Bottom = 2,
    Left = 4,
    TopLeft = 5,
    BottomLeft = 6,
    Right = 8,
    TopRight = 9,
    BottomRight = 10,
}

#[allow(dead_code)]
const STATE_MAXIMIZED: u32 = 1;
#[allow(dead_code)]
const STATE_FULLSCREEN: u32 = 2;
#[allow(dead_code)]
const STATE_RESIZING: u32 = 3;
#[allow(dead_code)]
const STATE_ACTIVATED: u32 = 4;
#[allow(dead_code)]
const STATE_TILED_LEFT: u32 = 5;
#[allow(dead_code)]
const STATE_TILED_RIGHT: u32 = 6;
#[allow(dead_code)]
const STATE_TILED_TOP: u32 = 7;
#[allow(dead_code)]
const STATE_TILED_BOTTOM: u32 = 8;

id!(XdgToplevelId);

pub struct XdgToplevel {
    pub id: XdgToplevelId,
    pub xdg: Rc<XdgSurface>,
    pub node_id: ToplevelNodeId,
    pub parent_node: CloneCell<Option<Rc<dyn Node>>>,
    pub parent: CloneCell<Option<Rc<XdgToplevel>>>,
    pub children: RefCell<AHashMap<XdgToplevelId, Rc<XdgToplevel>>>,
    states: RefCell<AHashSet<u32>>,
    pub toplevel_history: SmallMap<SeatId, LinkedNode<Rc<XdgToplevel>>, 1>,
}

impl XdgToplevel {
    pub fn new(id: XdgToplevelId, surface: &Rc<XdgSurface>) -> Self {
        let mut states = AHashSet::new();
        states.insert(STATE_TILED_LEFT);
        states.insert(STATE_TILED_RIGHT);
        states.insert(STATE_TILED_TOP);
        states.insert(STATE_TILED_BOTTOM);
        Self {
            id,
            xdg: surface.clone(),
            node_id: surface.surface.client.state.node_ids.next(),
            parent_node: Default::default(),
            parent: Default::default(),
            children: RefCell::new(Default::default()),
            states: RefCell::new(states),
            toplevel_history: Default::default(),
        }
    }

    pub fn parent_is_float(&self) -> bool {
        if let Some(parent) = self.parent_node.get() {
            return parent.is_float();
        }
        false
    }

    pub fn configure(self: &Rc<Self>, width: i32, height: i32) -> DynEventFormatter {
        Box::new(Configure {
            obj: self.clone(),
            width,
            height,
            states: self.states.borrow().iter().copied().collect(),
        })
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.xdg.surface.client.parse(self, parser)?;
        self.destroy_node(true);
        self.xdg.ext.set(None);
        if let Some(parent) = self.parent_node.take() {
            parent.remove_child(self);
        }
        {
            let mut children = self.children.borrow_mut();
            for (_, child) in children.drain() {
                child.parent.set(self.parent.get());
            }
        }
        Ok(())
    }

    fn set_parent(&self, parser: MsgParser<'_, '_>) -> Result<(), SetParentError> {
        let _req: SetParent = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn set_title(&self, parser: MsgParser<'_, '_>) -> Result<(), SetTitleError> {
        let _req: SetTitle = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn set_app_id(&self, parser: MsgParser<'_, '_>) -> Result<(), SetAppIdError> {
        let _req: SetAppId = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn show_window_menu(&self, parser: MsgParser<'_, '_>) -> Result<(), ShowWindowMenuError> {
        let _req: ShowWindowMenu = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn move_(&self, parser: MsgParser<'_, '_>) -> Result<(), MoveError> {
        let req: Move = self.xdg.surface.client.parse(self, parser)?;
        let seat = self.xdg.surface.client.get_wl_seat(req.seat)?;
        if let Some(parent) = self.parent_node.get() {
            if let Some(float) = parent.into_float() {
                seat.move_(&float);
            }
        }
        Ok(())
    }

    fn resize(&self, parser: MsgParser<'_, '_>) -> Result<(), ResizeError> {
        let _req: Resize = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn set_max_size(&self, parser: MsgParser<'_, '_>) -> Result<(), SetMaxSizeError> {
        let _req: SetMaxSize = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn set_min_size(&self, parser: MsgParser<'_, '_>) -> Result<(), SetMinSizeError> {
        let _req: SetMinSize = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn set_maximized(&self, parser: MsgParser<'_, '_>) -> Result<(), SetMaximizedError> {
        let _req: SetMaximized = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn unset_maximized(&self, parser: MsgParser<'_, '_>) -> Result<(), UnsetMaximizedError> {
        let _req: UnsetMaximized = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn set_fullscreen(&self, parser: MsgParser<'_, '_>) -> Result<(), SetFullscreenError> {
        let _req: SetFullscreen = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn unset_fullscreen(&self, parser: MsgParser<'_, '_>) -> Result<(), UnsetFullscreenError> {
        let _req: UnsetFullscreen = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn set_minimized(&self, parser: MsgParser<'_, '_>) -> Result<(), SetMinimizedError> {
        let _req: SetMinimized = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), XdgToplevelError> {
        match request {
            DESTROY => self.destroy(parser)?,
            SET_PARENT => self.set_parent(parser)?,
            SET_TITLE => self.set_title(parser)?,
            SET_APP_ID => self.set_app_id(parser)?,
            SHOW_WINDOW_MENU => self.show_window_menu(parser)?,
            MOVE => self.move_(parser)?,
            RESIZE => self.resize(parser)?,
            SET_MAX_SIZE => self.set_max_size(parser)?,
            SET_MIN_SIZE => self.set_min_size(parser)?,
            SET_MAXIMIZED => self.set_maximized(parser)?,
            UNSET_MAXIMIZED => self.unset_maximized(parser)?,
            SET_FULLSCREEN => self.set_fullscreen(parser)?,
            UNSET_FULLSCREEN => self.unset_fullscreen(parser)?,
            SET_MINIMIZED => self.set_minimized(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }

    fn map_child(self: &Rc<Self>, parent: &XdgToplevel) {
        let workspace = match parent.xdg.workspace.get() {
            Some(w) => w,
            _ => return self.map_tiled(),
        };
        self.xdg.set_workspace(&workspace);
        let output = workspace.output.get();
        let output_rect = output.position.get();
        let position = {
            let extents = self.xdg.extents.get().to_origin();
            let width = extents.width();
            let height = extents.height();
            let mut x1 = output_rect.x1();
            let mut y1 = output_rect.y1();
            if width < output_rect.width() {
                x1 += (output_rect.width() - width) as i32 / 2;
            }
            if height < output_rect.height() {
                y1 += (output_rect.height() - height) as i32 / 2;
            }
            Rect::new_sized(x1, y1, width, height).unwrap()
        };
        let state = &self.xdg.surface.client.state;
        let floater = Rc::new(FloatNode {
            id: state.node_ids.next(),
            visible: Cell::new(true),
            position: Cell::new(position),
            display: output.display.clone(),
            display_link: Cell::new(None),
            workspace_link: Cell::new(None),
            workspace: CloneCell::new(workspace.clone()),
            child: CloneCell::new(Some(self.clone())),
            seat_state: Default::default(),
        });
        self.parent_node.set(Some(floater.clone()));
        floater.display_link.set(Some(
            state
                .root
                .stacked
                .add_last(floater.clone()),
        ));
        floater.workspace_link.set(Some(
            workspace
                .stacked
                .add_last(floater.clone()),
        ));
    }

    fn map_tiled(self: &Rc<Self>) {
        log::info!("mapping tiled");
        let state = &self.xdg.surface.client.state;
        let seat = state.seat_queue.last();
        if let Some(seat) = seat {
            if let Some(prev) = seat.last_tiled_keyboard_toplevel() {
                if let Some(container) = prev.parent_node.get() {
                    if let Some(container) = container.into_container() {
                        container.add_child_after(&*prev, self.clone());
                        self.parent_node.set(Some(container));
                        return;
                    }
                }
            }
        }
        let output = {
            let outputs = state.root.outputs.lock();
            outputs.values().next().cloned()
        };
        if let Some(output) = output {
            if let Some(workspace) = output.workspace.get() {
                if let Some(container) = workspace.container.get() {
                    container.append_child(self.clone());
                    self.parent_node.set(Some(container));
                } else {
                    let container =
                        Rc::new(ContainerNode::new(state, &workspace, workspace.clone(), self.clone()));
                    workspace.set_container(&container);
                    self.parent_node.set(Some(container));
                };
                return;
            }
        }
        todo!("map_tiled");
    }
}

handle_request!(XdgToplevel);

impl Object for XdgToplevel {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::XdgToplevel
    }

    fn num_requests(&self) -> u32 {
        SET_MINIMIZED + 1
    }

    fn break_loops(&self) {
        self.destroy_node(true);
        if let Some(parent) = self.parent_node.take() {
            parent.remove_child(self);
        }
        self.parent.set(None);
        let _children = mem::take(&mut *self.children.borrow_mut());
    }
}

impl Node for XdgToplevel {
    fn id(&self) -> NodeId {
        self.node_id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.xdg.seat_state
    }

    fn destroy_node(&self, detach: bool) {
        if let Some(parent) = self.parent_node.take() {
            if detach {
                parent.remove_child(self);
            }
        }
        self.toplevel_history.take();
        self.xdg.destroy_node();
        self.xdg.seat_state.destroy_node(self)
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        self.xdg.find_tree_at(x, y, tree)
    }

    fn enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.enter_toplevel(&self);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_xdg_surface(&self.xdg, x, y)
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        let de = self.xdg.absolute_desired_extents.replace(*rect);
        if de.width() != rect.width() || de.height() != rect.height() {
            self.xdg
                .surface
                .client
                .event(self.configure(rect.width(), rect.height()));
            self.xdg.send_configure();
            self.xdg.surface.client.flush();
        }
        if de.x1() != rect.x1() || de.y1() != rect.y1() {
            self.xdg.update_popup_positions();
        }
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        self.xdg.set_workspace(ws);
    }
}

impl XdgSurfaceExt for XdgToplevel {
    fn initial_configure(self: Rc<Self>) -> Result<(), XdgSurfaceError> {
        self.xdg.surface.client.event(self.configure(0, 0));
        Ok(())
    }

    fn post_commit(self: Rc<Self>) {
        let surface = &self.xdg.surface;
        if let Some(parent) = self.parent_node.get() {
            if surface.buffer.get().is_none() {
                parent.remove_child(&*self);
                {
                    let new_parent = self.parent.get();
                    let mut children = self.children.borrow_mut();
                    for (_, child) in children.drain() {
                        child.parent.set(new_parent.clone());
                    }
                }
                surface.client.state.tree_changed();
            }
        } else if surface.buffer.get().is_some() {
            if let Some(parent) = self.parent.get() {
                self.map_child(&parent);
            } else {
                self.map_tiled();
            }
            self.extents_changed();
            surface.client.state.tree_changed();
        }
    }

    fn extents_changed(&self) {
        if let Some(parent) = self.parent_node.get() {
            let extents = self.xdg.extents.get();
            parent.child_size_changed(self, extents.width(), extents.height());
            self.xdg.surface.client.state.tree_changed();
        }
    }

    fn into_node(self: Rc<Self>) -> Option<Rc<dyn Node>> {
        Some(self)
    }
}
