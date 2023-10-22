use {
    crate::{
        bugs,
        bugs::Bugs,
        client::{Client, ClientError},
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            wl_seat::{NodeSeatState, SeatId, WlSeatGlobal},
            wl_surface::xdg_surface::{XdgSurface, XdgSurfaceError, XdgSurfaceExt},
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        renderer::Renderer,
        state::State,
        tree::{
            Direction, FindTreeResult, FoundNode, Node, NodeId, NodeVisitor, ToplevelData,
            ToplevelNode, ToplevelNodeId, WorkspaceNode,
        },
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{xdg_toplevel::*, XdgToplevelId},
    },
    ahash::{AHashMap, AHashSet},
    num_derive::FromPrimitive,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem,
        ops::Deref,
        rc::Rc,
    },
    thiserror::Error,
};

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

#[allow(dead_code)]
const CAP_WINDOW_MENU: u32 = 1;
#[allow(dead_code)]
const CAP_MAXIMIZE: u32 = 2;
const CAP_FULLSCREEN: u32 = 3;
#[allow(dead_code)]
const CAP_MINIMIZE: u32 = 4;

pub const WM_CAPABILITIES_SINCE: u32 = 5;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Decoration {
    #[allow(dead_code)]
    Client,
    Server,
}

pub struct XdgToplevel {
    pub id: XdgToplevelId,
    pub state: Rc<State>,
    pub xdg: Rc<XdgSurface>,
    pub node_id: ToplevelNodeId,
    pub parent: CloneCell<Option<Rc<XdgToplevel>>>,
    pub children: RefCell<AHashMap<XdgToplevelId, Rc<XdgToplevel>>>,
    states: RefCell<AHashSet<u32>>,
    pub decoration: Cell<Decoration>,
    bugs: Cell<&'static Bugs>,
    min_width: Cell<Option<i32>>,
    min_height: Cell<Option<i32>>,
    max_width: Cell<Option<i32>>,
    max_height: Cell<Option<i32>>,
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
        let state = &surface.surface.client.state;
        Self {
            id,
            state: state.clone(),
            xdg: surface.clone(),
            node_id: state.node_ids.next(),
            parent: Default::default(),
            children: RefCell::new(Default::default()),
            states: RefCell::new(states),
            decoration: Cell::new(Decoration::Server),
            bugs: Cell::new(&bugs::NONE),
            min_width: Cell::new(None),
            min_height: Cell::new(None),
            max_width: Cell::new(None),
            max_height: Cell::new(None),
            tracker: Default::default(),
            toplevel_data: ToplevelData::new(
                state,
                String::new(),
                Some(surface.surface.client.clone()),
            ),
        }
    }

    pub fn send_current_configure(&self) {
        let rect = self.xdg.absolute_desired_extents.get();
        self.send_configure_checked(rect.width(), rect.height());
        self.xdg.do_send_configure();
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

    fn send_close(&self) {
        self.xdg.surface.client.event(Close { self_id: self.id });
        // self.xdg.surface.client.flush();
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

    pub fn send_wm_capabilities(&self) {
        self.xdg.surface.client.event(WmCapabilities {
            self_id: self.id,
            capabilities: &[CAP_FULLSCREEN],
        })
    }

    fn destroy(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let _req: Destroy = self.xdg.surface.client.parse(self.deref(), parser)?;
        self.tl_destroy();
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

    fn set_parent(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let req: SetParent = self.xdg.surface.client.parse(self, parser)?;
        let mut parent = None;
        if req.parent.is_some() {
            parent = Some(self.xdg.surface.client.lookup(req.parent)?);
        }
        self.parent.set(parent);
        Ok(())
    }

    fn set_title(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let req: SetTitle = self.xdg.surface.client.parse(self, parser)?;
        *self.toplevel_data.title.borrow_mut() = req.title.to_string();
        self.tl_title_changed();
        Ok(())
    }

    fn set_app_id(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let req: SetAppId = self.xdg.surface.client.parse(self, parser)?;
        self.bugs.set(bugs::get(req.app_id));
        Ok(())
    }

    fn show_window_menu(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let _req: ShowWindowMenu = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn move_(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let req: Move = self.xdg.surface.client.parse(self, parser)?;
        let seat = self.xdg.surface.client.lookup(req.seat)?;
        if let Some(parent) = self.toplevel_data.parent.get() {
            if let Some(float) = parent.node_into_float() {
                seat.move_(&float);
            }
        }
        Ok(())
    }

    fn resize(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let _req: Resize = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn set_max_size(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let req: SetMaxSize = self.xdg.surface.client.parse(self, parser)?;
        if req.height < 0 || req.width < 0 {
            return Err(XdgToplevelError::NonNegative);
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

    fn set_min_size(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let req: SetMinSize = self.xdg.surface.client.parse(self, parser)?;
        if req.height < 0 || req.width < 0 {
            return Err(XdgToplevelError::NonNegative);
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

    fn set_maximized(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let _req: SetMaximized = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn unset_maximized(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let _req: UnsetMaximized = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn set_fullscreen(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let client = &self.xdg.surface.client;
        let req: SetFullscreen = client.parse(self.deref(), parser)?;
        self.states.borrow_mut().insert(STATE_FULLSCREEN);
        'set_fullscreen: {
            let output = if req.output.is_some() {
                match client.lookup(req.output)?.global.node.get() {
                    Some(node) => node,
                    _ => {
                        log::error!("Output global has no node attached");
                        break 'set_fullscreen;
                    }
                }
            } else if let Some(ws) = self.xdg.workspace.get() {
                ws.output.get()
            } else {
                break 'set_fullscreen;
            };
            self.toplevel_data
                .set_fullscreen(&client.state, self.clone(), &output);
        }
        self.send_current_configure();
        Ok(())
    }

    fn unset_fullscreen(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), XdgToplevelError> {
        let _req: UnsetFullscreen = self.xdg.surface.client.parse(self.deref(), parser)?;
        self.states.borrow_mut().remove(&STATE_FULLSCREEN);
        self.toplevel_data
            .unset_fullscreen(&self.state, self.clone());
        self.send_current_configure();
        Ok(())
    }

    fn set_minimized(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelError> {
        let _req: SetMinimized = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn map_floating(self: &Rc<Self>, workspace: &Rc<WorkspaceNode>) {
        let (width, height) = self.toplevel_data.float_size(workspace);
        self.state
            .map_floating(self.clone(), width, height, workspace);
    }

    fn map_child(self: &Rc<Self>, parent: &XdgToplevel) {
        match parent.xdg.workspace.get() {
            Some(w) => self.map_floating(&w),
            _ => self.map_tiled(),
        }
    }

    fn map_tiled(self: &Rc<Self>) {
        self.state.map_tiled(self.clone());
    }
}

object_base! {
    XdgToplevel;

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
        self.tl_destroy();
        self.parent.set(None);
        let _children = mem::take(&mut *self.children.borrow_mut());
    }
}

dedicated_add_obj!(XdgToplevel, XdgToplevelId, xdg_toplevel);

impl Node for XdgToplevel {
    fn node_id(&self) -> NodeId {
        self.node_id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.toplevel_data.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_toplevel(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self.xdg.surface);
    }

    fn node_visible(&self) -> bool {
        self.xdg.surface.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.xdg.absolute_desired_extents.get()
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _direction: Direction) {
        seat.focus_toplevel(self.clone());
    }

    fn node_find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        self.xdg.find_tree_at(x, y, tree)
    }

    fn node_render(
        &self,
        renderer: &mut Renderer,
        x: i32,
        y: i32,
        max_width: i32,
        max_height: i32,
    ) {
        renderer.render_xdg_surface(&self.xdg, x, y, max_width, max_height)
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        Some(self.xdg.surface.client.clone())
    }

    fn node_toplevel(self: Rc<Self>) -> Option<Rc<dyn crate::tree::ToplevelNode>> {
        Some(self)
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.enter_toplevel(self.clone());
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("xdg-toplevel focus");
        seat.set_known_cursor(KnownCursor::Default);
    }
}

impl ToplevelNode for XdgToplevel {
    tl_node_impl!();

    fn tl_data(&self) -> &ToplevelData {
        &self.toplevel_data
    }

    fn tl_set_active(&self, active: bool) {
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

    fn tl_focus_child(&self, _seat: SeatId) -> Option<Rc<dyn Node>> {
        Some(self.xdg.surface.clone())
    }

    fn tl_set_workspace_ext(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        self.xdg.set_workspace(ws);
    }

    fn tl_change_extents(self: Rc<Self>, rect: &Rect) {
        let nw = rect.width();
        let nh = rect.height();
        let de = self.xdg.absolute_desired_extents.get();
        if de.width() != nw || de.height() != nh {
            if self.toplevel_data.is_floating.get() {
                self.toplevel_data.float_width.set(rect.width());
                self.toplevel_data.float_height.set(rect.height());
            }
            self.send_configure_checked(nw, nh);
            self.xdg.do_send_configure();
            // self.xdg.surface.client.flush();
        }
        self.xdg.set_absolute_desired_extents(rect);
    }

    fn tl_close(self: Rc<Self>) {
        self.send_close();
    }

    fn tl_set_visible(&self, visible: bool) {
        // log::info!("set_visible {}", visible);
        // if !visible {
        //     log::info!("\n{:?}", Backtrace::new());
        // }
        self.xdg.set_visible(visible);
        self.toplevel_data.set_visible(self, visible);
    }

    fn tl_destroy(&self) {
        self.toplevel_data.destroy_node(self);
        self.xdg.destroy_node();
    }

    // fn move_to_workspace(self: &Rc<Self>, workspace: &Rc<WorkspaceNode>) {
    //     let parent = match self.parent_node.get() {
    //         Some(p) => p,
    //         _ => return,
    //     };
    //     if self.fullscreen_data.is_fullscreen.get() {
    //         if workspace.fullscreen.get().is_some() {
    //             log::info!("Not moving fullscreen node to workspace {} because that workspace already contains a fullscreen node", workspace.name);
    //             return;
    //         }
    //         parent.node_remove_child2(self.deref(), workspace.visible());
    //         workspace.fullscreen.set(Some(self.clone()));
    //         self.state.tree_changed();
    //         return;
    //     }
    //     parent.node_remove_child2(self.deref(), workspace.visible());
    //     if self.toplevel_data.is_floating.get() {
    //         self.map_floating(workspace);
    //     } else {
    //         self.map_tiled()
    //     }
    // }
}

impl XdgSurfaceExt for XdgToplevel {
    fn initial_configure(self: Rc<Self>) -> Result<(), XdgSurfaceError> {
        self.send_configure(0, 0);
        Ok(())
    }

    fn post_commit(self: Rc<Self>) {
        let surface = &self.xdg.surface;
        if self.toplevel_data.parent.get().is_some() {
            if surface.buffer.get().is_none() {
                self.tl_destroy();
                {
                    let new_parent = self.parent.get();
                    let mut children = self.children.borrow_mut();
                    for (_, child) in children.drain() {
                        child.parent.set(new_parent.clone());
                    }
                }
                self.state.tree_changed();
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
                surface.set_output(&output);
            }
            // {
            //     let seats = surface.client.state.globals.lock_seats();
            //     for seat in seats.values() {
            //         seat.focus_toplevel(self.clone());
            //     }
            // }
            self.state.tree_changed();
        }
    }

    fn extents_changed(&self) {
        self.toplevel_data.pos.set(self.xdg.extents.get());
        self.tl_extents_changed();
    }
}

#[derive(Debug, Error)]
pub enum XdgToplevelError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("width/height must be non-negative")]
    NonNegative,
}
efrom!(XdgToplevelError, MsgParserError);
efrom!(XdgToplevelError, ClientError);
