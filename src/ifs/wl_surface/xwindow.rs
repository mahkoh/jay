use {
    crate::{
        client::Client,
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            wl_seat::{NodeSeatState, WlSeatGlobal},
            wl_surface::{SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError},
        },
        rect::Rect,
        render::Renderer,
        state::State,
        tree::{
            FindTreeResult, FoundNode, Node, NodeId, NodeVisitor, StackedNode, ToplevelData,
            ToplevelNode,
        },
        utils::{clonecell::CloneCell, copyhashmap::CopyHashMap, linkedlist::LinkedNode},
        wire::WlSurfaceId,
        wire_xcon::CreateNotify,
        xwayland::XWaylandEvent,
    },
    bstr::BString,
    jay_config::Direction,
    std::{
        cell::{Cell, RefCell},
        ops::{Deref, Not},
        rc::Rc,
    },
    thiserror::Error,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum XInputModel {
    None,
    Passive,
    Local,
    Global,
}

impl Default for XInputModel {
    fn default() -> Self {
        Self::Passive
    }
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
    pub instance: RefCell<Option<BString>>,
    pub class: RefCell<Option<BString>>,
    pub title: RefCell<Option<String>>,
    pub role: RefCell<Option<BString>>,
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
    pub seat_state: NodeSeatState,
    pub data: Rc<XwindowData>,
    pub surface: Rc<WlSurface>,
    pub parent_node: CloneCell<Option<Rc<dyn Node>>>,
    pub display_link: RefCell<Option<LinkedNode<Rc<dyn StackedNode>>>>,
    pub toplevel_data: ToplevelData,
}

impl XwindowData {
    pub fn new(state: &Rc<State>, event: &CreateNotify, client: &Rc<Client>) -> Self {
        let extents = Rect::new_sized(
            event.x as _,
            event.y as _,
            event.width as _,
            event.height as _,
        )
        .unwrap();
        // log::info!("xwin {} new {:?} or {}", event.window, extents, event.override_redirect);
        Self {
            state: state.clone(),
            window_id: event.window,
            client: client.clone(),
            surface_id: Cell::new(None),
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
        if let Some(w) = self.window.get() {
            if let Some(p) = w.parent_node.get() {
                p.node_child_title_changed(w.deref(), title.as_deref().unwrap_or(""));
            }
        }
    }
}

pub enum Change {
    None,
    Map,
    Unmap,
}

impl Xwindow {
    pub fn new(data: &Rc<XwindowData>, surface: &Rc<WlSurface>) -> Self {
        Self {
            id: data.state.node_ids.next(),
            seat_state: Default::default(),
            data: data.clone(),
            surface: surface.clone(),
            parent_node: Default::default(),
            display_link: Default::default(),
            toplevel_data: ToplevelData::new(
                &data.state,
                data.info.title.borrow_mut().clone().unwrap_or_default(),
                Some(surface.client.clone()),
            ),
        }
    }

    pub fn destroy(&self) {
        self.break_loops();
        self.data.window.take();
    }

    pub fn break_loops(&self) {
        self.tl_destroy();
        self.surface.set_toplevel(None);
    }

    pub fn install(self: &Rc<Self>) -> Result<(), XWindowError> {
        self.surface.set_role(SurfaceRole::XSurface)?;
        if self.surface.ext.get().is_some() {
            return Err(XWindowError::AlreadyAttached);
        }
        self.surface.ext.set(self.clone());
        self.surface.set_toplevel(Some(self.clone()));
        Ok(())
    }

    fn notify_parent(&self) {
        let parent = match self.parent_node.get() {
            Some(p) => p,
            _ => return,
        };
        let extents = self.surface.extents.get();
        parent.clone().node_child_active_changed(
            self,
            self.toplevel_data.active_children.get() > 0,
            1,
        );
        parent.node_child_size_changed(self, extents.width(), extents.height());
        parent.node_child_title_changed(
            self,
            self.data.info.title.borrow_mut().as_deref().unwrap_or(""),
        );
    }

    pub fn is_mapped(&self) -> bool {
        self.parent_node.get().is_some() || self.display_link.borrow_mut().is_some()
    }

