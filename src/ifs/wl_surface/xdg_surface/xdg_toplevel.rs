use crate::bugs;
use crate::bugs::Bugs;
use crate::client::{Client, ClientError};
use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::{NodeSeatState, WlSeatGlobal};
use crate::ifs::wl_surface::xdg_surface::{XdgSurface, XdgSurfaceError, XdgSurfaceExt};
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::rect::Rect;
use crate::render::Renderer;
use crate::tree::toplevel::{ToplevelData, ToplevelNode};
use crate::tree::walker::NodeVisitor;
use crate::tree::FindTreeResult;
use crate::tree::{FoundNode, Node, NodeId, ToplevelNodeId, WorkspaceNode};
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::utils::clonecell::CloneCell;
use crate::wire::xdg_toplevel::*;
use crate::wire::XdgToplevelId;
use ahash::{AHashMap, AHashSet};
use jay_config::Direction;
use num_derive::FromPrimitive;
use std::cell::{Cell, RefCell};
use std::fmt::{Debug, Formatter};
use std::mem;
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;

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
const STATE_ACTIVATED: u32 = 4;
const STATE_TILED_LEFT: u32 = 5;
const STATE_TILED_RIGHT: u32 = 6;
const STATE_TILED_TOP: u32 = 7;
const STATE_TILED_BOTTOM: u32 = 8;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Decoration {
    #[allow(dead_code)]
    Client,
    Server,
}

pub struct XdgToplevel {
    pub id: XdgToplevelId,
    pub xdg: Rc<XdgSurface>,
    pub node_id: ToplevelNodeId,
    pub parent_node: CloneCell<Option<Rc<dyn Node>>>,
    pub parent: CloneCell<Option<Rc<XdgToplevel>>>,
    pub children: RefCell<AHashMap<XdgToplevelId, Rc<XdgToplevel>>>,
    states: RefCell<AHashSet<u32>>,
    pub decoration: Cell<Decoration>,
    bugs: Cell<&'static Bugs>,
    min_width: Cell<Option<i32>>,
    min_height: Cell<Option<i32>>,
    max_width: Cell<Option<i32>>,
    max_height: Cell<Option<i32>>,
    title: RefCell<String>,
    pub tracker: Tracker<Self>,
    toplevel_data: ToplevelData,
}

