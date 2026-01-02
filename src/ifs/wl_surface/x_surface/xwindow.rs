use {
    crate::{
        client::Client,
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            wl_seat::{NodeSeatState, WlSeatGlobal, tablet::TabletTool},
            wl_surface::{WlSurface, WlSurfaceError, x_surface::XSurface},
        },
        rect::Rect,
        renderer::Renderer,
        state::State,
        tree::{
            ContainerSplit, Direction, FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId,
            NodeLayerLink, NodeLocation, NodeVisitor, OutputNode, StackedNode, TileDragDestination,
            ToplevelData, ToplevelNode, ToplevelNodeBase, ToplevelType, WorkspaceNode,
            default_tile_drag_destination,
        },
        utils::{clonecell::CloneCell, copyhashmap::CopyHashMap, linkedlist::LinkedNode},
        wire::WlSurfaceId,
        wire_xcon::CreateNotify,
        xwayland::XWaylandEvent,
    },
    bstr::BString,
    jay_config::window::TileState,
    std::{
        cell::{Cell, RefCell},
        ops::{Deref, Not},
        rc::Rc,
    },
    thiserror::Error,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum XInputModel {
    None,
    #[default]
    Passive,
    Local,
    Global,
}

#[derive(Default, Debug)]
pub struct IcccmHints {
    pub flags: Cell<i32>,
    pub input: Cell<bool>,
    pub initial_state: Cell<i32>,
    pub icon_pixmap: Cell<u32>,
    pub icon_window: Cell<u32>,
    pub icon_x: Cell<i32>,
    pub icon_y: Cell<i32>,
    pub icon_mask: Cell<u32>,
    pub window_group: Cell<u32>,
}

#[derive(Default, Debug)]
pub struct SizeHints {
    pub flags: Cell<u32>,
    pub x: Cell<i32>,
    pub y: Cell<i32>,
    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub min_width: Cell<i32>,
    pub min_height: Cell<i32>,
    pub max_width: Cell<i32>,
    pub max_height: Cell<i32>,
    pub width_inc: Cell<i32>,
    pub height_inc: Cell<i32>,
    pub min_aspect_num: Cell<i32>,
    pub min_aspect_den: Cell<i32>,
    pub max_aspect_num: Cell<i32>,
    pub max_aspect_den: Cell<i32>,
    pub base_width: Cell<i32>,
    pub base_height: Cell<i32>,
    pub win_gravity: Cell<u32>,
}

#[derive(Default, Debug)]
pub struct MotifHints {
    pub flags: Cell<u32>,
    pub decorations: Cell<u32>,
}

#[derive(Default, Debug)]
pub struct XwindowInfo {
    pub has_alpha: Cell<bool>,
    pub override_redirect: Cell<bool>,
    pub extents: Cell<Rect>,
    pub pending_extents: Cell<Rect>,
    pub instance: RefCell<Option<String>>,
    pub class: RefCell<Option<String>>,
    pub title: RefCell<Option<String>>,
    pub role: RefCell<Option<String>>,
    pub protocols: CopyHashMap<u32, ()>,
    pub window_types: CopyHashMap<u32, ()>,
    pub never_focus: Cell<bool>,
    pub utf8_title: Cell<bool>,
    pub icccm_hints: IcccmHints,
    pub normal_hints: SizeHints,
    pub motif_hints: MotifHints,
    pub startup_id: RefCell<Option<BString>>,
    pub fullscreen: Cell<bool>,
    pub modal: Cell<bool>,
    pub maximized_vert: Cell<bool>,
    pub maximized_horz: Cell<bool>,
    pub minimized: Cell<bool>,
    pub pid: Cell<Option<u32>>,
    pub input_model: Cell<XInputModel>,
    pub mapped: Cell<bool>,
    pub wants_floating: Cell<bool>,
}

pub struct XwindowData {
    pub state: Rc<State>,
    pub window_id: u32,
    pub client: Rc<Client>,
    pub surface_id: Cell<Option<WlSurfaceId>>,
    pub surface_serial: Cell<Option<u64>>,
    pub window: CloneCell<Option<Rc<Xwindow>>>,
    pub info: XwindowInfo,
    pub children: CopyHashMap<u32, Rc<XwindowData>>,
    pub parent: CloneCell<Option<Rc<XwindowData>>>,
    pub stack_link: RefCell<Option<LinkedNode<Rc<XwindowData>>>>,
    pub map_link: Cell<Option<LinkedNode<Rc<XwindowData>>>>,
    pub startup_info: RefCell<Vec<u8>>,
    pub destroyed: Cell<bool>,
}

tree_id!(XwindowId);
pub struct Xwindow {
    pub id: XwindowId,
    pub data: Rc<XwindowData>,
    pub x: Rc<XSurface>,
    pub display_link: RefCell<Option<LinkedNode<Rc<dyn StackedNode>>>>,
    pub toplevel_data: ToplevelData,
}

