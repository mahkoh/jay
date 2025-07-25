pub mod xdg_dialog_v1;

use {
    crate::{
        bugs,
        bugs::Bugs,
        client::{Client, ClientError},
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
            wl_seat::{NodeSeatState, WlSeatGlobal, tablet::TabletTool},
            wl_surface::{
                WlSurface,
                xdg_surface::{
                    XdgSurface, XdgSurfaceError, XdgSurfaceExt,
                    xdg_toplevel::xdg_dialog_v1::XdgDialogV1,
                },
            },
            xdg_toplevel_drag_v1::XdgToplevelDragV1,
            zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        rect::Rect,
        renderer::Renderer,
        state::State,
        tree::{
            ContainerSplit, Direction, FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId,
            NodeLayerLink, NodeLocation, NodeVisitor, OutputNode, TileDragDestination,
            ToplevelData, ToplevelNode, ToplevelNodeBase, ToplevelNodeId, ToplevelType,
            WorkspaceNode, default_tile_drag_destination,
        },
        utils::{clonecell::CloneCell, hash_map_ext::HashMapExt},
        wire::{XdgToplevelId, xdg_toplevel::*},
    },
    ahash::{AHashMap, AHashSet},
    jay_config::window::TileState,
    num_derive::FromPrimitive,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem,
        rc::{Rc, Weak},
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

#[expect(dead_code)]
const STATE_MAXIMIZED: u32 = 1;
const STATE_FULLSCREEN: u32 = 2;
#[expect(dead_code)]
const STATE_RESIZING: u32 = 3;
const STATE_ACTIVATED: u32 = 4;
const STATE_TILED_LEFT: u32 = 5;
const STATE_TILED_RIGHT: u32 = 6;
const STATE_TILED_TOP: u32 = 7;
const STATE_TILED_BOTTOM: u32 = 8;
pub const STATE_SUSPENDED: u32 = 9;
const STATE_CONSTRAINED_LEFT: u32 = 10;
const STATE_CONSTRAINED_RIGHT: u32 = 11;
const STATE_CONSTRAINED_TOP: u32 = 12;
const STATE_CONSTRAINED_BOTTOM: u32 = 13;

#[expect(dead_code)]
const CAP_WINDOW_MENU: u32 = 1;
#[expect(dead_code)]
const CAP_MAXIMIZE: u32 = 2;
const CAP_FULLSCREEN: u32 = 3;
#[expect(dead_code)]
const CAP_MINIMIZE: u32 = 4;

pub const WM_CAPABILITIES_SINCE: Version = Version(5);
pub const SUSPENDED_SINCE: Version = Version(6);
pub const CONSTRAINTS_SINCE: Version = Version(7);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Decoration {
    #[expect(dead_code)]
    Client,
    Server,
}

#[derive(Debug)]
pub struct XdgToplevelToplevelData {
    pub tag: RefCell<String>,
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
    pub drag: CloneCell<Option<Rc<XdgToplevelDragV1>>>,
    is_mapped: Cell<bool>,
    dialog: CloneCell<Option<Rc<XdgDialogV1>>>,
    extents_set: Cell<bool>,
    pub data: Rc<XdgToplevelToplevelData>,
}

impl Debug for XdgToplevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XdgToplevel").finish_non_exhaustive()
    }
}

impl XdgToplevel {
    pub fn new(id: XdgToplevelId, surface: &Rc<XdgSurface>, slf: &Weak<Self>) -> Self {
        let mut states = AHashSet::new();
        states.insert(STATE_TILED_LEFT);
        states.insert(STATE_TILED_RIGHT);
        states.insert(STATE_TILED_TOP);
        states.insert(STATE_TILED_BOTTOM);
        if surface.base.version >= CONSTRAINTS_SINCE {
            states.insert(STATE_CONSTRAINED_LEFT);
            states.insert(STATE_CONSTRAINED_RIGHT);
            states.insert(STATE_CONSTRAINED_TOP);
            states.insert(STATE_CONSTRAINED_BOTTOM);
        }
        let state = &surface.surface.client.state;
        let node_id = state.node_ids.next();
        let data = Rc::new(XdgToplevelToplevelData {
            tag: Default::default(),
        });
        let toplevel_data = ToplevelData::new(
            state,
            String::new(),
            Some(surface.surface.client.clone()),
            ToplevelType::XdgToplevel(data.clone()),
            node_id,
            slf,
        );
        toplevel_data
            .content_type
            .set(surface.surface.content_type.get());
        toplevel_data.pos.set(surface.extents.get());
        Self {
            id,
            state: state.clone(),
            xdg: surface.clone(),
            node_id,
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
            toplevel_data,
            drag: Default::default(),
            is_mapped: Cell::new(false),
            dialog: Default::default(),
            extents_set: Cell::new(false),
            data,
        }
    }

