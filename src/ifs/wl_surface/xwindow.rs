use crate::client::Client;
use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::{NodeSeatState, SeatId, WlSeatGlobal};
use crate::ifs::wl_surface::{SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError};
use crate::rect::Rect;
use crate::render::Renderer;
use crate::tree::toplevel::ToplevelNode;
use crate::tree::walker::NodeVisitor;
use crate::tree::{FindTreeResult, FoundNode, Node, NodeId, WorkspaceNode};
use crate::utils::linkedlist::LinkedNode;
use crate::utils::smallmap::SmallMap;
use crate::wire::WlSurfaceId;
use crate::wire_xcon::CreateNotify;
use crate::xwayland::XWaylandEvent;
use crate::{AsyncQueue, CloneCell, State};
use jay_config::Direction;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use thiserror::Error;

pub struct XwindowData {
    pub state: Rc<State>,
    pub window_id: u32,
    pub override_redirect: bool,
    pub extents: Cell<Rect>,
    pub client: Rc<Client>,
    pub surface_id: Cell<Option<WlSurfaceId>>,
    pub window: CloneCell<Option<Rc<Xwindow>>>,
}

tree_id!(XwindowId);
pub struct Xwindow {
    pub id: XwindowId,
    pub seat_state: NodeSeatState,
    pub data: Rc<XwindowData>,
    pub surface: Rc<WlSurface>,
    pub parent: CloneCell<Option<Rc<dyn Node>>>,
    pub focus_history: SmallMap<SeatId, LinkedNode<Rc<dyn ToplevelNode>>, 1>,
    pub events: Rc<AsyncQueue<XWaylandEvent>>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub display_link: RefCell<Option<LinkedNode<Rc<dyn Node>>>>,
    pub display_xlink: RefCell<Option<LinkedNode<Rc<Xwindow>>>>,
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
        log::info!("extents = {:?}", extents);
        Self {
            state: state.clone(),
            window_id: event.window,
            override_redirect: event.override_redirect != 0,
            extents: Cell::new(extents),
            client: client.clone(),
            surface_id: Cell::new(None),
            window: Default::default(),
        }
    }
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
            parent: Default::default(),
            focus_history: Default::default(),
            events: events.clone(),
            workspace: Default::default(),
            display_link: Default::default(),
            display_xlink: Default::default(),
        }
    }

    pub fn destroy(&self) {
        self.break_loops();
        self.data.window.take();
    }

    pub fn break_loops(&self) {
        self.destroy_node(true);
    }

    pub fn install(self: &Rc<Self>) -> Result<(), XWindowError> {
        self.surface.set_role(SurfaceRole::XSurface)?;
        if self.surface.ext.get().is_some() {
            return Err(XWindowError::AlreadyAttached);
        }
        self.surface.ext.set(self.clone());
        Ok(())
    }

    fn notify_parent(&self) {
        let parent = match self.parent.get() {
            Some(p) => p,
            _ => return,
        };
        let extents = self.surface.extents.get();
        // let extents = self.xdg.extents.get();
        // parent.child_active_changed(self, self.active_surfaces.get() > 0);
        parent.child_size_changed(self, extents.width(), extents.height());
        // parent.child_title_changed(self, self.title.borrow_mut().deref());
    }

    fn managed_post_commit(self: &Rc<Self>) {
        let parent = self.parent.get();
        if self.surface.buffer.get().is_some() {
            if parent.is_none() {
                self.data.state.map_tiled(self.clone());
            }
        } else {
            if parent.is_some() {
                self.destroy_node(true);
            }
        }
    }

    fn unmanaged_post_commit(self: &Rc<Self>) {
        let mut dl = self.display_link.borrow_mut();
        let mut dxl = self.display_xlink.borrow_mut();
        if self.surface.buffer.get().is_some() {
            if dl.is_none() {
                *dl = Some(self.data.state.root.stacked.add_last(self.clone()));
                *dxl = Some(self.data.state.root.xstacked.add_last(self.clone()));
                self.data.state.tree_changed();
            }
        } else {
            if dl.is_some() {
                drop(dl);
                drop(dxl);
                self.destroy_node(true);
            }
        }
    }
}

impl SurfaceExt for Xwindow {
    fn post_commit(self: Rc<Self>) {
        if self.data.override_redirect {
            self.unmanaged_post_commit();
        } else {
            self.managed_post_commit();
        }
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

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, _detach: bool) {
        self.display_xlink.borrow_mut().take();
        self.display_link.borrow_mut().take();
        self.workspace.take();
        self.focus_history.clear();
        if let Some(parent) = self.parent.take() {
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

    fn is_contained_in(&self, other: NodeId) -> bool {
        if let Some(parent) = self.parent.get() {
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
        self.data.extents.get()
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
        let nw = rect.width();
        let nh = rect.height();
        let de = self.data.extents.replace(*rect);
        if de.width() != nw || de.height() != nh {
            self.events.push(XWaylandEvent::Configure(self.clone()));
        }
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        self.workspace.set(Some(ws.clone()));
    }

    fn set_parent(self: Rc<Self>, parent: Rc<dyn Node>) {
        self.parent.set(Some(parent));
        self.notify_parent();
    }

    fn client(&self) -> Option<Rc<Client>> {
        Some(self.data.client.clone())
    }
}

impl ToplevelNode for Xwindow {
    fn parent(&self) -> Option<Rc<dyn Node>> {
        self.parent.get()
    }

    fn focus_surface(&self, _seat: &WlSeatGlobal) -> Rc<WlSurface> {
        self.surface.clone()
    }

    fn set_focus_history_link(&self, seat: &WlSeatGlobal, link: LinkedNode<Rc<dyn ToplevelNode>>) {
        self.focus_history.insert(seat.id(), link);
    }

    fn as_node(&self) -> &dyn Node {
        self
    }
}

#[derive(Debug, Error)]
pub enum XWindowError {
    #[error("The surface is already attached")]
    AlreadyAttached,
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
}
