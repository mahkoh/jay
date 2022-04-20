use {
    crate::{
        cursor::KnownCursor,
        ifs::wl_seat::{NodeSeatState, WlSeatGlobal},
        rect::Rect,
        render::Renderer,
        tree::{
            container::ContainerNode, walker::NodeVisitor, FindTreeResult, FoundNode, Node, NodeId,
            OutputNode, SizedNode,
        },
        utils::{
            clonecell::CloneCell,
            linkedlist::{LinkedList, LinkedNode},
        },
    },
    jay_config::Direction,
    std::{cell::Cell, fmt::Debug, rc::Rc},
};
use crate::tree::FullscreenNode;

tree_id!(WorkspaceNodeId);

pub struct WorkspaceNode {
    pub id: WorkspaceNodeId,
    pub output: CloneCell<Rc<OutputNode>>,
    pub position: Cell<Rect>,
    pub container: CloneCell<Option<Rc<ContainerNode>>>,
    pub stacked: LinkedList<Rc<dyn Node>>,
    pub seat_state: NodeSeatState,
    pub name: String,
    pub output_link: Cell<Option<LinkedNode<Rc<WorkspaceNode>>>>,
    pub visible: Cell<bool>,
    pub fullscreen: CloneCell<Option<Rc<dyn FullscreenNode>>>,
}

impl WorkspaceNode {
    pub fn set_container(self: &Rc<Self>, container: &Rc<ContainerNode>) {
        let pos = self.position.get();
        container.change_extents(&pos);
        container.set_workspace(self);
        container.set_visible(self.visible.get());
        self.container.set(Some(container.clone()));
    }
}

impl SizedNode for WorkspaceNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, detach: bool) {
        if detach {
            self.output.get().node_remove_child(self);
        }
        self.output_link.set(None);
        if let Some(container) = self.container.take() {
            container.node_destroy(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_workspace(self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(c) = self.container.get() {
            visitor.visit_container(&c);
        }
        if let Some(fs) = self.fullscreen.get() {
            fs.into_node().node_visit(visitor);
        }
    }

    fn visible(&self) -> bool {
        self.visible.get()
    }

    fn parent(&self) -> Option<Rc<dyn Node>> {
        Some(self.output.get())
    }

    fn last_active_child(self: &Rc<Self>) -> Rc<dyn Node> {
        if let Some(c) = self.container.get() {
            return c.last_active_child();
        }
        self.clone()
    }

    fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        if let Some(container) = self.container.get() {
            container.node_set_visible(visible);
        }
        self.seat_state.set_visible(self, visible);
    }

    fn do_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        if let Some(container) = self.container.get() {
            container.do_focus(seat, direction);
        }
    }

    fn absolute_position(&self) -> Rect {
        self.position.get()
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        if let Some(n) = self.container.get() {
            tree.push(FoundNode {
                node: n.clone(),
                x,
                y,
            });
            n.node_find_tree_at(x, y, tree);
        }
        FindTreeResult::AcceptsInput
    }

    fn remove_child2(self: &Rc<Self>, _child: &dyn Node, _preserve_focus: bool) {
        self.container.set(None);
    }

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_workspace(self, x, y);
    }

    fn into_workspace(self: &Rc<Self>) -> Option<Rc<WorkspaceNode>> {
        Some(self.clone())
    }

    fn accepts_child(&self, node: &dyn Node) -> bool {
        node.node_is_container()
    }

    fn is_workspace(&self) -> bool {
        true
    }

    fn change_extents(self: &Rc<Self>, rect: &Rect) {
        self.position.set(*rect);
        if let Some(c) = self.container.get() {
            c.node_change_extents(rect);
        }
    }
}