    pub fn may_be_mapped(&self) -> bool {
        self.surface.buffer.get().is_some() && self.data.info.mapped.get()
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
        match map_change {
            Change::None => return,
            Change::Unmap => {
                self.data
                    .info
                    .pending_extents
                    .set(self.data.info.extents.take());
                self.tl_destroy();
            }
            Change::Map if self.data.info.override_redirect.get() => {
                self.clone()
                    .tl_change_extents(&self.data.info.pending_extents.get());
                *self.display_link.borrow_mut() =
                    Some(self.data.state.root.stacked.add_last(self.clone()));
                self.data.state.tree_changed();
            }
            Change::Map if self.data.info.wants_floating.get() => {
                let ws = self.data.state.float_map_ws();
                let ext = self.data.info.pending_extents.get();
                self.data
                    .state
                    .map_floating(self.clone(), ext.width(), ext.height(), &ws);
                self.data.title_changed();
            }
            Change::Map => {
                self.data.state.map_tiled(self.clone());
                self.data.title_changed();
            }
        }
        match map_change {
            Change::Unmap => self.tl_set_visible(false),
            Change::Map => self.tl_set_visible(true),
            Change::None => {}
        }
        self.data.state.tree_changed();
    }
}

impl SurfaceExt for Xwindow {
    fn post_commit(self: Rc<Self>) {
        self.map_status_changed();
    }

    fn on_surface_destroy(&self) -> Result<(), WlSurfaceError> {
        self.tl_destroy();
        self.surface.unset_ext();
        self.data.window.set(None);
        self.data.surface_id.set(None);
        self.data
            .state
            .xwayland
            .queue
            .push(XWaylandEvent::SurfaceDestroyed(self.surface.id));
        Ok(())
    }

    fn extents_changed(&self) {
        self.notify_parent();
    }
}

impl Node for Xwindow {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_xwindow(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self.surface);
    }

    fn node_visible(&self) -> bool {
        self.surface.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.data.info.extents.get()
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _direction: Direction) {
        seat.focus_toplevel(self.clone());
    }

    fn node_find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        if let Some(buffer) = self.surface.buffer.get() {
            if x < buffer.rect.width() && y < buffer.rect.height() {
                tree.push(FoundNode {
                    node: self.surface.clone(),
                    x,
                    y,
                });
                return FindTreeResult::AcceptsInput;
            }
        }
        FindTreeResult::Other
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_surface(&self.surface, x, y)
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        Some(self.data.client.clone())
    }

    fn node_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        Some(self)
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.enter_toplevel(self.clone());
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }
}

impl ToplevelNode for Xwindow {
    tl_node_impl!();

    fn tl_data(&self) -> &ToplevelData {
        &self.toplevel_data
    }

    fn tl_default_focus_child(&self) -> Option<Rc<dyn Node>> {
        Some(self.surface.clone())
    }

    fn tl_accepts_keyboard_focus(&self) -> bool {
        self.data.info.never_focus.get().not()
            && self.data.info.input_model.get() != XInputModel::None
    }

    fn tl_set_active(&self, active: bool) {
        if let Some(pn) = self.parent_node.get() {
            pn.node_child_active_changed(self, active, 1);
        }
    }

    fn tl_on_activate(&self) {
        self.data
            .state
            .xwayland
            .queue
            .push(XWaylandEvent::Activate(self.data.clone()));
    }

    fn tl_change_extents(self: Rc<Self>, rect: &Rect) {
        // log::info!("xwin {} change_extents {:?}", self.data.window_id, rect);
        let old = self.data.info.extents.replace(*rect);
        if old != *rect {
            if self.toplevel_data.is_floating.get() {
                self.toplevel_data.float_width.set(rect.width());
                self.toplevel_data.float_height.set(rect.height());
            }
            if !self.data.info.override_redirect.get() {
                self.data
                    .state
                    .xwayland
                    .queue
                    .push(XWaylandEvent::Configure(self.clone()));
            }
            if old.position() != rect.position() {
                self.surface.set_absolute_position(rect.x1(), rect.y1());
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

    fn tl_set_visible(&self, visible: bool) {
        self.surface.set_visible(visible);
        self.seat_state.set_visible(self, visible);
    }

    fn tl_destroy(&self) {
        self.toplevel_data.destroy_node(self);
        self.display_link.borrow_mut().take();
        self.surface.destroy_node();
    }
}

impl StackedNode for Xwindow {
    stacked_node_impl!();
}

#[derive(Debug, Error)]
pub enum XWindowError {
    #[error("The surface is already attached")]
    AlreadyAttached,
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
}
