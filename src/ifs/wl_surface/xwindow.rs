use {
    crate::{
        client::Client,
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            wl_seat::{NodeSeatState, SeatId, WlSeatGlobal},
            wl_surface::{SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError},
        },
        rect::Rect,
        render::Renderer,
        state::State,
        tree::{
            FindTreeResult, FoundNode, Node, NodeId, NodeVisitor, ToplevelData, ToplevelNode,
            WorkspaceNode,
        },
        utils::{
            clonecell::CloneCell, copyhashmap::CopyHashMap, linkedlist::LinkedNode,
            queue::AsyncQueue, smallmap::SmallMap,
        },
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
    pub focus_history: SmallMap<SeatId, LinkedNode<Rc<dyn ToplevelNode>>, 1>,
    pub events: Rc<AsyncQueue<XWaylandEvent>>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub display_link: RefCell<Option<LinkedNode<Rc<dyn Node>>>>,
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
        Self {
            state: state.clone(),
            window_id: event.window,
            client: client.clone(),
            surface_id: Cell::new(None),
            window: Default::default(),
            info: XwindowInfo {
                override_redirect: Cell::new(event.override_redirect != 0),
                extents: Cell::new(extents),
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
                p.child_title_changed(w.deref(), title.as_deref().unwrap_or(""));
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
    pub fn new(
        data: &Rc<XwindowData>,
        surface: &Rc<WlSurface>,
        events: &Rc<AsyncQueue<XWaylandEvent>>,
    ) -> Self {
        Self {
            id: data.state.node_ids.next(),
            seat_state: Default::default(),
            data: data.clone(),
            surface: surface.clone(),
            parent_node: Default::default(),
            focus_history: Default::default(),
            events: events.clone(),
            workspace: Default::default(),
            display_link: Default::default(),
            toplevel_data: Default::default(),
        }
    }

    pub fn destroy(&self) {
        self.break_loops();
        self.data.window.take();
    }

    pub fn break_loops(&self) {
        self.destroy_node(true);
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
        // let extents = self.xdg.extents.get();
        // parent.child_active_changed(self, self.active_surfaces.get() > 0);
        parent.child_size_changed(self, extents.width(), extents.height());
        // parent.child_title_changed(self, self.title.borrow_mut().deref());
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
            Change::Unmap => self.destroy_node(true),
            Change::Map if self.data.info.override_redirect.get() => {
                *self.display_link.borrow_mut() =
                    Some(self.data.state.root.stacked.add_last(self.clone()));
                self.data.state.tree_changed();
            }
            Change::Map if self.data.info.wants_floating.get() => {
                let ws = self.data.state.float_map_ws();
                let ext = self.data.info.extents.get();
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
            Change::Unmap => self.set_visible(false),
            Change::Map => self.set_visible(true),
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
        self.destroy_node(true);
        self.surface.unset_ext();
        self.data.window.set(None);
        self.data.surface_id.set(None);
        self.events
            .push(XWaylandEvent::SurfaceDestroyed(self.surface.id));
        Ok(())
    }

    fn extents_changed(&self) {
        self.notify_parent();
    }
}

impl Node for Xwindow {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn close(&self) {
        self.events.push(XWaylandEvent::Close(self.data.clone()));
    }

    fn visible(&self) -> bool {
        self.surface.visible.get()
    }

    fn set_visible(&self, visible: bool) {
        self.surface.set_visible(visible);
        self.seat_state.set_visible(self, visible);
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, _detach: bool) {
        self.toplevel_data.clear();
        self.display_link.borrow_mut().take();
        self.workspace.take();
        self.focus_history.clear();
        if let Some(parent) = self.parent_node.take() {
            parent.remove_child(self);
        }
        self.surface.destroy_node(false);
        self.seat_state.destroy_node(self);
    }

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_xwindow(&self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self.surface);
    }

    fn get_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        self.workspace.get()
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
        self.data.info.extents.get()
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
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

    fn pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.enter_toplevel(self);
    }

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_surface(&self.surface, x, y)
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        let old = self.data.info.extents.replace(*rect);
        if old != *rect {
            self.events.push(XWaylandEvent::Configure(self.clone()));
            if old.position() != rect.position() {
                self.surface.set_absolute_position(rect.x1(), rect.y1());
            }
        }
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        self.workspace.set(Some(ws.clone()));
    }

    fn set_parent(self: Rc<Self>, parent: Rc<dyn Node>) {
        self.parent_node.set(Some(parent));
        self.notify_parent();
    }

    fn client(&self) -> Option<Rc<Client>> {
        Some(self.data.client.clone())
    }
}

impl ToplevelNode for Xwindow {
    fn data(&self) -> &ToplevelData {
        &self.toplevel_data
    }

    fn parent(&self) -> Option<Rc<dyn Node>> {
        self.parent_node.get()
    }

    fn as_node(&self) -> &dyn Node {
        self
    }

    fn into_node(self: Rc<Self>) -> Rc<dyn Node> {
        self
    }

    fn accepts_keyboard_focus(&self) -> bool {
        self.data.info.never_focus.get().not()
            && self.data.info.input_model.get() != XInputModel::None
    }

    fn default_surface(&self) -> Rc<WlSurface> {
        self.surface.clone()
    }

    fn set_active(&self, active: bool) {
        if let Some(pn) = self.parent_node.get() {
            pn.child_active_changed(self, active);
        }
    }

    fn activate(&self) {
        self.events.push(XWaylandEvent::Activate(self.data.clone()));
    }

    fn toggle_floating(self: Rc<Self>) {
        let parent = match self.parent_node.get() {
            Some(p) => p,
            _ => return,
        };
        if parent.is_float() {
            parent.remove_child(&*self);
            self.data.state.map_tiled(self.clone());
        } else if let Some(ws) = self.workspace.get() {
            parent.remove_child(&*self);
            let extents = self.data.info.extents.get();
            self.data
                .state
                .map_floating(self.clone(), extents.width(), extents.height(), &ws);
        }
    }

    fn close(&self) {
        self.events.push(XWaylandEvent::Close(self.data.clone()));
    }
}

#[derive(Debug, Error)]
pub enum XWindowError {
    #[error("The surface is already attached")]
    AlreadyAttached,
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
}
