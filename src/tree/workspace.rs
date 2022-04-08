use {
    crate::{
        cursor::KnownCursor,
        ifs::wl_seat::{NodeSeatState, WlSeatGlobal},
        rect::Rect,
        render::Renderer,
        tree::{
            container::ContainerNode, walker::NodeVisitor, FindTreeResult, FoundNode, Node, NodeId,
            OutputNode,
        },
        utils::{
            clonecell::CloneCell,
            linkedlist::{LinkedList, LinkedNode},
        },
    },
    std::{cell::Cell, fmt::Debug, rc::Rc},
};

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
}

impl WorkspaceNode {
    pub fn set_container(self: &Rc<Self>, container: &Rc<ContainerNode>) {
        let pos = self.position.get();
        container.clone().change_extents(&pos);
        container.clone().set_workspace(self);
        self.container.set(Some(container.clone()));
    }
}

impl Node for WorkspaceNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn visible(&self) -> bool {
        self.visible.get()
    }

    fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        if let Some(container) = self.container.get() {
            container.set_visible(visible);
        }
        self.seat_state.set_visible(self, visible);
    }

    fn destroy_node(&self, detach: bool) {
        if detach {
            self.output.get().remove_child(self);
        }
        self.output_link.set(None);
        if let Some(container) = self.container.take() {
            container.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_workspace(&self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(c) = self.container.get() {
            visitor.visit_container(&c);
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
            n.find_tree_at(x, y, tree);
        }
        FindTreeResult::AcceptsInput
    }

    fn remove_child(self: Rc<Self>, _child: &dyn Node) {
        self.container.set(None);
    }

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_workspace(self, x, y);
    }

    fn accepts_child(&self, node: &dyn Node) -> bool {
        node.is_container()
    }

    fn is_workspace(&self) -> bool {
        true
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        self.position.set(*rect);
        if let Some(c) = self.container.get() {
            c.change_extents(rect);
        }
    }
}
