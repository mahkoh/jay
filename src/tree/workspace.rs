use {
    crate::{
        cursor::KnownCursor,
        ifs::wl_seat::{NodeSeatState, WlSeatGlobal},
        rect::Rect,
        render::Renderer,
        tree::{
            container::ContainerNode, walker::NodeVisitor, ContainingNode, Direction,
            FindTreeResult, FoundNode, Node, NodeId, OutputNode, StackedNode, ToplevelNode,
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
    pub stacked: LinkedList<Rc<dyn StackedNode>>,
    pub seat_state: NodeSeatState,
    pub name: String,
    pub output_link: Cell<Option<LinkedNode<Rc<WorkspaceNode>>>>,
    pub visible: Cell<bool>,
    pub fullscreen: CloneCell<Option<Rc<dyn ToplevelNode>>>,
}

impl WorkspaceNode {
    pub fn clear(&self) {
        self.container.set(None);
        self.output_link.set(None);
        self.fullscreen.set(None);
    }

    pub fn set_container(self: &Rc<Self>, container: &Rc<ContainerNode>) {
        let pos = self.position.get();
        container.clone().tl_change_extents(&pos);
        container.clone().tl_set_workspace(self);
        container.tl_set_parent(self.clone());
        container.tl_set_visible(self.stacked_visible());
        self.container.set(Some(container.clone()));
    }

    pub fn is_empty(&self) -> bool {
        self.stacked.is_empty() && self.fullscreen.get().is_none() && self.container.get().is_none()
    }

    pub fn stacked_visible(&self) -> bool {
        self.visible.get() && self.fullscreen.get().is_none()
    }

    pub fn change_extents(&self, rect: &Rect) {
        self.position.set(*rect);
        if let Some(c) = self.container.get() {
            c.tl_change_extents(rect);
        }
    }

    pub fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        if let Some(fs) = self.fullscreen.get() {
            fs.tl_set_visible(visible);
        } else {
            if let Some(container) = self.container.get() {
                container.tl_set_visible(visible);
            }
            for stacked in self.stacked.iter() {
                stacked.stacked_set_visible(visible);
            }
        }
        self.seat_state.set_visible(self, visible);
    }
}

impl Node for WorkspaceNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_workspace(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(c) = self.container.get() {
            visitor.visit_container(&c);
        }
        if let Some(fs) = self.fullscreen.get() {
            fs.tl_into_node().node_visit(visitor);
        }
    }

    fn node_visible(&self) -> bool {
        self.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.position.get()
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        if let Some(fs) = self.fullscreen.get() {
            fs.tl_into_node().node_do_focus(seat, direction);
        } else if let Some(container) = self.container.get() {
            container.node_do_focus(seat, direction);
        }
    }

    fn node_find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
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

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_workspace(self, x, y);
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("workspace focus");
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn node_into_workspace(self: Rc<Self>) -> Option<Rc<WorkspaceNode>> {
        Some(self.clone())
    }

    fn node_into_containing_node(self: Rc<Self>) -> Option<Rc<dyn ContainingNode>> {
        Some(self)
    }

    fn node_is_workspace(&self) -> bool {
        true
    }
}

impl ContainingNode for WorkspaceNode {
    containing_node_impl!();

    fn cnode_replace_child(self: Rc<Self>, old: &dyn Node, new: Rc<dyn ToplevelNode>) {
        if let Some(container) = self.container.get() {
            if container.node_id() == old.node_id() {
                let new = match new.tl_into_node().node_into_container() {
                    Some(c) => c,
                    _ => {
                        log::error!("cnode_replace_child called with non-container new");
                        return;
                    }
                };
                self.set_container(&new);
                return;
            }
        }
        log::error!("Trying to replace child that's not a child");
    }

    fn cnode_remove_child2(self: Rc<Self>, child: &dyn Node, _preserve_focus: bool) {
        if let Some(container) = self.container.get() {
            if container.node_id() == child.node_id() {
                self.container.set(None);
                return;
            }
        }
        if let Some(fs) = self.fullscreen.get() {
            if fs.tl_as_node().node_id() == child.node_id() {
                self.fullscreen.set(None);
                return;
            }
        }
        log::error!("Trying to remove child that's not a child");
    }

    fn cnode_accepts_child(&self, node: &dyn Node) -> bool {
        node.node_is_container()
    }
}