impl XwindowData {
    pub fn new(state: &Rc<State>, event: &CreateNotify, client: &Rc<Client>) -> Self {
        let mut x = event.x as i32;
        let mut y = event.y as i32;
        let mut width = event.width as i32;
        let mut height = event.height as i32;
        client_wire_scale_to_logical!(client, x, y, width, height);
        let extents = Rect::new_sized_saturating(x, y, width, height);
        // log::info!("xwin {} new {:?} or {}", event.window, extents, event.override_redirect);
        Self {
            state: state.clone(),
            window_id: event.window,
            client: client.clone(),
            surface_id: Cell::new(None),
            surface_serial: Cell::new(None),
            window: Default::default(),
            info: XwindowInfo {
                override_redirect: Cell::new(event.override_redirect != 0),
                pending_extents: Cell::new(extents),
                ..Default::default()
            },
            children: Default::default(),
            parent: Default::default(),
            stack_link: Default::default(),
            map_link: Default::default(),
            startup_info: Default::default(),
            destroyed: Cell::new(false),
        }
    }

    pub fn is_ancestor_of(&self, mut other: Rc<Self>) -> bool {
        loop {
            if other.window_id == self.window_id {
                return true;
            }
            other = match other.parent.get() {
                Some(p) => p,
                _ => return false,
            }
        }
    }

    pub fn title_changed(&self) {
        let title = self.info.title.borrow_mut();
        if let Some(w) = self.window.get()
            && let Some(p) = w.toplevel_data.parent.get()
        {
            p.node_child_title_changed(w.deref(), title.as_deref().unwrap_or(""));
        }
    }
}

pub enum Change {
    None,
    Map,
    Unmap,
}

impl Xwindow {
    pub fn install(
        data: &Rc<XwindowData>,
        surface: &Rc<WlSurface>,
    ) -> Result<Rc<Self>, XWindowError> {
        let xsurface = surface.get_xsurface()?;
        if xsurface.xwindow.is_some() {
            return Err(XWindowError::AlreadyAttached);
        }
        let id = data.state.node_ids.next();
        let slf = Rc::new_cyclic(|weak| {
            let tld = ToplevelData::new(
                &data.state,
                data.info.title.borrow_mut().clone().unwrap_or_default(),
                Some(surface.client.clone()),
                ToplevelType::XWindow(data.clone()),
                id,
                weak,
            );
            tld.pos.set(surface.extents.get());
            tld.content_type.set(surface.content_type.get());
            Self {
                id,
                data: data.clone(),
                display_link: Default::default(),
                toplevel_data: tld,
                x: xsurface,
            }
        });
        slf.x.xwindow.set(Some(slf.clone()));
        slf.update_toplevel();
        Ok(slf)
    }

    pub fn destroy(&self) {
        self.break_loops();
        self.data.window.take();
    }

    pub fn break_loops(&self) {
        self.tl_destroy();
        self.x.surface.set_toplevel(None);
        self.x.xwindow.set(None);
        self.x
            .surface
            .client
            .state
            .xwayland
            .windows
            .remove(&self.id);
    }

    pub fn is_mapped(&self) -> bool {
        self.toplevel_data.parent.is_some() || self.display_link.borrow_mut().is_some()
    }

    pub fn may_be_mapped(&self) -> bool {
        self.x.surface.buffer.is_some() && self.data.info.mapped.get()
    }

    fn map_change(&self) -> Change {
        match (self.may_be_mapped(), self.is_mapped()) {
            (true, false) => Change::Map,
            (false, true) => Change::Unmap,
            _ => Change::None,
        }
    }

    pub fn map_status_changed(self: &Rc<Self>) {
        let map_change = self.map_change();
        let override_redirect = self.data.info.override_redirect.get();
        let map_floating = match self
            .toplevel_data
            .state
            .initial_tile_state(&self.toplevel_data)
        {
            None => self.data.info.wants_floating.get(),
            Some(m) => m == TileState::Floating,
        };
        match map_change {
            Change::None => return,
            Change::Unmap => {
                self.data
                    .info
                    .pending_extents
                    .set(self.data.info.extents.take());
                self.tl_destroy();
            }
            Change::Map if override_redirect => {
                self.clone()
                    .tl_change_extents(&self.data.info.pending_extents.get());
                *self.display_link.borrow_mut() =
                    Some(self.data.state.root.stacked.add_last(self.clone()));
                self.data.state.tree_changed();
            }
            Change::Map if map_floating => {
                let ws = self.data.state.float_map_ws();
                let ext = self.data.info.pending_extents.get();
                self.data
                    .state
                    .map_floating(self.clone(), ext.width(), ext.height(), &ws, None);
                self.data.title_changed();
            }
            Change::Map => {
                self.data.state.map_tiled(self.clone());
                if self.data.info.fullscreen.get() {
                    self.clone().tl_set_fullscreen(true, None);
                }
                self.data.title_changed();
            }
        }
        match map_change {
            Change::Unmap => self.tl_set_visible(false),
            Change::Map => {
                if override_redirect {
                    self.tl_set_visible(true);
                }
                self.toplevel_data.broadcast(self.clone());
            }
            Change::None => {}
        }
        self.data.state.tree_changed();
        self.damage_override_redirect();
    }