impl Debug for XdgToplevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XdgToplevel").finish_non_exhaustive()
    }
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
            decoration: Cell::new(Decoration::Server),
            bugs: Cell::new(&bugs::NONE),
            min_width: Cell::new(None),
            min_height: Cell::new(None),
            max_width: Cell::new(None),
            max_height: Cell::new(None),
            title: RefCell::new("".to_string()),
            tracker: Default::default(),
            toplevel_data: Default::default(),
        }
    }

    fn send_configure_checked(&self, mut width: i32, mut height: i32) {
        width = width.max(1);
        height = height.max(1);
        if self.bugs.get().respect_min_max_size {
            if let Some(min) = self.min_width.get() {
                width = width.max(min);
            }
            if let Some(min) = self.min_height.get() {
                height = height.max(min);
            }
            if let Some(max) = self.max_width.get() {
                width = width.min(max);
            }
            if let Some(max) = self.max_height.get() {
                height = height.min(max);
            }
        }
        self.send_configure(width, height)
    }

    fn send_configure(&self, width: i32, height: i32) {
        let states: Vec<_> = self.states.borrow().iter().copied().collect();
        self.xdg.surface.client.event(Configure {
            self_id: self.id,
            width,
            height,
            states: &states,
        })
    }

    fn destroy(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.xdg.surface.client.parse(self.deref(), parser)?;
        self.destroy_node(true);
        self.xdg.ext.set(None);
        {
            let mut children = self.children.borrow_mut();
            let parent = self.parent.get();
            let mut parent_children = match &parent {
                Some(p) => Some(p.children.borrow_mut()),
                _ => None,
            };
            for (_, child) in children.drain() {
                child.parent.set(parent.clone());
                if let Some(parent_children) = &mut parent_children {
                    parent_children.insert(child.id, child);
                }
            }
        }
        {
            if let Some(parent) = self.parent.take() {
                parent.children.borrow_mut().remove(&self.id);
            }
        }
        self.xdg.surface.client.remove_obj(self.deref())?;
        self.xdg.surface.set_toplevel(None);
        Ok(())
    }

    fn set_parent(&self, parser: MsgParser<'_, '_>) -> Result<(), SetParentError> {
        let req: SetParent = self.xdg.surface.client.parse(self, parser)?;
        let mut parent = None;
        if req.parent.is_some() {
            parent = Some(self.xdg.surface.client.lookup(req.parent)?);
        }
        self.parent.set(parent);
        Ok(())
    }

    fn set_title(&self, parser: MsgParser<'_, '_>) -> Result<(), SetTitleError> {
        let req: SetTitle = self.xdg.surface.client.parse(self, parser)?;
        let mut title = self.title.borrow_mut();
        title.clear();
        title.push_str(req.title);
        if let Some(parent) = self.parent_node.get() {
            parent.child_title_changed(self, &title);
        }
        Ok(())
    }

    fn set_app_id(&self, parser: MsgParser<'_, '_>) -> Result<(), SetAppIdError> {
        let req: SetAppId = self.xdg.surface.client.parse(self, parser)?;
        self.bugs.set(bugs::get(req.app_id));
        Ok(())
    }

    fn show_window_menu(&self, parser: MsgParser<'_, '_>) -> Result<(), ShowWindowMenuError> {
        let _req: ShowWindowMenu = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn move_(&self, parser: MsgParser<'_, '_>) -> Result<(), MoveError> {
        let req: Move = self.xdg.surface.client.parse(self, parser)?;
        let seat = self.xdg.surface.client.lookup(req.seat)?;
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
        let req: SetMaxSize = self.xdg.surface.client.parse(self, parser)?;
        if req.height < 0 || req.width < 0 {
            return Err(SetMaxSizeError::NonNegative);
        }
        self.max_width.set(if req.width == 0 {
            None
        } else {
            Some(req.width)
        });
        self.max_height.set(if req.height == 0 {
            None
        } else {
            Some(req.height)
        });
        Ok(())
    }

    fn set_min_size(&self, parser: MsgParser<'_, '_>) -> Result<(), SetMinSizeError> {
        let req: SetMinSize = self.xdg.surface.client.parse(self, parser)?;
        if req.height < 0 || req.width < 0 {
            return Err(SetMinSizeError::NonNegative);
        }
        self.min_width.set(if req.width == 0 {
            None
        } else {
            Some(req.width)
        });
        self.min_height.set(if req.height == 0 {
            None
        } else {
            Some(req.height)
        });
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

    fn notify_parent(&self) {
        let parent = match self.parent_node.get() {
            Some(p) => p,
            _ => return,
        };
        let extents = self.xdg.extents.get();
        parent
            .clone()
            .child_active_changed(self, self.toplevel_data.active_surfaces.get() > 0);
        parent.child_size_changed(self, extents.width(), extents.height());
        parent.child_title_changed(self, self.title.borrow_mut().deref());
    }

    fn map_floating(self: &Rc<Self>, workspace: &Rc<WorkspaceNode>) {
        let extents = self.xdg.extents.get();
        let state = &self.xdg.surface.client.state;
        state.map_floating(self.clone(), extents.width(), extents.height(), workspace);
    }

    fn map_child(self: &Rc<Self>, parent: &XdgToplevel) {
        match parent.xdg.workspace.get() {
            Some(w) => self.map_floating(&w),
            _ => self.map_tiled(),
        }
    }

    fn map_tiled(self: &Rc<Self>) {
        let state = &self.xdg.surface.client.state;
        state.map_tiled(self.clone());
    }
}

object_base! {
    XdgToplevel, XdgToplevelError;

    DESTROY => destroy,
    SET_PARENT => set_parent,
    SET_TITLE => set_title,
    SET_APP_ID => set_app_id,
    SHOW_WINDOW_MENU => show_window_menu,
    MOVE => move_,
    RESIZE => resize,
    SET_MAX_SIZE => set_max_size,
    SET_MIN_SIZE => set_min_size,
    SET_MAXIMIZED => set_maximized,
    UNSET_MAXIMIZED => unset_maximized,
    SET_FULLSCREEN => set_fullscreen,
    UNSET_FULLSCREEN => unset_fullscreen,
    SET_MINIMIZED => set_minimized,
}

impl Object for XdgToplevel {
    fn num_requests(&self) -> u32 {
        SET_MINIMIZED + 1
    }

    fn break_loops(&self) {
        self.destroy_node(true);
        self.parent.set(None);
        let _children = mem::take(&mut *self.children.borrow_mut());
    }
}

dedicated_add_obj!(XdgToplevel, XdgToplevelId, xdg_toplevel);

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
                self.xdg.surface.client.state.tree_changed();
            }
        }
        self.toplevel_data.clear();
        self.xdg.destroy_node();
        self.xdg.seat_state.destroy_node(self)
    }

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_toplevel(&self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self.xdg.surface);
    }

    fn is_contained_in(&self, other: NodeId) -> bool {
        if let Some(parent) = self.parent_node.get() {
            if parent.id() == other {
                return true;
            }
            return parent.is_contained_in(other);
        }
        false
    }

    fn do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _direction: Direction) {
        seat.focus_toplevel(self);
    }

    fn absolute_position(&self) -> Rect {
        self.xdg.absolute_desired_extents.get()
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        self.xdg.find_tree_at(x, y, tree)
    }

    fn pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.enter_toplevel(self);
    }

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_xdg_surface(&self.xdg, x, y)
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        let nw = rect.width();
        let nh = rect.height();
        let de = self.xdg.absolute_desired_extents.get();
        if de.width() != nw || de.height() != nh {
            self.send_configure_checked(nw, nh);
            self.xdg.do_send_configure();
            self.xdg.surface.client.flush();
        }
        self.xdg.set_absolute_desired_extents(rect);
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        self.xdg.set_workspace(ws);
    }

    fn set_parent(self: Rc<Self>, parent: Rc<dyn Node>) {
        self.parent_node.set(Some(parent));
        self.notify_parent();
    }

    fn client(&self) -> Option<Rc<Client>> {
        Some(self.xdg.surface.client.clone())
    }
}

