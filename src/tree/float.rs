use std::cell::{Cell, RefCell};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::rc::Rc;
use i4config::Direction;
use crate::{CloneCell, ErrorFmt, State, text};
use crate::backend::{KeyState, ScrollAxis};
use crate::client::{Client, ClientId};
use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::{Dnd, NodeSeatState, WlSeatGlobal};
use crate::ifs::wl_surface::WlSurface;
use crate::rect::Rect;
use crate::render::{Renderer, Texture};
use crate::theme::Color;
use crate::tree::{ContainerNode, ContainerSplit, FindTreeResult, FoundNode, Node, NodeId, WorkspaceNode};
use crate::tree::walker::NodeVisitor;
use crate::utils::linkedlist::LinkedNode;
use crate::xkbcommon::ModifierState;

tree_id!(FloatNodeId);
pub struct FloatNode {
    pub id: FloatNodeId,
    pub state: Rc<State>,
    pub visible: Cell<bool>,
    pub position: Cell<Rect>,
    pub display_link: Cell<Option<LinkedNode<Rc<dyn Node>>>>,
    pub workspace_link: Cell<Option<LinkedNode<Rc<dyn Node>>>>,
    pub workspace: CloneCell<Rc<WorkspaceNode>>,
    pub child: CloneCell<Option<Rc<dyn Node>>>,
    pub active: Cell<bool>,
    pub seat_state: NodeSeatState,
    pub layout_scheduled: Cell<bool>,
    pub render_titles_scheduled: Cell<bool>,
    pub title: RefCell<String>,
    pub title_texture: CloneCell<Option<Rc<Texture>>>,
}

pub async fn float_layout(state: Rc<State>) {
    loop {
        let node = state.pending_float_layout.pop().await;
        if node.layout_scheduled.get() {
            node.perform_layout();
        }
    }
}

pub async fn float_titles(state: Rc<State>) {
    loop {
        let node = state.pending_float_titles.pop().await;
        if node.render_titles_scheduled.get() {
            node.render_title();
        }
    }
}

impl FloatNode {
    pub fn new(state: &Rc<State>, ws: &Rc<WorkspaceNode>, position: Rect, child: Rc<dyn Node>) -> Rc<Self> {
        let floater = Rc::new(FloatNode {
            id: state.node_ids.next(),
            state: state.clone(),
            visible: Cell::new(true),
            position: Cell::new(position),
            display_link: Cell::new(None),
            workspace_link: Cell::new(None),
            workspace: CloneCell::new(ws.clone()),
            child: CloneCell::new(Some(child.clone())),
            active: Cell::new(false),
            seat_state: Default::default(),
            layout_scheduled: Cell::new(false),
            render_titles_scheduled: Cell::new(false),
            title: Default::default(),
            title_texture: Default::default(),
        });
        floater
            .display_link
            .set(Some(state.root.stacked.add_last(floater.clone())));
        floater
            .workspace_link
            .set(Some(ws.stacked.add_last(floater.clone())));
        child.clone().set_workspace(ws);
        child.clone().set_parent(floater.clone());
        floater.schedule_layout();
        floater
    }

    pub fn on_spaces_changed(self: &Rc<Self>) {
        self.schedule_layout();
    }

    pub fn on_colors_changed(self: &Rc<Self>) {
        self.schedule_render_titles();
    }

    fn schedule_layout(self: &Rc<Self>) {
        if !self.layout_scheduled.replace(true) {
            self.state.pending_float_layout.push(self.clone());
        }
    }

    fn perform_layout(self: &Rc<Self>) {
        let child = match self.child.get() {
            Some(c) => c,
            _ => return,
        };
        let pos = self.position.get();
        let theme = &self.state.theme;
        let bw = theme.border_width.get();
        let th = theme.title_height.get();
        let cpos = Rect::new_sized(
            pos.x1() + bw,
            pos.y1() + bw + th + 1,
            (pos.width() - 2 * bw).max(0),
            (pos.height() - 2 * bw - th - 1).max(0),
        ).unwrap();
        child.clone().change_extents(&cpos);
        self.layout_scheduled.set(false);
        self.schedule_render_titles();
    }

    fn schedule_render_titles(self: &Rc<Self>) {
        if !self.render_titles_scheduled.replace(true) {
            self.state.pending_float_titles.push(self.clone());
        }
    }

    fn render_title(&self) {
        self.render_titles_scheduled.set(false);
        let theme = &self.state.theme;
        let th = theme.title_height.get();
        let font = theme.font.borrow_mut();
        let title = self.title.borrow_mut();
        self.title_texture.set(None);
        let pos = self.position.get();
        let ctx = match self.state.render_ctx.get() {
            Some(c) => c,
            _ => return,
        };
        let texture = match text::render(&ctx, pos.width(), th, &font, &title, Color::GREY) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Could not render title {}: {}", title, ErrorFmt(e));
                return;
            }
        };
        self.title_texture.set(Some(texture));
    }
}

impl Debug for FloatNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FloatNode").finish_non_exhaustive()
    }
}

impl Node for FloatNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, _detach: bool) {
        let _v = self.display_link.take();
        let _v = self.workspace_link.take();
        if let Some(child) = self.child.get() {
            child.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_float(&self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(c) = self.child.get() {
            c.visit(visitor);
        }
    }

    fn absolute_position(&self) -> Rect {
        self.position.get()
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        let theme = &self.state.theme;
        let th = theme.title_height.get();
        let bw = theme.border_width.get();
        let pos = self.position.get();
        if x < bw || x >= pos.width() - bw {
            return FindTreeResult::AcceptsInput;
        }
        if y < bw + th + 1 || x >= pos.height() - bw {
            return FindTreeResult::AcceptsInput;
        }
        let child = match self.child.get() {
            Some(c) => c,
            _ => return FindTreeResult::Other,
        };
        let x = x - bw;
        let y = y - bw - th - 1;
        tree.push(FoundNode {
            node: child.clone(),
            x,
            y,
        });
        child.find_tree_at(x, y, tree)
    }

    fn remove_child(self: Rc<Self>, _child: &dyn Node) {
        self.child.set(None);
        self.display_link.set(None);
        self.workspace_link.set(None);
    }

    fn pointer_target(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_floating(self, x, y)
    }

    fn into_float(self: Rc<Self>) -> Option<Rc<FloatNode>> {
        Some(self)
    }

    fn accepts_child(&self, _node: &dyn Node) -> bool {
        true
    }

    fn is_float(&self) -> bool {
        true
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        if let Some(c) = self.child.get() {
            c.set_workspace(ws);
        }
        self.workspace_link
            .set(Some(ws.stacked.add_last(self.clone())));
        self.workspace.set(ws.clone());
    }

    fn child_active_changed(&self, _child: &dyn Node, active: bool) {
        self.active.set(active);
    }

    fn child_title_changed(self: Rc<Self>, child: &dyn Node, title: &str) {
        let mut t = self.title.borrow_mut();
        if t.deref() != title {
            t.clear();
            t.push_str(title);
            self.schedule_render_titles();
        }
    }
}