    fn damage_override_redirect(&self) {
        if !self.data.info.override_redirect.get() {
            return;
        }
        let extents = self.x.surface.extents.get();
        let (x, y) = self.x.surface.buffer_abs_pos.get().position();
        let extents = extents.move_(x, y);
        self.data.state.damage(extents);
    }

    pub fn update_toplevel(self: &Rc<Self>) {
        let mut toplevel = None;
        if !self.data.info.override_redirect.get() {
            toplevel = Some(self.clone() as _);
        }
        self.x.surface.set_toplevel(toplevel);
    }
}

impl Node for Xwindow {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.toplevel_data.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_xwindow(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self.x.surface);
    }

    fn node_visible(&self) -> bool {
        self.x.surface.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.data.info.extents.get()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        self.toplevel_data.output_opt()
    }

    fn node_location(&self) -> Option<NodeLocation> {
        self.x.surface.node_location()
    }

    fn node_layer(&self) -> NodeLayerLink {
        if let Some(link) = self.display_link.borrow().as_ref() {
            return NodeLayerLink::Stacked(link.to_ref());
        }
        self.toplevel_data.node_layer()
    }

    fn node_accepts_focus(&self) -> bool {
        self.tl_accepts_keyboard_focus()
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
        match usecase {
            FindTreeUsecase::None => {}
            FindTreeUsecase::SelectToplevel => return FindTreeResult::AcceptsInput,
            FindTreeUsecase::SelectToplevelOrPopup => return FindTreeResult::AcceptsInput,
            FindTreeUsecase::SelectWorkspace => return FindTreeResult::Other,
        }
        let rect = self.x.surface.buffer_abs_pos.get();
        if x < rect.width() && y < rect.height() {
            return self.x.surface.find_tree_at_(x, y, tree);
        }
        FindTreeResult::Other
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, bounds: Option<&Rect>) {
        renderer.render_xwindow(self, x, y, bounds)
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        Some(self.data.client.clone())
    }

    fn node_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        Some(self)
    }

    fn node_make_visible(self: Rc<Self>) {
        self.toplevel_data.make_visible(&*self);
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.enter_toplevel(self.clone());
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("wl-surface focus");
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

impl ToplevelNodeBase for Xwindow {
    fn tl_data(&self) -> &ToplevelData {
        &self.toplevel_data
    }

    fn tl_accepts_keyboard_focus(&self) -> bool {
        self.data.info.never_focus.get().not()
            && self.data.info.input_model.get() != XInputModel::None
    }

    fn tl_focus_child(&self) -> Option<Rc<dyn Node>> {
        Some(self.x.surface.clone())
    }

    fn tl_set_workspace_ext(&self, ws: &Rc<WorkspaceNode>) {
        self.x.surface.set_output(&ws.output.get(), ws.location());
    }

    fn tl_change_extents_impl(self: Rc<Self>, rect: &Rect) {
        // log::info!("xwin {} change_extents {:?}", self.data.window_id, rect);
        let old = self.data.info.extents.replace(*rect);
        if old != *rect {
            if self.data.info.override_redirect.get() {
                let (x, y) = rect.center();
                let output = self.data.state.find_closest_output(x, y).0;
                self.x
                    .surface
                    .set_output(&output, NodeLocation::Output(output.id));
            } else {
                self.data
                    .state
                    .xwayland
                    .queue
                    .push(XWaylandEvent::Configure(self.clone()));
            }
            if old.position() != rect.position() {
                self.x.surface.set_absolute_position(rect.x1(), rect.y1());
            }
        }
    }

    fn tl_close(self: Rc<Self>) {
        self.data
            .state
            .xwayland
            .queue
            .push(XWaylandEvent::Close(self.data.clone()));
    }

    fn tl_set_visible_impl(&self, visible: bool) {
        self.x.surface.set_visible(visible);
    }

    fn tl_destroy_impl(&self) {
        self.display_link.borrow_mut().take();
        self.x.surface.destroy_node();
    }

    fn tl_last_active_child(self: Rc<Self>) -> Rc<dyn ToplevelNode> {
        self
    }

    fn tl_scanout_surface(&self) -> Option<Rc<WlSurface>> {
        Some(self.x.surface.clone())
    }

    fn tl_admits_children(&self) -> bool {
        false
    }

    fn tl_tile_drag_destination(
        self: Rc<Self>,
        source: NodeId,
        split: Option<ContainerSplit>,
        abs_bounds: Rect,
        abs_x: i32,
        abs_y: i32,
    ) -> Option<TileDragDestination> {
        default_tile_drag_destination(self, source, split, abs_bounds, abs_x, abs_y)
    }
}

impl StackedNode for Xwindow {
    fn stacked_set_visible(&self, visible: bool) {
        self.damage_override_redirect();
        self.tl_set_visible(visible);
    }

    fn stacked_has_workspace_link(&self) -> bool {
        false
    }
}

#[derive(Debug, Error)]
pub enum XWindowError {
    #[error("The surface is already attached")]
    AlreadyAttached,
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
}