impl ToplevelNode for XdgToplevel {
    fn data(&self) -> &ToplevelData {
        &self.toplevel_data
    }

    fn parent(&self) -> Option<Rc<dyn Node>> {
        self.parent_node.get()
    }

    fn workspace(&self) -> Option<Rc<WorkspaceNode>> {
        self.xdg.workspace.get()
    }

    fn as_node(&self) -> &dyn Node {
        self
    }

    fn into_node(self: Rc<Self>) -> Rc<dyn Node> {
        self
    }

    fn accepts_keyboard_focus(&self) -> bool {
        true
    }

    fn default_surface(&self) -> Rc<WlSurface> {
        self.xdg.surface.clone()
    }

    fn set_active(&self, active: bool) {
        if let Some(parent) = self.parent_node.get() {
            parent.child_active_changed(self, active);
        }
        let changed = {
            let mut states = self.states.borrow_mut();
            match active {
                true => states.insert(STATE_ACTIVATED),
                false => states.remove(&STATE_ACTIVATED),
            }
        };
        if changed {
            let rect = self.xdg.absolute_desired_extents.get();
            self.send_configure_checked(rect.width(), rect.height());
            self.xdg.do_send_configure();
        }
    }

    fn activate(&self) {
        // nothing
    }

    fn toggle_floating(self: Rc<Self>) {
        let parent = match self.parent_node.get() {
            Some(p) => p,
            _ => return,
        };
        if parent.is_float() {
            parent.remove_child(&*self);
            self.map_tiled();
        } else if let Some(ws) = self.xdg.workspace.get() {
            parent.remove_child(&*self);
            self.map_floating(&ws);
        }
    }
}

impl XdgSurfaceExt for XdgToplevel {
    fn initial_configure(self: Rc<Self>) -> Result<(), XdgSurfaceError> {
        self.send_configure(0, 0);
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
            if let Some(workspace) = self.xdg.workspace.get() {
                let output = workspace.output.get();
                let bindings = output.global.bindings.borrow_mut();
                if let Some(binding) = bindings.get(&self.xdg.surface.client.id) {
                    for binding in binding.values() {
                        self.xdg.surface.send_enter(binding.id);
                    }
                }
            }
            {
                let seats = surface.client.state.globals.lock_seats();
                for seat in seats.values() {
                    seat.focus_toplevel(self.clone());
                }
            }
            surface.client.state.tree_changed();
        }
    }

    fn extents_changed(&self) {
        self.notify_parent();
        if self.parent_node.get().is_some() {
            self.xdg.surface.client.state.tree_changed();
        }
    }
}

