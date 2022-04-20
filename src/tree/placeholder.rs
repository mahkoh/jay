use {
    crate::{
        client::Client,
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            wl_seat::{NodeSeatState, WlSeatGlobal},
            wl_surface::WlSurface,
        },
        rect::Rect,
        render::{Renderer, Texture},
        state::State,
        text,
        theme::Color,
        tree::{
            FindTreeResult, FoundNode, FullscreenNode, Node, NodeId, NodeVisitor, SizedNode,
            SizedToplevelNode, ToplevelData, WorkspaceNode,
        },
        utils::clonecell::CloneCell,
    },
    jay_config::Direction,
    std::{
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
    },
};
use crate::utils::errorfmt::ErrorFmt;

tree_id!(DetachedNodeId);
pub struct PlaceholderNode {
    id: DetachedNodeId,
    state: Rc<State>,
    seat_state: NodeSeatState,
    parent: CloneCell<Option<Rc<dyn Node>>>,
    workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    title: RefCell<String>,
    visible: Cell<bool>,
    pos: Cell<Rect>,
    client: Option<Rc<Client>>,
    toplevel: ToplevelData,
    active: Cell<bool>,
    destroyed: Cell<bool>,
    texture: CloneCell<Option<Rc<Texture>>>,
}

impl PlaceholderNode {
    pub fn new_for(state: &Rc<State>, node: Rc<dyn FullscreenNode>) -> Self {
        Self {
            id: state.node_ids.next(),
            state: state.clone(),
            seat_state: Default::default(),
            parent: Default::default(),
            workspace: Default::default(),
            title: RefCell::new(node.title()),
            visible: Default::default(),
            pos: Default::default(),
            client: node.as_node().node_client(),
            toplevel: Default::default(),
            active: Default::default(),
            destroyed: Default::default(),
            texture: Default::default(),
        }
    }

    pub fn texture(&self) -> Option<Rc<Texture>> {
        self.texture.get()
    }

    pub fn set_title(&self, title: &str) {
        *self.title.borrow_mut() = title.to_string();
        if let Some(parent) = self.parent.get() {
            parent.node_child_title_changed(self, title);
        }
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }

    pub fn position(&self) -> Rect {
        self.pos.get()
    }
}

impl SizedNode for PlaceholderNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, detach: bool) {
        if detach {
            if let Some(parent) = self.parent.get() {
                parent.node_remove_child(self);
            }
        }
        self.parent.take();
        self.workspace.take();
        self.seat_state.destroy_node(self);
        self.destroyed.set(true);
    }

    fn visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_placeholder(self);
    }

    fn visit_children(&self, _visitor: &mut dyn NodeVisitor) {
        // nothing
    }

    fn visible(&self) -> bool {
        self.visible.get()
    }

    fn parent(&self) -> Option<Rc<dyn Node>> {
        self.parent.get()
    }

    fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
    }

    fn get_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        self.workspace.get()
    }

    fn do_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, _direction: Direction) {
        seat.focus_toplevel(self.clone());
    }

    fn close(self: &Rc<Self>) {
        let slf = self.clone();
        self.state.run_toplevel.schedule(move || {
            slf.destroy_node(true);
        });
    }

    fn absolute_position(&self) -> Rect {
        self.pos.get()
    }

    fn active_changed(&self, active: bool) {
        self.active.set(active);
        if let Some(parent) = self.parent.get() {
            parent.node_child_active_changed(self, active, 1);
        }
    }

    fn find_tree_at(&self, _x: i32, _y: i32, _tree: &mut Vec<FoundNode>) -> FindTreeResult {
        FindTreeResult::AcceptsInput
    }

    fn pointer_enter(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.set_known_cursor(KnownCursor::Default);
        seat.enter_toplevel(self.clone());
    }

    fn change_extents(self: &Rc<Self>, rect: &Rect) {
        log::info!("{:?}", rect);
        self.pos.set(*rect);
        if let Some(p) = self.parent.get() {
            p.node_child_size_changed(self.deref(), rect.width(), rect.height());
        }
        self.texture.set(None);
        if let Some(ctx) = self.state.render_ctx.get() {
            if rect.width() != 0 && rect.height() != 0 {
                let font = format!("monospace {}", rect.width() / 10);
                match text::render_fitting(
                    &ctx,
                    rect.height(),
                    &font,
                    "Fullscreen",
                    Color::GREY,
                    false,
                ) {
                    Ok(t) => {
                        self.texture.set(Some(t));
                    },
                    Err(e) => {
                        log::warn!("Could not render fullscreen texture: {}", ErrorFmt(e));
                    }
                }
            }
        }
    }

    fn set_workspace(self: &Rc<Self>, ws: &Rc<WorkspaceNode>) {
        self.workspace.set(Some(ws.clone()));
    }

    fn set_parent(self: &Rc<Self>, parent: Rc<dyn Node>) {
        self.parent.set(Some(parent.clone()));
        parent.node_child_title_changed(self.deref(), self.title.borrow_mut().deref());
    }

    fn client(&self) -> Option<Rc<Client>> {
        self.client.clone()
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_placeholder(self, x, y);
    }
}

impl SizedToplevelNode for PlaceholderNode {
    fn data(&self) -> &ToplevelData {
        &self.toplevel
    }

    fn accepts_keyboard_focus(&self) -> bool {
        true
    }

    fn default_surface(&self) -> Option<Rc<WlSurface>> {
        None
    }

    fn set_active(&self, _active: bool) {
        // nothing
    }

    fn activate(&self) {
        // nothing
    }

    fn set_fullscreen(self: &Rc<Self>, _fullscreen: bool) {
        // nothing
    }

    fn fullscreen(&self) -> bool {
        false
    }
}
