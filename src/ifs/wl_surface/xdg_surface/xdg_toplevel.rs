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
                    InitialCommitState, XdgSurface, XdgSurfaceConfigureData, XdgSurfaceExt,
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
            ToplevelData, ToplevelNode, ToplevelNodeBase, ToplevelNodeId, ToplevelType, TreeSerial,
            WorkspaceNode, default_tile_drag_destination,
            transaction::{TreeTransaction, TreeTransactionOp},
        },
        utils::{
            bitflags::BitflagsExt, clonecell::CloneCell, hash_map_ext::HashMapExt, numcell::NumCell,
        },
        wire::{XdgToplevelId, xdg_toplevel::*},
    },
    ahash::AHashMap,
    arrayvec::ArrayVec,
    jay_config::window::TileState,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem,
        rc::{Rc, Weak},
    },
    thiserror::Error,
};

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

const fn state_bits(state: u32) -> u32 {
    1 << (state - 1)
}

#[expect(dead_code)]
const CAP_WINDOW_MENU: u32 = 1;
#[expect(dead_code)]
const CAP_MAXIMIZE: u32 = 2;
const CAP_FULLSCREEN: u32 = 3;
#[expect(dead_code)]
const CAP_MINIMIZE: u32 = 4;

pub const WM_CAPABILITIES_SINCE: Version = Version(5);
pub const SUSPENDED_SINCE: Version = Version(6);
pub const TILED_SINCE: Version = Version(2);
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
    states: NumCell<u32>,
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
    mapped_fullscreen: Cell<bool>,
}

impl Debug for XdgToplevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XdgToplevel").finish_non_exhaustive()
    }
}

impl XdgToplevel {
    pub fn new(id: XdgToplevelId, surface: &Rc<XdgSurface>, slf: &Weak<Self>) -> Self {
        let mut states = 0;
        if surface.base.version >= TILED_SINCE {
            states |= state_bits(STATE_TILED_LEFT);
            states |= state_bits(STATE_TILED_RIGHT);
            states |= state_bits(STATE_TILED_TOP);
            states |= state_bits(STATE_TILED_BOTTOM);
        } else {
            states |= state_bits(STATE_MAXIMIZED);
        }
        if surface.base.version >= CONSTRAINTS_SINCE {
            states |= state_bits(STATE_CONSTRAINED_LEFT);
            states |= state_bits(STATE_CONSTRAINED_RIGHT);
            states |= state_bits(STATE_CONSTRAINED_TOP);
            states |= state_bits(STATE_CONSTRAINED_BOTTOM);
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
        toplevel_data.content_size.set(surface.extents.get());
        Self {
            id,
            state: state.clone(),
            xdg: surface.clone(),
            node_id,
            parent: Default::default(),
            children: RefCell::new(Default::default()),
            states: NumCell::new(states),
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
            mapped_fullscreen: Cell::new(false),
        }
    }

    pub fn send_to(self: &Rc<Self>, list: &ExtForeignToplevelListV1) {
        self.toplevel_data.send(self.clone(), list);
    }

    pub fn manager_send_to(self: &Rc<Self>, manager: &ZwlrForeignToplevelManagerV1) {
        self.toplevel_data.manager_send(self.clone(), manager);
    }

    fn send_close(&self) {
        self.xdg.surface.client.event(Close { self_id: self.id });
        // self.xdg.surface.client.flush();
    }

    fn send_configure(&self, width: i32, height: i32, mut state_bits: u32) {
        let mut states = ArrayVec::<u32, 24>::new();
        while state_bits != 0 {
            let ts = state_bits.trailing_zeros();
            states.push(ts + 1);
            state_bits &= !(1 << ts);
        }
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

    fn add_op(self: &Rc<Self>, tt: &TreeTransaction, op: XdgToplevelTreeOpKind) {
        tt.add_op(
            &self.toplevel_data.transaction_timeline,
            XdgToplevelTreeOp {
                tl: self.clone(),
                kind: op,
            },
        );
    }
}

impl XdgToplevelRequestHandler for XdgToplevel {
    type Error = XdgToplevelError;

    fn destroy(&self, _req: Destroy, slf: &Rc<Self>) -> Result<(), Self::Error> {
        slf.clone().tl_destroy();
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
        self.bugs.set(bugs::get_by_app_id(req.app_id));
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
        self.states.or_assign(state_bits(STATE_FULLSCREEN));
        let tt = &self.state.tree_transaction();
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
                ws.current.output.get()
            } else {
                break 'set_fullscreen;
            };
            self.toplevel_data
                .set_fullscreen(&client.state, tt, slf.clone(), &output);
        }
        self.xdg.request_configure(tt);
        Ok(())
    }