#[derive(Debug, Error)]
pub enum XdgToplevelError {
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `set_parent` request")]
    SetParentError(#[from] SetParentError),
    #[error("Could not process `set_title` request")]
    SetTitleError(#[from] SetTitleError),
    #[error("Could not process `set_app_id` request")]
    SetAppIdError(#[from] SetAppIdError),
    #[error("Could not process `show_window_menu` request")]
    ShowWindowMenuError(#[from] ShowWindowMenuError),
    #[error("Could not process `move` request")]
    MoveError(#[from] MoveError),
    #[error("Could not process `resize` request")]
    ResizeError(#[from] ResizeError),
    #[error("Could not process `set_max_size` request")]
    SetMaxSizeError(#[from] SetMaxSizeError),
    #[error("Could not process `set_min_size` request")]
    SetMinSizeError(#[from] SetMinSizeError),
    #[error("Could not process `set_maximized` request")]
    SetMaximizedError(#[from] SetMaximizedError),
    #[error("Could not process `unset_maximized` request")]
    UnsetMaximizedError(#[from] UnsetMaximizedError),
    #[error("Could not process `set_fullscreen` request")]
    SetFullscreenError(#[from] SetFullscreenError),
    #[error("Could not process `unset_fullscreen` request")]
    UnsetFullscreenError(#[from] UnsetFullscreenError),
    #[error("Could not process `set_minimized` request")]
    SetMinimizedError(#[from] SetMinimizedError),
}

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum SetParentError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetParentError, ParseFailed, MsgParserError);
efrom!(SetParentError, ClientError);

#[derive(Debug, Error)]
pub enum SetTitleError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetTitleError, ParseFailed, MsgParserError);
efrom!(SetTitleError, ClientError);

#[derive(Debug, Error)]
pub enum SetAppIdError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetAppIdError, ParseFailed, MsgParserError);
efrom!(SetAppIdError, ClientError);

#[derive(Debug, Error)]
pub enum ShowWindowMenuError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ShowWindowMenuError, ParseFailed, MsgParserError);
efrom!(ShowWindowMenuError, ClientError);

#[derive(Debug, Error)]
pub enum MoveError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(MoveError, ParseFailed, MsgParserError);
efrom!(MoveError, ClientError);

#[derive(Debug, Error)]
pub enum ResizeError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ResizeError, ParseFailed, MsgParserError);
efrom!(ResizeError, ClientError);

#[derive(Debug, Error)]
pub enum SetMaxSizeError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("width/height must be non-negative")]
    NonNegative,
}
efrom!(SetMaxSizeError, ParseFailed, MsgParserError);
efrom!(SetMaxSizeError, ClientError);

#[derive(Debug, Error)]
pub enum SetMinSizeError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("width/height must be non-negative")]
    NonNegative,
}
efrom!(SetMinSizeError, ParseFailed, MsgParserError);
efrom!(SetMinSizeError, ClientError);

#[derive(Debug, Error)]
pub enum SetMaximizedError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetMaximizedError, ParseFailed, MsgParserError);
efrom!(SetMaximizedError, ClientError);

#[derive(Debug, Error)]
pub enum UnsetMaximizedError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(UnsetMaximizedError, ParseFailed, MsgParserError);
efrom!(UnsetMaximizedError, ClientError);

#[derive(Debug, Error)]
pub enum SetFullscreenError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetFullscreenError, ParseFailed, MsgParserError);
efrom!(SetFullscreenError, ClientError);

#[derive(Debug, Error)]
pub enum UnsetFullscreenError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(UnsetFullscreenError, ParseFailed, MsgParserError);
efrom!(UnsetFullscreenError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum SetMinimizedError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetMinimizedError, ParseFailed, MsgParserError);
efrom!(SetMinimizedError, ClientError, ClientError);