    pub fn send_to(self: &Rc<Self>, list: &ExtForeignToplevelListV1) {
        self.toplevel_data.send(self.clone(), list);
    }

    pub fn manager_send_to(self: &Rc<Self>, manager: &ZwlrForeignToplevelManagerV1) {
        self.toplevel_data.manager_send(self.clone(), manager);
    }

    pub fn send_current_configure(&self) {
        if self.drag.is_none() {
            let rect = self.xdg.absolute_desired_extents.get();
            self.send_configure_checked(rect.width(), rect.height());
        }
        self.xdg.schedule_configure();
    }

    fn send_configure_checked(&self, mut width: i32, mut height: i32) {
        if self.extents_set.get() {
            width = width.max(1);
            height = height.max(1);
        }
        let bugs = self.bugs.get();
        macro_rules! apply {
            ($field:expr, $min:ident, $max:ident) => {
                if $field != 0 {
                    if let Some(min) = bugs.$min {
                        $field = $field.max(min);
                    }
                    if bugs.respect_min_max_size {
                        if let Some(min) = self.$min.get() {
                            $field = $field.max(min);
                        }
                        if let Some(max) = self.$max.get() {
                            $field = $field.min(max);
                        }
                    }
                }
            };
        }
        apply!(width, min_width, max_width);
        apply!(height, min_height, max_height);
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
}

impl XdgToplevelRequestHandler for XdgToplevel {
    type Error = XdgToplevelError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.tl_destroy();
        self.xdg.ext.set(None);
        {
            let mut children = self.children.borrow_mut();
            let parent = self.parent.get();
            let mut parent_children = match &parent {
                Some(p) => Some(p.children.borrow_mut()),
                _ => None,
            };
            for child in children.drain_values() {
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
        self.xdg.surface.client.remove_obj(self)?;
        self.xdg.surface.set_toplevel(None);
        Ok(())
    }

    fn set_parent(&self, req: SetParent, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let mut parent = None;
        if req.parent.is_some() {
            parent = Some(self.xdg.surface.client.lookup(req.parent)?);
        }
        self.parent.set(parent);
        Ok(())
    }

    fn set_title(&self, req: SetTitle, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.toplevel_data.set_title(req.title);
        self.tl_title_changed();
        Ok(())
    }

    fn set_app_id(&self, req: SetAppId, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.toplevel_data.set_app_id(req.app_id);
        self.bugs.set(bugs::get(req.app_id));
        Ok(())
    }

    fn show_window_menu(&self, _req: ShowWindowMenu, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn move_(&self, _req: Move, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn resize(&self, _req: Resize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_max_size(&self, req: SetMaxSize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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

    fn set_min_size(&self, req: SetMinSize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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

    fn set_maximized(&self, _req: SetMaximized, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn unset_maximized(&self, _req: UnsetMaximized, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_fullscreen(&self, req: SetFullscreen, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let client = &self.xdg.surface.client;
        self.states.borrow_mut().insert(STATE_FULLSCREEN);
        'set_fullscreen: {
            let output = if req.output.is_some() {
                match client.lookup(req.output)?.global.node() {
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
                .set_fullscreen(&client.state, slf.clone(), &output);
        }
        self.send_current_configure();
        Ok(())
    }

    fn unset_fullscreen(&self, _req: UnsetFullscreen, slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.states.borrow_mut().remove(&STATE_FULLSCREEN);
        self.toplevel_data
            .unset_fullscreen(&self.state, slf.clone());
        self.send_current_configure();
        Ok(())
    }

    fn set_minimized(&self, _req: SetMinimized, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl XdgToplevel {
    fn map(
        self: &Rc<Self>,
        parent: Option<&XdgToplevel>,
        pos: Option<(&Rc<OutputNode>, i32, i32)>,
    ) {
        if let Some(state) = self.state.initial_tile_state(&self.toplevel_data) {
            match state {
                TileState::Floating => {
                    let mut ws = None;
                    if let Some(parent) = parent {
                        ws = parent.xdg.workspace.get();
                    }
                    let ws = ws.unwrap_or_else(|| self.state.ensure_map_workspace(None));
                    self.map_floating(&ws, pos.map(|p| (p.1, p.2)));
                }
                _ => self.map_tiled(),
            }
            return;
        }
        match parent {
            None => self.map_tiled(),
            Some(p) => self.map_child(p, pos),
        }
    }

    fn map_floating(self: &Rc<Self>, workspace: &Rc<WorkspaceNode>, abs_pos: Option<(i32, i32)>) {
        let (width, height) = self.toplevel_data.float_size(workspace);
        self.state
            .map_floating(self.clone(), width, height, workspace, abs_pos);
    }

    fn map_child(self: &Rc<Self>, parent: &XdgToplevel, pos: Option<(&Rc<OutputNode>, i32, i32)>) {
        if let Some((output, x, y)) = pos {
            let w = output.ensure_workspace();
            self.map_floating(&w, Some((x, y)));
            return;
        }
        match parent.xdg.workspace.get() {
            Some(w) => self.map_floating(&w, None),
            _ => self.map_tiled(),
        }
    }

    fn map_tiled(self: &Rc<Self>) {
        self.state.map_tiled(self.clone());
        let fullscreen = self.states.borrow().contains(&STATE_FULLSCREEN);
        if fullscreen && let Some(ws) = self.xdg.workspace.get() {
            self.toplevel_data
                .set_fullscreen2(&self.state, self.clone(), &ws);
        }
    }

    pub fn prepare_toplevel_drag(&self) {
        if self.toplevel_data.parent.get().is_none() {
            return;
        }
        self.toplevel_data.detach_node(self);
        self.xdg.detach_node();
        self.tl_set_visible(self.state.root_visible());
    }

    pub fn after_toplevel_drag(self: &Rc<Self>, output: &Rc<OutputNode>, x: i32, y: i32) {
        assert!(self.toplevel_data.parent.is_none());
        if self.node_visible() {
            self.xdg.damage();
        }
        let extents = match self.xdg.geometry.get() {
            None => self.xdg.extents.get(),
            Some(g) => g,
        };
        self.toplevel_data.float_width.set(extents.width());
        self.toplevel_data.float_height.set(extents.height());
        self.clone().after_commit(Some((output, x, y)));
    }

    fn after_commit(self: &Rc<Self>, pos: Option<(&Rc<OutputNode>, i32, i32)>) {
        if pos.is_some() {
            self.is_mapped.set(false);
        }
        let surface = &self.xdg.surface;
        let should_be_mapped = surface.buffer.is_some();
        if let Some(drag) = self.drag.get()
            && drag.is_ongoing()
        {
            if should_be_mapped {
                if !self.is_mapped.replace(true) {
                    if let Some(seat) = drag.source.data.seat.get() {
                        self.xdg.set_output(&seat.get_output());
                    }
                    self.toplevel_data.broadcast(self.clone());
                    self.tl_set_visible(self.state.root_visible());
                    self.xdg.damage();
                }
                self.extents_changed();
            } else {
                if self.is_mapped.replace(false) {
                    self.tl_set_visible(false);
                    self.xdg.damage();
                }
            }
            return;
        }
        if self.is_mapped.replace(should_be_mapped) == should_be_mapped {
            return;
        }
        if !should_be_mapped {
            self.tl_destroy();
            {
                let new_parent = self.parent.get();
                let mut children = self.children.borrow_mut();
                for child in children.drain_values() {
                    child.parent.set(new_parent.clone());
                }
            }
            self.state.tree_changed();
        } else {
            self.map(self.parent.get().as_deref(), pos);
            self.extents_changed();
            if let Some(workspace) = self.xdg.workspace.get() {
                let output = workspace.output.get();
                surface.set_output(&output, workspace.location());
            }
            // {
            //     let seats = surface.client.state.globals.lock_seats();
            //     for seat in seats.values() {
            //         seat.focus_toplevel(self.clone());
            //     }
            // }
            self.state.tree_changed();
            self.toplevel_data.broadcast(self.clone());
        }
        self.toplevel_data
            .set_content_type(self.xdg.surface.content_type.get());
    }
}

object_base! {
    self = XdgToplevel;
    version = self.xdg.base.version;
}

impl Object for XdgToplevel {
    fn break_loops(&self) {
        self.tl_destroy();
        self.parent.set(None);
        self.dialog.set(None);
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

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        self.toplevel_data.output_opt()
    }

    fn node_location(&self) -> Option<NodeLocation> {
        self.xdg.surface.node_location()
    }

    fn node_layer(&self) -> NodeLayerLink {
        self.toplevel_data.node_layer()
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _direction: Direction) {
        seat.focus_toplevel(self.clone());
    }

    fn node_active_changed(&self, active: bool) {
        self.toplevel_data.update_self_active(self, active);
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        if usecase == FindTreeUsecase::SelectToplevel {
            return FindTreeResult::AcceptsInput;
        }
        self.xdg.find_tree_at(x, y, tree)
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, bounds: Option<&Rect>) {
        renderer.render_xdg_toplevel(self, x, y, bounds)
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        Some(self.xdg.surface.client.clone())
    }

    fn node_toplevel(self: Rc<Self>) -> Option<Rc<dyn crate::tree::ToplevelNode>> {
        Some(self)
    }

    fn node_make_visible(self: Rc<Self>) {
        self.toplevel_data.make_visible(&*self)
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.enter_toplevel(self.clone());
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("xdg-toplevel focus");
        seat.pointer_cursor().set_known(KnownCursor::Default);
    }

    fn node_on_tablet_tool_enter(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        _time_usec: u64,
        _x: Fixed,
        _y: Fixed,
    ) {
        tool.cursor().set_known(KnownCursor::Default)
    }

    fn node_into_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        Some(self)
    }
}

impl ToplevelNodeBase for XdgToplevel {
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
            self.xdg.schedule_configure();
        }
    }

    fn tl_focus_child(&self) -> Option<Rc<dyn Node>> {
        Some(self.xdg.surface.clone())
    }

    fn tl_set_workspace_ext(&self, ws: &Rc<WorkspaceNode>) {
        self.xdg.set_workspace(ws);
    }

    fn tl_change_extents_impl(self: Rc<Self>, rect: &Rect) {
        self.extents_set.set(true);
        let nw = rect.width();
        let nh = rect.height();
        let de = self.xdg.absolute_desired_extents.get();
        if de.width() != nw || de.height() != nh {
            self.send_configure_checked(nw, nh);
            self.xdg.schedule_configure();
            // self.xdg.surface.client.flush();
        }
        self.xdg.set_absolute_desired_extents(rect);
    }

    fn tl_close(self: Rc<Self>) {
        self.send_close();
    }

    fn tl_set_visible_impl(&self, visible: bool) {
        // log::info!("set_visible {}", visible);
        // if !visible {
        //     log::info!("\n{:?}", Backtrace::new());
        // }
        self.xdg.set_visible(visible);
        if self.xdg.base.version >= SUSPENDED_SINCE {
            if visible {
                self.states.borrow_mut().remove(&STATE_SUSPENDED);
            } else {
                self.states.borrow_mut().insert(STATE_SUSPENDED);
            }
            self.send_current_configure();
        }
    }

    fn tl_destroy_impl(&self) {
        if let Some(drag) = self.drag.take() {
            self.xdg.damage();
            drag.toplevel.take();
        }
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

    fn tl_last_active_child(self: Rc<Self>) -> Rc<dyn ToplevelNode> {
        self
    }

    fn tl_scanout_surface(&self) -> Option<Rc<WlSurface>> {
        Some(self.xdg.surface.clone())
    }

    fn tl_restack_popups(&self) {
        self.xdg.restack_popups();
    }

    fn tl_admits_children(&self) -> bool {
        false
    }

    fn tl_tile_drag_destination(
        self: Rc<Self>,
        source: NodeId,
        split: Option<ContainerSplit>,
        abs_bounds: Rect,
        x: i32,
        y: i32,
    ) -> Option<TileDragDestination> {
        default_tile_drag_destination(self, source, split, abs_bounds, x, y)
    }
}

impl XdgSurfaceExt for XdgToplevel {
    fn initial_configure(self: Rc<Self>) -> Result<(), XdgSurfaceError> {
        let rect = self.xdg.absolute_desired_extents.get();
        if rect.is_empty() {
            self.send_configure(0, 0);
        } else {
            self.send_configure_checked(rect.width(), rect.height());
        }
        Ok(())
    }

    fn post_commit(self: Rc<Self>) {
        self.after_commit(None);
    }

    fn extents_changed(&self) {
        self.toplevel_data.pos.set(self.xdg.extents.get());
        self.tl_extents_changed();
    }

    fn geometry_changed(&self) {
        self.xdg
            .surface
            .client
            .state
            .damage(self.node_absolute_position());
    }

    fn make_visible(self: Rc<Self>) {
        self.node_make_visible();
    }

    fn node_layer(&self) -> NodeLayerLink {
        self.toplevel_data.node_layer()
    }
}

#[derive(Debug, Error)]
pub enum XdgToplevelError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("width/height must be non-negative")]
    NonNegative,
}
efrom!(XdgToplevelError, ClientError);