    fn unset_fullscreen(&self, _req: UnsetFullscreen, slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.states.and_assign(!state_bits(STATE_FULLSCREEN));
        let tt = &self.state.tree_transaction();
        self.toplevel_data
            .unset_fullscreen(&self.state, tt, slf.clone());
        self.xdg.request_configure(tt);
        Ok(())
    }

    fn set_minimized(&self, _req: SetMinimized, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl XdgToplevel {
    fn map(
        self: &Rc<Self>,
        tt: &TreeTransaction,
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
                    let ws = ws.unwrap_or_else(|| self.state.ensure_map_workspace(tt, None));
                    self.map_floating(tt, &ws, pos.map(|p| (p.1, p.2)));
                }
                _ => self.map_tiled(tt),
            }
            return;
        }
        match parent {
            None => self.map_tiled(tt),
            Some(p) => self.map_child(tt, p, pos),
        }
    }

    fn map_floating(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        workspace: &Rc<WorkspaceNode>,
        abs_pos: Option<(i32, i32)>,
    ) {
        let (width, height) = self.toplevel_data.float_size(workspace);
        self.state
            .map_floating(&tt, self.clone(), width, height, workspace, abs_pos);
    }

    fn map_child(
        self: &Rc<Self>,
        tt: &TreeTransaction,
        parent: &XdgToplevel,
        pos: Option<(&Rc<OutputNode>, i32, i32)>,
    ) {
        if let Some((output, x, y)) = pos {
            let w = output.ensure_workspace(tt);
            self.map_floating(tt, &w, Some((x, y)));
            return;
        }
        match parent.xdg.workspace.get() {
            Some(w) => self.map_floating(tt, &w, None),
            _ => self.map_tiled(tt),
        }
    }

    fn map_tiled(self: &Rc<Self>, tt: &TreeTransaction) {
        self.state.map_tiled(tt, self.clone());
        let fullscreen = self.states.get().contains(STATE_FULLSCREEN);
        if fullscreen && let Some(ws) = self.xdg.workspace.get() {
            self.toplevel_data
                .set_fullscreen2(&self.state, tt, self.clone(), &ws);
        }
    }

    pub fn prepare_toplevel_drag(self: &Rc<Self>) {
        if self.toplevel_data.parent.get().is_none() {
            return;
        }
        let tt = &self.state.tree_transaction();
        self.toplevel_data.detach_node(self.clone());
        self.xdg.detach_node();
        self.clone().tl_set_visible(tt, self.state.root_visible());
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
                    let tt = &self.state.tree_transaction();
                    self.clone().tl_set_visible(tt, self.state.root_visible());
                    self.xdg.damage();
                }
                self.extents_changed();
            } else {
                if self.is_mapped.replace(false) {
                    let tt = &self.state.tree_transaction();
                    self.clone().tl_set_visible(tt, false);
                    self.xdg.damage();
                }
            }
            return;
        }
        if self.is_mapped.replace(should_be_mapped) == should_be_mapped {
            return;
        }
        if !should_be_mapped {
            self.clone().tl_destroy();
            {
                let new_parent = self.parent.get();
                let mut children = self.children.borrow_mut();
                for child in children.drain_values() {
                    child.parent.set(new_parent.clone());
                }
            }
            self.state.tree_changed();
        } else {
            let tt = &self.state.tree_transaction();
            self.map(tt, self.parent.get().as_deref(), pos);
            self.extents_changed();
            if let Some(workspace) = self.xdg.workspace.get() {
                let output = workspace.current.output.get();
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
    fn break_loops(self: Rc<Self>) {
        self.clone().tl_destroy();
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

    fn node_mapped_position(&self) -> Rect {
        self.xdg.absolute_extents.get()
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

    fn node_make_visible(self: Rc<Self>, tt: &TreeTransaction) {
        self.toplevel_data.make_visible(&*self, tt)
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
            let old = self.states.get();
            match active {
                true => self.states.or_assign(state_bits(STATE_ACTIVATED)),
                false => self.states.and_assign(!state_bits(STATE_ACTIVATED)),
            }
            old != self.states.get()
        };
        if changed {
            self.xdg.schedule_configure();
        }
    }

    fn tl_focus_child(&self) -> Option<Rc<dyn Node>> {
        Some(self.xdg.surface.clone())
    }

    fn tl_set_workspace_ext(&self, ws: &Rc<WorkspaceNode>) {
        self.xdg.set_workspace(ws);
    }

    fn tl_set_mapped_position_impl(self: Rc<Self>, rect: &Rect) {
        self.extents_set.set(true);
        self.xdg.set_absolute_extents(rect);
    }

    fn tl_request_config_impl(self: Rc<Self>, tt: &TreeTransaction, _rect: &Rect) {
        self.xdg.request_configure(tt);
    }

    fn tl_close(self: Rc<Self>) {
        self.send_close();
    }

    fn tl_set_visible_impl(&self, tt: &TreeTransaction, visible: bool) {
        // log::info!("set_visible {}", visible);
        // if !visible {
        //     log::info!("\n{:?}", Backtrace::new());
        // }
        self.xdg.set_visible(visible);
        if self.xdg.base.version >= SUSPENDED_SINCE {
            if visible {
                self.states.and_assign(!state_bits(STATE_SUSPENDED));
            } else {
                self.states.or_assign(state_bits(STATE_SUSPENDED));
            }
            self.xdg.request_configure(tt);
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

    fn tl_mark_fullscreen_ext(self: Rc<Self>, tt: &TreeTransaction) {
        self.add_op(
            tt,
            XdgToplevelTreeOpKind::MarkFullscreen(self.toplevel_data.is_fullscreen.get()),
        );
    }
}

impl XdgSurfaceExt for XdgToplevel {
    fn post_commit(self: Rc<Self>) {
        self.after_commit(None);
    }

    fn extents_changed(&self) {
        self.toplevel_data.content_size.set(self.xdg.extents.get());
        self.tl_extents_changed();
    }

    fn geometry_changed(&self) {
        self.xdg
            .surface
            .client
            .state
            .damage(self.node_mapped_position());
    }

    fn effective_geometry(&self, geometry: Rect) -> Rect {
        if !self.mapped_fullscreen.get() {
            return geometry;
        }
        let output = self
            .toplevel_data
            .output()
            .node_mapped_position()
            .at_point(0, 0);
        let x_overflow = output.width() - geometry.width();
        let y_overflow = output.height() - geometry.height();
        output.at_point(
            geometry.x1() - x_overflow / 2,
            geometry.y1() - y_overflow / 2,
        )
    }

    fn make_visible(self: Rc<Self>, tt: &TreeTransaction) {
        self.node_make_visible(tt);
    }

    fn node_layer(&self) -> NodeLayerLink {
        self.toplevel_data.node_layer()
    }

    fn configure_data(&self) -> XdgSurfaceConfigureData {
        let initial = self.xdg.initial_commit_state.get() == InitialCommitState::Unmapped;
        let size = self
            .toplevel_data
            .requested_pos
            .get()
            .unwrap_or_default()
            .size2();
        let mut w = size.width();
        let mut h = size.height();
        if initial {
            (w, h) = (0, 0);
        } else {
            if self.extents_set.get() {
                w = w.max(1);
                h = h.max(1);
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
            apply!(w, min_width, max_width);
            apply!(h, min_height, max_height);
        }
        XdgSurfaceConfigureData::Toplevel {
            w,
            h,
            state: self.states.get(),
        }
    }

    fn send_configure(&self, data: XdgSurfaceConfigureData) {
        let XdgSurfaceConfigureData::Toplevel { w, h, state } = data else {
            return;
        };
        self.send_configure(w, h, state);
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

pub struct XdgToplevelTreeOp {
    tl: Rc<XdgToplevel>,
    kind: XdgToplevelTreeOpKind,
}

enum XdgToplevelTreeOpKind {
    MarkFullscreen(bool),
}

impl TreeTransactionOp for XdgToplevelTreeOp {
    fn unblocked(self, _serial: TreeSerial, _timeout: bool) {
        match self.kind {
            XdgToplevelTreeOpKind::MarkFullscreen(b) => {
                self.tl.mapped_fullscreen.set(b);
                self.tl.xdg.update_effective_geometry();
            }
        }
    }
}
